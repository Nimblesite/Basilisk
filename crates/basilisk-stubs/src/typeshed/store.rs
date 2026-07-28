//! Implements [STUBRES-TYPESHED-STORE] and [STUBRES-TYPESHED-PIN]. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-STORE.
//!
//! The content-addressed typeshed store: one immutable directory per commit,
//! written only by the download component and read — never repaired, never
//! evicted, never expired — by the checker.
//!
//! ```text
//! <store>/<40-hex commit sha>/
//!   commit-object   # raw Git commit object; hashes to the directory name
//!   manifest.json   # the commit's full Git tree listing (path, blob SHA, mode)
//!   stdlib/… LICENSE NOTICE…
//! ```
//!
//! Reading IS the pin verification ([STUBRES-TYPESHED-PIN]), fully offline:
//!
//! 1. hash `commit-object` — it MUST equal the pinned SHA;
//! 2. read the root-tree SHA out of that verified commit object;
//! 3. hash every materialised file into Git blob IDs and re-hash the full
//!    manifest listing into Git tree objects — the root MUST equal that SHA.
//!
//! The manifest lists the commit's **entire** repository tree while only the
//! `stdlib/` subtree and relevant legal files are materialised on disk. That
//! subset is still fully bound: a materialised file's blob ID is computed from
//! its actual bytes, every other ID comes from the manifest, and any lie in
//! either changes the reconstructed root away from the SHA the verified commit
//! object names. There is no waiver.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::archive::{Archive, ArchiveEntry, ArchiveVfs};
use super::bundle::approved_license_manifest;
use super::gate::manifest::is_legal_file;
use super::gate::{license_gate, safety_gate, shape_gate, SafetyLimits};
use super::gittree::{
    commit_root_tree, git_blob_oid, git_commit_oid, reconstruct_root_tree_oid, FileMode, GitFile,
    Oid,
};
use super::snapshot::Snapshot;
use super::source::{LicenseStatus, SourceIdentity, SourceKind, TypeshedStatus};

/// The raw commit object file inside a store entry.
pub const COMMIT_OBJECT_FILE: &str = "commit-object";
/// The tree-listing manifest inside a store entry.
pub const MANIFEST_FILE: &str = "manifest.json";

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// One leaf of the commit's full repository tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreTreeFile {
    /// Repository-relative path.
    pub path: String,
    /// Full Git blob object ID (hex).
    pub oid: String,
    /// Canonical Git leaf mode (`100644`, `100755`, `120000`, or `160000`).
    pub mode: String,
}

/// The store entry manifest: the commit's complete Git tree listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreManifest {
    /// The commit SHA (hex) — must match the directory name.
    pub commit: String,
    /// The commit's root-tree SHA (hex) — must match the commit object.
    pub tree: String,
    /// Every leaf of the commit's repository tree.
    pub tree_files: Vec<StoreTreeFile>,
}

/// Why a store entry could not activate. Paths never appear here; the caller
/// knows the store root and the SHA it asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// No entry directory exists for the requested commit.
    #[error("no store entry for this commit")]
    Missing,
    /// The entry exists but failed offline verification or is unreadable.
    #[error("store entry failed offline verification")]
    Corrupt,
    /// The entry verified but its legal-file identity is not the approved one.
    #[error("store entry license identity changed")]
    LicenseChanged,
}

/// The store entry directory for a commit.
#[must_use]
pub fn entry_dir(store_root: &Path, commit: Oid) -> PathBuf {
    store_root.join(commit.to_hex())
}

/// Whether a store entry materialises this tree path on disk: the `stdlib/`
/// subtree plus the relevant legal files ([STUBRES-TYPESHED-STORE]). The rule
/// is fixed in code — never in the manifest — so a tampered manifest cannot
/// silently shrink what must exist on disk.
#[must_use]
pub fn is_materialized(path: &str) -> bool {
    path.starts_with("stdlib/") || is_legal_file(path)
}

/// Read and verify one store entry into an immutable snapshot
/// ([STUBRES-TYPESHED-PIN]). `explicit` records whether the pin is a user's
/// `typeshed-commit` (suppressing `typeshed_source_unpinned`) or the bundled default.
///
/// # Errors
///
/// Returns [`StoreError::Missing`] when no entry exists,
/// [`StoreError::LicenseChanged`] on approved-identity drift, and
/// [`StoreError::Corrupt`] on any verification failure.
pub fn read_snapshot(
    store_root: &Path,
    commit: Oid,
    explicit: bool,
) -> Result<Snapshot, StoreError> {
    let dir = entry_dir(store_root, commit);
    if !dir.is_dir() {
        return Err(StoreError::Missing);
    }
    let tree = verified_commit_tree(&dir, commit)?;
    let manifest = read_manifest(&dir, commit, tree)?;
    let archive = verified_archive(&dir, &manifest)?;
    gate(&archive)?;
    build_snapshot(commit, tree, explicit, archive)
}

/// Steps 1–2 of the chain: the stored commit object must hash to the pinned
/// SHA, and only then is its tree header trusted.
fn verified_commit_tree(dir: &Path, commit: Oid) -> Result<Oid, StoreError> {
    let bytes = fs::read(dir.join(COMMIT_OBJECT_FILE)).map_err(|_error| StoreError::Corrupt)?;
    if git_commit_oid(&bytes) != commit {
        return Err(StoreError::Corrupt);
    }
    commit_root_tree(&bytes).map_err(|_error| StoreError::Corrupt)
}

fn read_manifest(dir: &Path, commit: Oid, tree: Oid) -> Result<StoreManifest, StoreError> {
    let bytes = fs::read(dir.join(MANIFEST_FILE)).map_err(|_error| StoreError::Corrupt)?;
    let manifest: StoreManifest =
        serde_json::from_slice(&bytes).map_err(|_error| StoreError::Corrupt)?;
    if manifest.commit != commit.to_hex() || manifest.tree != tree.to_hex() {
        return Err(StoreError::Corrupt);
    }
    Ok(manifest)
}

/// Step 3 of the chain: hash the materialised bytes, then re-hash the full
/// listing into Git tree objects and require the verified commit's root.
fn verified_archive(dir: &Path, manifest: &StoreManifest) -> Result<Archive, StoreError> {
    let expected_root = Oid::from_hex(&manifest.tree).map_err(|_error| StoreError::Corrupt)?;
    let mut git_files = Vec::with_capacity(manifest.tree_files.len());
    let mut entries = Vec::new();
    for file in &manifest.tree_files {
        let mode = parse_mode(&file.mode)?;
        let recorded = Oid::from_hex(&file.oid).map_err(|_error| StoreError::Corrupt)?;
        let oid = if is_materialized(&file.path) {
            // A materialised leaf's ID comes from its actual on-disk bytes, so
            // tampering with either the file or its manifest row breaks the root.
            if !matches!(mode, FileMode::Regular | FileMode::Executable) {
                return Err(StoreError::Corrupt);
            }
            let data = read_materialized(dir, &file.path)?;
            let actual = git_blob_oid(&data);
            if actual != recorded {
                return Err(StoreError::Corrupt);
            }
            entries.push(ArchiveEntry {
                path: file.path.clone(),
                mode,
                data,
            });
            actual
        } else {
            recorded
        };
        git_files.push(GitFile {
            path: file.path.clone(),
            oid,
            mode,
        });
    }
    let root = reconstruct_root_tree_oid(&git_files).map_err(|_error| StoreError::Corrupt)?;
    if root != expected_root {
        return Err(StoreError::Corrupt);
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Archive::new(entries))
}

/// Read one materialised file, refusing any path the safety rules would refuse
/// in an archive (the manifest is untrusted input until the root hash matches).
fn read_materialized(dir: &Path, path: &str) -> Result<Vec<u8>, StoreError> {
    if path.split('/').any(|segment| {
        segment.is_empty() || segment == "." || segment == ".." || segment.contains('\\')
    }) {
        return Err(StoreError::Corrupt);
    }
    fs::read(dir.join(path)).map_err(|_error| StoreError::Corrupt)
}

fn gate(archive: &Archive) -> Result<(), StoreError> {
    safety_gate(archive, &SafetyLimits::default()).map_err(|_error| StoreError::Corrupt)?;
    shape_gate(archive).map_err(|_error| StoreError::Corrupt)?;
    let approved = approved_license_manifest().map_err(|_error| StoreError::Corrupt)?;
    license_gate(archive, &approved).map_err(|_error| StoreError::LicenseChanged)
}

fn build_snapshot(
    commit: Oid,
    tree: Oid,
    explicit: bool,
    archive: Archive,
) -> Result<Snapshot, StoreError> {
    let identity = SourceIdentity::Commit {
        commit,
        pinned: explicit,
    };
    let status = TypeshedStatus {
        active_source: SourceKind::ExactCommit,
        commit: Some(commit),
        tree: Some(tree),
        license_status: LicenseStatus::Approved,
        license_reference: Some(format!(
            "https://github.com/python/typeshed/blob/{commit}/LICENSE"
        )),
        warnings: Vec::new(),
    };
    let uri_identity = identity.uri_component();
    Snapshot::build(
        identity,
        status,
        ArchiveVfs::new(uri_identity, archive),
        None,
    )
    .map_err(|_error| StoreError::Corrupt)
}

fn parse_mode(mode: &str) -> Result<FileMode, StoreError> {
    match mode {
        "100644" => Ok(FileMode::Regular),
        "100755" => Ok(FileMode::Executable),
        "120000" => Ok(FileMode::Symlink),
        "160000" => Ok(FileMode::Submodule),
        _ => Err(StoreError::Corrupt),
    }
}

/// Everything the download component hands over for one accepted commit.
#[derive(Debug, Clone)]
pub struct StoreEntry {
    /// The commit SHA the entry is addressed by.
    pub commit: Oid,
    /// The raw commit object whose hash IS `commit`.
    pub commit_object: Vec<u8>,
    /// The commit's full tree listing.
    pub manifest: StoreManifest,
    /// The materialised files (`stdlib/…` + legal files) with verified bytes.
    pub files: Vec<ArchiveEntry>,
}

/// Write one store entry atomically: everything lands in a staging directory
/// that is renamed into place, so an interrupted download leaves **nothing**
/// ([STUBRES-TYPESHED-DOWNLOAD]). An existing entry for the commit is kept —
/// entries are content-addressed and immutable, so there is nothing to update.
///
/// # Errors
///
/// Returns [`StoreError::Corrupt`] on any I/O or serialization failure.
pub fn write_entry(store_root: &Path, entry: &StoreEntry) -> Result<(), StoreError> {
    let target = entry_dir(store_root, entry.commit);
    if target.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(store_root).map_err(|_error| StoreError::Corrupt)?;
    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let staging = store_root.join(format!(".stage-{}-{sequence}", std::process::id()));
    let staged = stage_entry(&staging, entry);
    match staged.and_then(|()| promote(&staging, &target)) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(error)
        }
    }
}

fn stage_entry(staging: &Path, entry: &StoreEntry) -> Result<(), StoreError> {
    fs::create_dir(staging).map_err(|_error| StoreError::Corrupt)?;
    fs::write(staging.join(COMMIT_OBJECT_FILE), &entry.commit_object)
        .map_err(|_error| StoreError::Corrupt)?;
    let manifest =
        serde_json::to_vec_pretty(&entry.manifest).map_err(|_error| StoreError::Corrupt)?;
    fs::write(staging.join(MANIFEST_FILE), manifest).map_err(|_error| StoreError::Corrupt)?;
    for file in &entry.files {
        let path = staging.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_error| StoreError::Corrupt)?;
        }
        fs::write(&path, &file.data).map_err(|_error| StoreError::Corrupt)?;
    }
    Ok(())
}

fn promote(staging: &Path, target: &Path) -> Result<(), StoreError> {
    match fs::rename(staging, target) {
        Ok(()) => Ok(()),
        // A concurrent download of the same commit won the rename; the entry
        // is content-addressed, so the winner's bytes are equally correct.
        Err(_error) if target.is_dir() => {
            let _ = fs::remove_dir_all(staging);
            Ok(())
        }
        Err(_error) => Err(StoreError::Corrupt),
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only store fixtures require infallible setup"
)]
mod tests {
    use super::*;

    /// A minimal but complete repository tree: a materialised stdlib plus the
    /// **real** bundle LICENSE (so the approved-identity gate passes), plus an
    /// unmaterialised repo file bound only through the manifest.
    fn fixture_entry() -> (StoreEntry, Oid) {
        let bundle = super::super::bundle::bundled_snapshot().expect("bundled snapshot");
        let license_body = bundle.vfs.read("LICENSE").expect("bundle LICENSE").to_vec();
        let files = vec![
            ArchiveEntry {
                path: "LICENSE".to_owned(),
                mode: FileMode::Regular,
                data: license_body,
            },
            ArchiveEntry {
                path: "stdlib/VERSIONS".to_owned(),
                mode: FileMode::Regular,
                data: b"os: 3.0-\n".to_vec(),
            },
            ArchiveEntry {
                path: "stdlib/os.pyi".to_owned(),
                mode: FileMode::Regular,
                data: b"def getcwd() -> str: ...\n".to_vec(),
            },
        ];
        let unmaterialized = GitFile {
            path: "README.md".to_owned(),
            oid: git_blob_oid(b"readme body\n"),
            mode: FileMode::Regular,
        };
        let mut git_files: Vec<GitFile> = files
            .iter()
            .map(|entry| GitFile {
                path: entry.path.clone(),
                oid: git_blob_oid(&entry.data),
                mode: entry.mode,
            })
            .collect();
        git_files.push(unmaterialized);
        let tree = reconstruct_root_tree_oid(&git_files).expect("fixture tree");
        let commit_object =
            format!("tree {tree}\nauthor a <a@a> 0 +0000\ncommitter a <a@a> 0 +0000\n\nfixture\n")
                .into_bytes();
        let commit = git_commit_oid(&commit_object);
        let manifest = StoreManifest {
            commit: commit.to_hex(),
            tree: tree.to_hex(),
            tree_files: git_files
                .iter()
                .map(|file| StoreTreeFile {
                    path: file.path.clone(),
                    oid: file.oid.to_hex(),
                    mode: file.mode.as_str().to_owned(),
                })
                .collect(),
        };
        (
            StoreEntry {
                commit,
                commit_object,
                manifest,
                files,
            },
            commit,
        )
    }

    #[test]
    fn missing_entry_is_missing_never_created() {
        let root = tempfile::tempdir().expect("tempdir");
        let (_, commit) = fixture_entry();
        assert_eq!(
            read_snapshot(root.path(), commit, true).err(),
            Some(StoreError::Missing)
        );
        // The checker never creates, repairs, or evicts: reading leaves the
        // store untouched ([STUBRES-TYPESHED-STORE] inert-store rule).
        assert_eq!(
            fs::read_dir(root.path()).expect("readdir").count(),
            0,
            "a read must not write"
        );
    }

    #[test]
    fn write_then_read_verifies_the_full_offline_chain() {
        let root = tempfile::tempdir().expect("tempdir");
        let (entry, commit) = fixture_entry();
        write_entry(root.path(), &entry).expect("the fixture entry writes cleanly");
        let snapshot = read_snapshot(root.path(), commit, true).expect("verified entry");
        assert_eq!(snapshot.status.active_source, SourceKind::ExactCommit);
        assert_eq!(snapshot.status.commit, Some(commit));
        assert!(snapshot.status.warnings.is_empty());
        assert_eq!(
            snapshot.read_stub("os").map(|(_, body)| body),
            Some("def getcwd() -> str: ...\n")
        );
        // An unmaterialised repo file participates in the root hash but never
        // reaches the VFS.
        assert!(snapshot.vfs.read("README.md").is_none());
        // The default (bundled) pin keeps typeshed_source_unpinned semantics via `pinned`.
        let default_pin = read_snapshot(root.path(), commit, false).expect("default pin");
        assert!(!default_pin.identity.is_pinned());
    }

    #[test]
    fn license_drift_blocks_a_chain_that_otherwise_verifies() {
        let root = tempfile::tempdir().expect("tempdir");
        let (mut entry, _) = fixture_entry();
        // A consistent entry (commit object, manifest, and bytes all agree)
        // whose LICENSE is not the build-approved identity: the chain passes,
        // the License gate blocks ([STUBRES-TYPESHED-LICENSE]).
        for file in &mut entry.files {
            if file.path == "LICENSE" {
                file.data = b"a different license\n".to_vec();
            }
        }
        let rebuilt = rebuild_identity(entry);
        let commit = rebuilt.commit;
        write_entry(root.path(), &rebuilt).expect("the rebuilt entry writes cleanly");
        assert_eq!(
            read_snapshot(root.path(), commit, true).err(),
            Some(StoreError::LicenseChanged)
        );
    }

    /// Recompute manifest, tree, and commit object after editing fixture bytes,
    /// so the entry is self-consistent and only the approved identity differs.
    fn rebuild_identity(mut entry: StoreEntry) -> StoreEntry {
        let mut git_files: Vec<GitFile> = entry
            .files
            .iter()
            .map(|file| GitFile {
                path: file.path.clone(),
                oid: git_blob_oid(&file.data),
                mode: file.mode,
            })
            .collect();
        git_files.push(GitFile {
            path: "README.md".to_owned(),
            oid: git_blob_oid(b"readme body\n"),
            mode: FileMode::Regular,
        });
        let tree = reconstruct_root_tree_oid(&git_files).expect("rebuilt tree");
        entry.commit_object =
            format!("tree {tree}\nauthor a <a@a> 0 +0000\ncommitter a <a@a> 0 +0000\n\nfixture\n")
                .into_bytes();
        entry.commit = git_commit_oid(&entry.commit_object);
        entry.manifest = StoreManifest {
            commit: entry.commit.to_hex(),
            tree: tree.to_hex(),
            tree_files: git_files
                .iter()
                .map(|file| StoreTreeFile {
                    path: file.path.clone(),
                    oid: file.oid.to_hex(),
                    mode: file.mode.as_str().to_owned(),
                })
                .collect(),
        };
        entry
    }

    #[test]
    fn any_mutated_byte_fails_the_pin() {
        let root = tempfile::tempdir().expect("tempdir");
        let (entry, commit) = fixture_entry();
        write_entry(root.path(), &entry).expect("the fixture entry writes cleanly");
        let stub = entry_dir(root.path(), commit).join("stdlib/os.pyi");
        fs::write(&stub, b"def getcwd() -> bytes: ...\n").expect("tamper");
        assert_eq!(
            read_snapshot(root.path(), commit, true).err(),
            Some(StoreError::Corrupt)
        );
    }

    #[test]
    fn a_tampered_commit_object_fails_before_its_tree_is_trusted() {
        let root = tempfile::tempdir().expect("tempdir");
        let (entry, commit) = fixture_entry();
        write_entry(root.path(), &entry).expect("the fixture entry writes cleanly");
        let commit_object = entry_dir(root.path(), commit).join(COMMIT_OBJECT_FILE);
        // Rewrite the commit object to name a different tree; it no longer
        // hashes to the directory's SHA, so nothing after it is consulted.
        fs::write(
            &commit_object,
            b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\nforged\n",
        )
        .expect("tamper");
        assert_eq!(
            read_snapshot(root.path(), commit, true).err(),
            Some(StoreError::Corrupt)
        );
    }

    #[test]
    fn a_manifest_lie_about_an_unmaterialized_file_breaks_the_root() {
        let root = tempfile::tempdir().expect("tempdir");
        let (mut entry, commit) = fixture_entry();
        // Lie about the README blob: the root reconstruction then cannot reach
        // the tree SHA the verified commit object names.
        for file in &mut entry.manifest.tree_files {
            if file.path == "README.md" {
                file.oid = git_blob_oid(b"a different readme\n").to_hex();
            }
        }
        write_entry(root.path(), &entry).expect("the fixture entry writes cleanly");
        assert_eq!(
            read_snapshot(root.path(), commit, true).err(),
            Some(StoreError::Corrupt)
        );
    }

    #[test]
    fn a_manifest_that_drops_a_materialized_file_breaks_the_root() {
        let root = tempfile::tempdir().expect("tempdir");
        let (mut entry, commit) = fixture_entry();
        entry
            .manifest
            .tree_files
            .retain(|file| file.path != "stdlib/os.pyi");
        write_entry(root.path(), &entry).expect("the fixture entry writes cleanly");
        assert_eq!(
            read_snapshot(root.path(), commit, true).err(),
            Some(StoreError::Corrupt)
        );
    }

    #[test]
    fn traversal_paths_in_a_manifest_never_escape_the_entry() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("0123456789012345678901234567890123456789");
        fs::create_dir_all(&dir).expect("entry dir");
        for path in ["stdlib/../../../etc/passwd", "stdlib//x.pyi", "stdlib/.\\x"] {
            assert_eq!(
                read_materialized(&dir, path).err(),
                Some(StoreError::Corrupt),
                "{path} must be refused"
            );
        }
    }

    #[test]
    fn interrupted_staging_never_becomes_an_entry() {
        let root = tempfile::tempdir().expect("tempdir");
        let (entry, commit) = fixture_entry();
        // Simulate an interrupt: a stale staging directory exists.
        let staging = root.path().join(".stage-dead-0");
        fs::create_dir_all(staging.join("stdlib")).expect("staging");
        fs::write(staging.join(COMMIT_OBJECT_FILE), b"partial").expect("partial");
        assert_eq!(
            read_snapshot(root.path(), commit, true).err(),
            Some(StoreError::Missing),
            "a staging directory is not an entry"
        );
        write_entry(root.path(), &entry).expect("the fixture entry writes cleanly");
        // The real entry activates regardless of the stale staging litter.
        let snapshot = read_snapshot(root.path(), commit, true).expect("the real entry activates");
        assert_eq!(snapshot.status.commit, Some(commit));
    }

    #[test]
    fn an_existing_entry_is_immutable_under_rewrites() {
        let root = tempfile::tempdir().expect("tempdir");
        let (entry, commit) = fixture_entry();
        write_entry(root.path(), &entry).expect("the fixture entry writes cleanly");
        let marker = entry_dir(root.path(), commit).join("stdlib/marker.pyi");
        fs::write(&marker, b"x: int\n").expect("marker");
        // A second write of the same commit is a no-op, not a replacement.
        write_entry(root.path(), &entry).expect("the fixture entry writes cleanly");
        assert!(
            marker.exists(),
            "content-addressed entries are never rewritten"
        );
    }
}
