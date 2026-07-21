//! Tests [STUBRES-TYPESHED-DOWNLOAD] end to end, offline: a fake GitHub API
//! serving a synthetic-but-honest repository (real Git hashing, real zip
//! encoding, the real approved LICENSE bytes) drives the full pipeline into a
//! temp store, and `basilisk_stubs::typeshed::store::read_snapshot` — the
//! checker's own offline reader — verifies what was written. Every failure
//! phase must leave the store byte-for-byte empty (atomic download).

use std::cell::RefCell;
use std::io::Write as _;

use basilisk_stubs::typeshed::bundle::bundled_snapshot;
use basilisk_stubs::typeshed::gittree::{
    git_commit_oid, reconstruct_root_tree_oid, GitFile,
};
use basilisk_stubs::typeshed::store::read_snapshot;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

use super::github::CommitInfo;
use super::*;

/// A complete fake repository at one commit: the trusted tree, the raw commit
/// object (unsigned, so payload == raw), and the file bytes.
struct FakeRepo {
    commit: Oid,
    tree: Oid,
    payload: String,
    tree_entries: Vec<TreeEntry>,
    files: Vec<(String, Vec<u8>)>,
}

fn fake_repo() -> FakeRepo {
    let license = bundled_snapshot()
        .ok()
        .and_then(|bundle| bundle.vfs.read("LICENSE").map(<[u8]>::to_vec))
        .unwrap_or_default();
    let files: Vec<(String, Vec<u8>)> = vec![
        ("LICENSE".to_owned(), license),
        ("README.md".to_owned(), b"readme body\n".to_vec()),
        ("stdlib/VERSIONS".to_owned(), b"os: 3.0-\n".to_vec()),
        (
            "stdlib/os.pyi".to_owned(),
            b"def getcwd() -> str: ...\n".to_vec(),
        ),
    ];
    let tree_entries: Vec<TreeEntry> = files
        .iter()
        .map(|(path, data)| TreeEntry {
            path: path.clone(),
            oid: git_blob_oid(data),
            mode: FileMode::Regular,
        })
        .collect();
    let git_files: Vec<GitFile> = tree_entries
        .iter()
        .map(|entry| GitFile {
            path: entry.path.clone(),
            oid: entry.oid,
            mode: entry.mode,
        })
        .collect();
    let tree = reconstruct_root_tree_oid(&git_files).unwrap_or_else(|_error| git_blob_oid(b""));
    let payload =
        format!("tree {tree}\nauthor a <a@a> 0 +0000\ncommitter a <a@a> 0 +0000\n\nfixture\n");
    let commit = git_commit_oid(payload.as_bytes());
    FakeRepo {
        commit,
        tree,
        payload,
        tree_entries,
        files,
    }
}

/// Encode the repo files as a codeload-style zipball (single root prefix).
fn zipball(repo: &FakeRepo) -> Vec<u8> {
    let prefix = format!("python-typeshed-{}", repo.commit);
    let mut buffer = Vec::new();
    {
        let mut writer = ZipWriter::new(std::io::Cursor::new(&mut buffer));
        for (path, data) in &repo.files {
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Stored)
                .unix_permissions(0o644);
            if writer.start_file(format!("{prefix}/{path}"), options).is_err() {
                return Vec::new();
            }
            if writer.write_all(data).is_err() {
                return Vec::new();
            }
        }
        if writer.finish().is_err() {
            return Vec::new();
        }
    }
    buffer
}

/// One switchable failure per pipeline phase.
#[derive(Default, Clone, Copy)]
struct Faults {
    resolve_fails: bool,
    tree_fails: bool,
    archive_fails: bool,
}

struct FakeApi {
    repo: FakeRepo,
    archive: Vec<u8>,
    faults: Faults,
}

impl FakeApi {
    fn new(repo: FakeRepo) -> Self {
        let archive = zipball(&repo);
        Self {
            repo,
            archive,
            faults: Faults::default(),
        }
    }
}

impl GithubApi for FakeApi {
    fn resolve(&self, _reference: &str) -> Result<CommitInfo, TransportError> {
        if self.faults.resolve_fails {
            return Err(TransportError::Metadata);
        }
        Ok(CommitInfo {
            commit: self.repo.commit,
            tree: self.repo.tree,
            payload: self.repo.payload.clone(),
            signature: None,
        })
    }

    fn fetch_tree(&self, _root_tree: Oid) -> Result<Vec<TreeEntry>, TransportError> {
        if self.faults.tree_fails {
            return Err(TransportError::Metadata);
        }
        Ok(self.repo.tree_entries.clone())
    }

    fn fetch_archive(&self, _commit: Oid) -> Result<Vec<u8>, TransportError> {
        if self.faults.archive_fails {
            return Err(TransportError::Download);
        }
        Ok(self.archive.clone())
    }
}

fn store_entry_count(root: &std::path::Path) -> usize {
    std::fs::read_dir(root).map_or(0, Iterator::count)
}

#[expect(
    clippy::expect_used,
    reason = "test-only fixtures require infallible setup"
)]
mod cases {
    use super::*;

    /// The acceptance test: download latest, then read the entry back through
    /// the checker's own offline verifier — the exact code path analysis uses.
    #[test]
    fn download_latest_round_trips_through_the_offline_store_reader() {
        let root = tempfile::tempdir().expect("tempdir");
        let api = FakeApi::new(fake_repo());
        let phases = RefCell::new(Vec::new());
        let outcome = download_latest(Some(root.path().to_path_buf()), &api, &|phase| {
            phases.borrow_mut().push(phase);
        })
        .expect("download must succeed");
        assert_eq!(outcome.commit, api.repo.commit);
        assert_eq!(outcome.tree, api.repo.tree);
        assert_eq!(
            phases.into_inner(),
            vec![
                DownloadPhase::Resolving,
                DownloadPhase::FetchingTree,
                DownloadPhase::FetchingArchive,
                DownloadPhase::Verifying,
                DownloadPhase::Writing,
            ]
        );
        let snapshot =
            read_snapshot(root.path(), outcome.commit, true).expect("offline verification");
        assert_eq!(
            snapshot.read_stub("os").map(|(_, body)| body),
            Some("def getcwd() -> str: ...\n")
        );
        // Unmaterialised repo files participate in the pin, never in the VFS.
        assert!(snapshot.vfs.read("README.md").is_none());
    }

    /// `download_commit` is for materialising an existing pin; the resolved
    /// SHA must be the requested one, and any other answer is terminal.
    #[test]
    fn download_commit_verifies_the_resolved_sha_against_the_request() {
        let root = tempfile::tempdir().expect("tempdir");
        let api = FakeApi::new(fake_repo());
        let requested = api.repo.commit;
        assert!(download_commit(requested, Some(root.path().to_path_buf()), &api, &|_phase| {})
            .is_ok());

        let other_root = tempfile::tempdir().expect("tempdir");
        let other = git_blob_oid(b"a different commit entirely");
        assert_eq!(
            download_commit(other, Some(other_root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::Metadata)
        );
        assert_eq!(store_entry_count(other_root.path()), 0);
    }

    /// Every failure phase leaves the store empty — the atomic-download
    /// acceptance item ([STUBRES-TYPESHED-DOWNLOAD]).
    #[test]
    fn every_transport_failure_writes_nothing() {
        let cases = [
            (
                Faults {
                    resolve_fails: true,
                    ..Faults::default()
                },
                DownloadError::Metadata,
            ),
            (
                Faults {
                    tree_fails: true,
                    ..Faults::default()
                },
                DownloadError::Metadata,
            ),
            (
                Faults {
                    archive_fails: true,
                    ..Faults::default()
                },
                DownloadError::Download,
            ),
        ];
        for (faults, expected) in cases {
            let root = tempfile::tempdir().expect("tempdir");
            let mut api = FakeApi::new(fake_repo());
            api.faults = faults;
            assert_eq!(
                download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
                Err(expected)
            );
            assert_eq!(store_entry_count(root.path()), 0, "store must stay empty");
        }
    }

    /// A payload that does not hash to the reported SHA — a lying or tampered
    /// metadata response — is rejected before anything is fetched or written.
    #[test]
    fn a_commit_payload_that_misses_its_sha_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut api = FakeApi::new(fake_repo());
        api.repo.payload = api.repo.payload.replace("fixture", "tampered");
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::Validation)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// The API's tree SHA must agree with the tree named inside the verified
    /// commit object; a divergence means the tree listing cannot be trusted.
    #[test]
    fn a_tree_sha_disagreeing_with_the_commit_object_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut api = FakeApi::new(fake_repo());
        api.repo.tree = git_blob_oid(b"some other tree");
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::Validation)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// Archive bytes that differ from the trusted tree — one flipped stub —
    /// fail content binding even though the zip itself is well-formed.
    #[test]
    fn tampered_archive_bytes_fail_content_binding_and_write_nothing() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut repo = fake_repo();
        for (path, data) in &mut repo.files {
            if path == "stdlib/os.pyi" {
                *data = b"def getcwd() -> bytes: ...\n".to_vec();
            }
        }
        // The tree entries still describe the ORIGINAL bytes.
        let honest = fake_repo();
        repo.tree_entries = honest.tree_entries;
        repo.tree = honest.tree;
        repo.payload = honest.payload;
        repo.commit = honest.commit;
        let api = FakeApi::new(repo);
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::Validation)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// An archive with a file the tree does not list (or vice versa) fails the
    /// exact path-set binding.
    #[test]
    fn an_archive_file_missing_from_the_tree_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut repo = fake_repo();
        repo.files.push(("smuggled.py".to_owned(), b"x\n".to_vec()));
        let api = FakeApi::new(repo);
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::Validation)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// A commit whose legal identity drifted from the build-approved one is
    /// blocked as `LicenseChanged` — distinct from generic validation so the
    /// surface can say "pending legal review" ([STUBRES-TYPESHED-LICENSE]).
    #[test]
    fn license_drift_is_blocked_as_license_changed() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut repo = fake_repo();
        for (path, data) in &mut repo.files {
            if path == "LICENSE" {
                *data = b"a different license\n".to_vec();
            }
        }
        // Rebuild an internally consistent identity around the drifted bytes.
        let tree_entries: Vec<TreeEntry> = repo
            .files
            .iter()
            .map(|(path, data)| TreeEntry {
                path: path.clone(),
                oid: git_blob_oid(data),
                mode: FileMode::Regular,
            })
            .collect();
        let git_files: Vec<GitFile> = tree_entries
            .iter()
            .map(|entry| GitFile {
                path: entry.path.clone(),
                oid: entry.oid,
                mode: entry.mode,
            })
            .collect();
        let tree = reconstruct_root_tree_oid(&git_files).expect("rebuilt tree");
        repo.payload = format!(
            "tree {tree}\nauthor a <a@a> 0 +0000\ncommitter a <a@a> 0 +0000\n\nfixture\n"
        );
        repo.commit = git_commit_oid(repo.payload.as_bytes());
        repo.tree = tree;
        repo.tree_entries = tree_entries;
        let api = FakeApi::new(repo);
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::LicenseChanged)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// Re-downloading an existing commit is a no-op success: the store is
    /// content-addressed and immutable.
    #[test]
    fn redownloading_an_existing_commit_succeeds_without_rewriting() {
        let root = tempfile::tempdir().expect("tempdir");
        let api = FakeApi::new(fake_repo());
        let store = Some(root.path().to_path_buf());
        let first = download_latest(store.clone(), &api, &|_phase| {}).expect("first download");
        let second = download_latest(store, &api, &|_phase| {}).expect("second download");
        assert_eq!(first, second);
        assert_eq!(store_entry_count(root.path()), 1);
    }
}
