//! Implements [STUBRES-TYPESHED] source selection. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED
//!
//! The policy layer over local source backends. There are exactly two sources
//! and both fail closed: a custom folder never falls back, and a pinned commit
//! is served from the embedded bundle (when it IS that commit) or from the
//! local store — never from the network, which this crate cannot even reach
//! ([STUBRES-TYPESHED-OFFLINE]). A pin that is not on this machine is the
//! terminal `NO SOURCE` failure and analysis does not run.

use super::gittree::Oid;
use super::snapshot::Snapshot;
use super::source::{
    LicenseStatus, SourceIdentity, SourceKind, SourceSelection, StatusWarning, TypeshedRequest,
};
use super::warning::{TypeshedWarning, UnpinnedKind};

/// A redacted backend failure category.
///
/// No variant carries a path or adapter detail. This error is safe to expose
/// through MCP/LSP; detailed errors belong in redacted tracing at the backend
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BackendError {
    /// The configured source was malformed or unusable.
    #[error("invalid typeshed source configuration")]
    InvalidConfiguration,
    /// The pinned commit has no entry in the local store.
    #[error("commit is not in the local store")]
    Missing,
    /// A store entry exists but failed offline pin verification.
    #[error("store entry failed offline verification")]
    Corrupt,
    /// The approved legal-file identity drifted.
    #[error("typeshed license identity changed")]
    LicenseChanged,
    /// A custom source could not be read or indexed.
    #[error("custom typeshed source is unavailable")]
    Custom,
    /// The embedded bundle could not be activated.
    #[error("bundled typeshed snapshot is unavailable")]
    Bundle,
}

/// Local source adapter consumed by the policy selector.
///
/// Implementations must return only fully gated, immutable [`Snapshot`]s read
/// from this machine — there is no network seam to implement
/// ([STUBRES-TYPESHED-OFFLINE]).
pub trait SourceBackend: Send + Sync {
    /// Load and validate a user-managed custom tree.
    ///
    /// # Errors
    ///
    /// Returns a redacted failure when the tree cannot become a snapshot.
    fn load_custom(&self, path: &str) -> Result<Snapshot, BackendError>;

    /// Load and offline-verify one pinned commit from the local store
    /// ([STUBRES-TYPESHED-PIN]).
    ///
    /// # Errors
    ///
    /// Returns a redacted failure when no verified entry exists.
    fn load_pinned(&self, commit: Oid, explicit: bool) -> Result<Snapshot, BackendError>;

    /// Load and validate the embedded offline bundle.
    ///
    /// # Errors
    ///
    /// Returns a redacted failure when embedded assets fail their gates.
    fn load_bundled(&self) -> Result<Snapshot, BackendError>;
}

/// A terminal source-selection failure. Analysis does not run
/// ([STUBRES-TYPESHED-OFFLINE]): there is no substitute source and no
/// degraded untyped mode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    /// Custom is the sole step-3 source and could not activate.
    #[error("custom typeshed failed without fallback: {0}")]
    Custom(BackendError),
    /// The pinned commit is not on this machine (or failed verification).
    /// The message is the spec's `NO SOURCE` status line verbatim
    /// ([STUBRES-TYPESHED-WARN]).
    #[error("NO SOURCE — {commit} is not on this machine; run Download latest or basilisk typeshed download --commit {commit}")]
    NoSource {
        /// The full pinned commit SHA.
        commit: Oid,
        /// The redacted category (missing, corrupt, license drift…).
        reason: BackendError,
    },
    /// A backend returned a source different from the requested candidate.
    #[error("typeshed backend returned an inconsistent source identity")]
    InconsistentIdentity,
}

/// Select exactly one complete step-3 source under the configured policy.
///
/// # Errors
///
/// Returns a redacted [`SelectionError`] when the selected source does not
/// activate; nothing is ever substituted for it.
pub fn select_snapshot(
    request: &TypeshedRequest,
    backend: &dyn SourceBackend,
) -> Result<Snapshot, SelectionError> {
    match &request.selection {
        SourceSelection::Custom { path } => select_custom(path, backend),
        SourceSelection::Pinned { commit, explicit } => {
            select_pinned(*commit, *explicit, backend)
        }
    }
}

fn select_custom(path: &str, backend: &dyn SourceBackend) -> Result<Snapshot, SelectionError> {
    let mut snapshot = backend.load_custom(path).map_err(SelectionError::Custom)?;
    if !matches!(&snapshot.identity, SourceIdentity::Custom { .. })
        || !identity_matches_vfs(&snapshot)
    {
        return Err(SelectionError::InconsistentIdentity);
    }
    snapshot.status.active_source = SourceKind::Custom;
    snapshot.status.commit = None;
    snapshot.status.tree = None;
    snapshot.status.license_status = LicenseStatus::NotSupplied;
    snapshot.status.license_reference = None;
    set_warnings(
        &mut snapshot,
        &[
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
            TypeshedWarning::UserManaged,
        ],
    );
    Ok(snapshot)
}

fn select_pinned(
    commit: Oid,
    explicit: bool,
    backend: &dyn SourceBackend,
) -> Result<Snapshot, SelectionError> {
    // A pin naming the bundled commit is complete inside the binary: the
    // commit is content-addressed, so the embedded bytes ARE the pinned
    // source, and no store lookup is needed ([STUBRES-TYPESHED]).
    if commit.to_hex() == super::bundle::bundled_commit_sha() {
        return backend
            .load_bundled()
            .map_err(|reason| SelectionError::NoSource { commit, reason })
            .and_then(|bundle| pinned_bundle(bundle, commit, explicit));
    }
    match backend.load_pinned(commit, explicit) {
        Ok(snapshot) => {
            if !matches!(
                &snapshot.identity,
                SourceIdentity::Commit { commit: actual, pinned } if *actual == commit && *pinned == explicit
            ) || !identity_matches_vfs(&snapshot)
                || snapshot.status.active_source != SourceKind::ExactCommit
                || snapshot.status.commit != Some(commit)
            {
                return Err(SelectionError::InconsistentIdentity);
            }
            Ok(snapshot)
        }
        // The bundle cannot satisfy this pin and there is no network to reach:
        // the pin fails closed as NO SOURCE ([STUBRES-TYPESHED-OFFLINE]).
        Err(reason) => Err(SelectionError::NoSource { commit, reason }),
    }
}

/// The embedded bundle serving a pin of exactly its own commit. An explicit
/// pin is deterministic and suppresses `UNPINNED`; the bundled default keeps
/// it ([STUBRES-TYPESHED-WARN]).
fn pinned_bundle(
    mut bundle: Snapshot,
    commit: Oid,
    explicit: bool,
) -> Result<Snapshot, SelectionError> {
    if !matches!(&bundle.identity, SourceIdentity::Bundled { commit: actual } if *actual == commit)
        || !identity_matches_vfs(&bundle)
    {
        return Err(SelectionError::InconsistentIdentity);
    }
    bundle.status.active_source = SourceKind::Bundled;
    bundle.status.commit = Some(commit);
    let warnings: &[TypeshedWarning] = if explicit {
        &[]
    } else {
        &[TypeshedWarning::Unpinned(UnpinnedKind::BundledDefault)]
    };
    set_warnings(&mut bundle, warnings);
    Ok(bundle)
}

fn set_warnings(snapshot: &mut Snapshot, warnings: &[TypeshedWarning]) {
    snapshot.status.warnings = StatusWarning::list(warnings);
}

fn identity_matches_vfs(snapshot: &Snapshot) -> bool {
    snapshot.vfs.identity() == snapshot.identity.uri_component()
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only fixtures use fixed embedded assets and SHA constants"
)]
mod tests;
