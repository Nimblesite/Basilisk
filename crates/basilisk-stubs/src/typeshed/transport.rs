//! Implements [STUBRES-TYPESHED-ACQUIRE] transport. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE
//!
//! The injectable archive transport seam.
//!
//! Basilisk **never clones** ([STUBRES-TYPESHED-ACQUIRE]): a transport resolves
//! trusted commit→tree metadata and downloads the commit's archive over HTTPS.
//! This trait is the boundary between the acquisition backend and the network,
//! so the whole selection/gate pipeline is testable with a mock. The **trusted
//! recursive tree is the attestation authority** for per-blob object IDs and
//! modes — content is bound to it, not to the archive's self-reported metadata.

mod http;

use super::gittree::{FileMode, Oid};
use super::source::Transport as SourceTransport;

pub use http::HttpsTransport;

/// Trusted commit→tree metadata, resolved from the GitHub API over TLS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitMetadata {
    /// The full commit SHA.
    pub commit: Oid,
    /// The commit's root-tree SHA.
    pub tree: Oid,
}

/// One trusted recursive-tree entry: a repo-relative path, its blob object ID,
/// and its Git mode. These trusted modes and OIDs drive content attestation
/// because codeload archives do not preserve them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// Repo-relative path.
    pub path: String,
    /// The blob (or submodule commit) object ID.
    pub oid: Oid,
    /// The Git file mode.
    pub mode: FileMode,
}

/// A transport failure. Detailed URLs and credentials are redacted before this
/// crosses the acquisition boundary (the selector maps it to a redacted
/// `BackendError`; raw detail belongs only in redacted tracing).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    /// Commit or `main` metadata could not be resolved.
    #[error("metadata resolution failed")]
    Metadata,
    /// The archive could not be downloaded.
    #[error("archive download failed")]
    Download,
    /// A configured mirror URL was not an authenticated HTTPS `{sha}` template.
    #[error("invalid archive mirror configuration")]
    InvalidMirror,
}

/// An injectable archive transport.
///
/// Implementations MUST use HTTPS and MUST NEVER invoke `git`, `git clone`, or
/// any subprocess. The production adapter always resolves commit/tree metadata
/// from the official GitHub API; a configured `{sha}` mirror replaces only the
/// archive-byte request, so Latest remains bound to current official metadata.
///
/// [`fetch_archive`]: Transport::fetch_archive
/// [`resolve_latest`]: Transport::resolve_latest
pub trait Transport: Send + Sync + std::fmt::Debug {
    /// Resolve `main` to its current commit and root-tree.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when official metadata cannot be resolved.
    fn resolve_latest(&self) -> Result<CommitMetadata, TransportError>;

    /// Resolve one explicit commit to its trusted root tree.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when official metadata cannot bind that
    /// exact commit to a root tree.
    fn resolve_commit(&self, commit: Oid) -> Result<CommitMetadata, TransportError>;

    /// Fetch the trusted recursive tree for a commit (path → blob OID + mode).
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the tree metadata cannot be fetched.
    fn fetch_tree(&self, root_tree: Oid) -> Result<Vec<TreeEntry>, TransportError>;

    /// Fetch a commit's archive (zipball) bytes.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the archive cannot be downloaded.
    fn fetch_archive(&self, commit: Oid) -> Result<Vec<u8>, TransportError>;

    /// Report the actual archive byte origin. Cache metadata persists this
    /// value so a later configuration change cannot relabel cached bytes.
    fn archive_transport(&self) -> SourceTransport;
}
