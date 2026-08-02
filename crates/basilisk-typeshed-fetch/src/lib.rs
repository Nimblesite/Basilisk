//! Implements [STUBRES-TYPESHED-DOWNLOAD] and [TYPESHEDRT-SEGREGATION]. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-DOWNLOAD.
//!
//! The typeshed download component — the ONLY typeshed code in the workspace
//! that opens a network connection. It runs only when a person invokes it
//! (`basilisk typeshed download`, the editor's Download buttons); nothing on
//! the analysis path can call it, because the checker's crates do not depend
//! on this one (asserted by `scripts/check-dependency-shape.sh`).
//!
//! A download: resolve commit metadata over authenticated HTTPS, reconstruct
//! the raw commit object and require it to hash to the requested SHA, fetch
//! the trusted recursive tree, download the codeload archive, bind every byte
//! to that tree through the four activation gates, then dump the accepted
//! `stdlib/` subset + legal files, the commit object, and the full tree
//! manifest into the content-addressed store. **A failure at any step writes
//! nothing** — no partial entry, no unverified entry, no config change.

mod commit;
mod github;
mod pypi;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;

use std::path::PathBuf;

use basilisk_stubs::typeshed::archive::{Archive, ArchiveEntry};
use basilisk_stubs::typeshed::bundle::approved_license_manifest;
use basilisk_stubs::typeshed::codec::{decode_zip, DecodeLimits, ZipLayout};
use basilisk_stubs::typeshed::gate::{
    manifest::sha256_hex, run_activation, safety_gate, shape_gate, GateConfig, GateError,
    SafetyLimits,
};
use basilisk_stubs::typeshed::gittree::{git_blob_oid, FileMode, Oid};
use basilisk_stubs::typeshed::runtime::default_store_path;
use basilisk_stubs::typeshed::store::{
    self, is_materialized, StoreEntry, StoreManifest, StoreTreeFile,
};
use basilisk_stubs::typeshed::wheel;

pub use github::{CommitInfo, GithubApi, GithubClient, TransportError, TreeEntry};
pub use pypi::{PypiApi, PypiClient};

/// Coarse download progress, for surfaces that render it on the invoking
/// control ([LSPCFGED-TYPESHED-DOWNLOAD]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPhase {
    /// Resolving commit metadata.
    Resolving,
    /// Fetching the trusted recursive tree.
    FetchingTree,
    /// Downloading the archive bytes.
    FetchingArchive,
    /// Running the activation gates and commit-object verification.
    Verifying,
    /// Writing the store entry.
    Writing,
}

/// A terminal, redacted download failure. Nothing was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    /// Commit or tree metadata could not be resolved (offline, rate-limited,
    /// or inconsistent with the requested SHA).
    #[error("typeshed metadata resolution failed")]
    Metadata,
    /// The archive could not be downloaded.
    #[error("typeshed archive download failed")]
    Download,
    /// The archive or commit object failed verification against the requested
    /// identity, or a gate rejected it.
    #[error("typeshed download failed verification")]
    Validation,
    /// The legal-file identity is not the build-approved one; activation is
    /// blocked pending review ([STUBRES-TYPESHED-LICENSE]).
    #[error("typeshed license identity changed")]
    LicenseChanged,
    /// The verified entry could not be written to the store.
    #[error("typeshed store write failed")]
    Store,
}

/// What a completed download materialised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadOutcome {
    /// The commit now present, verified, in the store. **Download latest**
    /// callers write this as `typeshed-commit`; **Download pinned** callers
    /// write nothing ([STUBRES-TYPESHED-DOWNLOAD]).
    pub commit: Oid,
    /// The commit's verified root tree.
    pub tree: Oid,
}

/// Download the current `python/typeshed@main` into the store. The caller is
/// responsible for writing the returned SHA as `typeshed-commit` — that
/// config write is the *action's* contract, not this component's.
///
/// # Errors
///
/// Returns a redacted [`DownloadError`]; on any error nothing was written.
pub fn download_latest(
    store_path: Option<PathBuf>,
    api: &dyn GithubApi,
    progress: &dyn Fn(DownloadPhase),
) -> Result<DownloadOutcome, DownloadError> {
    download("main", None, store_path, api, progress)
}

/// Download one exact commit into the store (materialising an existing pin on
/// a machine that never downloaded it). Writes no configuration.
///
/// # Errors
///
/// Returns a redacted [`DownloadError`]; on any error nothing was written.
pub fn download_commit(
    commit: Oid,
    store_path: Option<PathBuf>,
    api: &dyn GithubApi,
    progress: &dyn Fn(DownloadPhase),
) -> Result<DownloadOutcome, DownloadError> {
    download(&commit.to_hex(), Some(commit), store_path, api, progress)
}

fn download(
    reference: &str,
    expected: Option<Oid>,
    store_path: Option<PathBuf>,
    api: &dyn GithubApi,
    progress: &dyn Fn(DownloadPhase),
) -> Result<DownloadOutcome, DownloadError> {
    let store_root = store_path
        .or_else(default_store_path)
        .ok_or(DownloadError::Store)?;
    progress(DownloadPhase::Resolving);
    let info = api
        .resolve(reference)
        .map_err(|_error| DownloadError::Metadata)?;
    // `commits/{ref}` resolves branches and tags as well as SHAs, so a pin is
    // re-checked against the response rather than assumed.
    if expected.is_some_and(|requested| requested != info.commit) {
        return Err(DownloadError::Metadata);
    }
    // Reconstruct the raw commit object and require it to hash to the SHA —
    // the same object the checker will re-hash offline on every activation
    // ([STUBRES-TYPESHED-PIN]). Its tree must agree with the API's.
    let object = commit::reconstruct(&info.payload, info.signature.as_deref(), info.commit)
        .map_err(|_error| DownloadError::Validation)?;
    if object.tree != info.tree {
        return Err(DownloadError::Validation);
    }
    progress(DownloadPhase::FetchingTree);
    let entries = api
        .fetch_tree(object.tree)
        .map_err(|_error| DownloadError::Metadata)?;
    progress(DownloadPhase::FetchingArchive);
    let bytes = api
        .fetch_archive(info.commit)
        .map_err(|_error| DownloadError::Download)?;
    progress(DownloadPhase::Verifying);
    let archive = decode_zip(
        &bytes,
        ZipLayout::CodeloadPrefixed,
        &DecodeLimits::default(),
    )
    .map_err(|_error| DownloadError::Validation)?;
    let bound = bind_to_tree(&archive, &entries)?;
    let approved = approved_license_manifest().map_err(|_error| DownloadError::Validation)?;
    let config = GateConfig {
        limits: SafetyLimits::default(),
        approved_license: approved,
        expected_root_tree: object.tree,
    };
    let activation = run_activation(bound, &config, info.commit.to_hex()).map_err(|error| {
        if matches!(error, GateError::License(_)) {
            DownloadError::LicenseChanged
        } else {
            DownloadError::Validation
        }
    })?;
    progress(DownloadPhase::Writing);
    let entry = StoreEntry {
        commit: info.commit,
        commit_object: object.raw,
        manifest: manifest_from(&info, &entries),
        files: materialized_subset(activation.vfs.archive()),
    };
    store::write_entry(&store_root, &entry).map_err(|_error| DownloadError::Store)?;
    tracing::info!(
        commit = %info.commit,
        tree = %object.tree,
        "typeshed commit downloaded and stored"
    );
    Ok(DownloadOutcome {
        commit: info.commit,
        tree: object.tree,
    })
}

/// Coarse progress for a `PyPI`-package wheel download, for surfaces that
/// render it on the invoking control ([STUBRES-TYPESHED-PYPI]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageDownloadPhase {
    /// Resolving the project index and selecting the pinned wheel.
    Resolving,
    /// Re-hashing the fetched bytes and running the structural gates.
    Verifying,
    /// Writing the store entry.
    Writing,
}

/// Download one `PyPI` typeshed distribution wheel into the store
/// ([STUBRES-TYPESHED-PYPI], [STUBRES-TYPESHED-DOWNLOAD]). `name` and
/// `sha256` are the configured pin (`name@sha256:<hex>`). The wheel is
/// resolved from `PyPI`'s JSON index by the pinned digest, **re-hashed** on
/// arrival (the index's reported digest is never trusted on its own), run
/// through the same Safety and Shape gates the check-time read path runs, and
/// written to `<store>/<sha256>/wheel.whl`. **A failure at any step writes
/// nothing.** No configuration is written — the pin is the caller's contract.
///
/// The License and Content gates are intentionally not run: the License gate
/// attests the build-approved *typeshed* `LICENSE` identity (a third-party
/// wheel ships its own license), and the Content gate reconstructs a Git root
/// tree (a wheel is not a Git tree). The wheel SHA-256 IS the content
/// attestation for a `PyPI` source.
///
/// # Errors
///
/// Returns a redacted [`DownloadError`]; on any error nothing was written.
pub fn download_package(
    name: &str,
    sha256: &str,
    store_path: Option<PathBuf>,
    api: &dyn PypiApi,
    progress: &dyn Fn(PackageDownloadPhase),
) -> Result<(), DownloadError> {
    let store_root = store_path
        .or_else(default_store_path)
        .ok_or(DownloadError::Store)?;
    progress(PackageDownloadPhase::Resolving);
    let bytes = api
        .fetch_wheel(name, sha256)
        .map_err(|_error| DownloadError::Download)?;
    progress(PackageDownloadPhase::Verifying);
    // The wheel SHA-256 IS the pin: re-hash the fetched bytes and require the
    // pinned digest. PyPI's index digest is never the basis of trust.
    if sha256_hex(&bytes) != sha256 {
        return Err(DownloadError::Validation);
    }
    // Defense in depth: decode and run the same structural gates the
    // check-time read path (`wheel::read_snapshot`) runs, so a malformed wheel
    // never lands in the store to fail later as `NO SOURCE`.
    let archive = decode_zip(&bytes, ZipLayout::BundledRootless, &DecodeLimits::default())
        .map_err(|_error| DownloadError::Validation)?;
    safety_gate(&archive, &SafetyLimits::default()).map_err(|_error| DownloadError::Validation)?;
    shape_gate(&archive).map_err(|_error| DownloadError::Validation)?;
    progress(PackageDownloadPhase::Writing);
    wheel::write_wheel(&store_root, sha256, &bytes).map_err(|_error| DownloadError::Store)?;
    tracing::info!(name, sha256, "typeshed package wheel downloaded and stored");
    Ok(())
}

/// Bind the decoded archive to the trusted tree: exact path set, trusted
/// modes, and per-blob object IDs. Codeload archives do not preserve Git
/// modes, so the tree's modes are authoritative
/// ([STUBRES-TYPESHED-DOWNLOAD] Content gate input).
fn bind_to_tree(archive: &Archive, trusted: &[TreeEntry]) -> Result<Archive, DownloadError> {
    if archive.len() != trusted.len() {
        return Err(DownloadError::Validation);
    }
    let by_path: std::collections::BTreeMap<&str, &TreeEntry> = trusted
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    if by_path.len() != trusted.len() {
        return Err(DownloadError::Validation);
    }
    let mut entries = Vec::with_capacity(archive.len());
    for entry in archive.entries() {
        let metadata = by_path
            .get(entry.path.as_ref())
            .ok_or(DownloadError::Validation)?;
        if !matches!(metadata.mode, FileMode::Regular | FileMode::Executable)
            || git_blob_oid(&entry.data) != metadata.oid
        {
            return Err(DownloadError::Validation);
        }
        entries.push(ArchiveEntry {
            path: entry.path.clone(),
            mode: metadata.mode,
            data: entry.data.clone(),
        });
    }
    Ok(Archive::new(entries))
}

fn manifest_from(info: &github::CommitInfo, entries: &[TreeEntry]) -> StoreManifest {
    StoreManifest {
        commit: info.commit.to_hex(),
        tree: info.tree.to_hex(),
        tree_files: entries
            .iter()
            .map(|entry| StoreTreeFile {
                path: entry.path.clone(),
                oid: entry.oid.to_hex(),
                mode: entry.mode.as_str().to_owned(),
            })
            .collect(),
    }
}

fn materialized_subset(archive: &Archive) -> Vec<ArchiveEntry> {
    archive
        .entries()
        .iter()
        .filter(|entry| is_materialized(&entry.path))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
