//! Implements [STUBRES-TYPESHED] source selection. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED
//!
//! The policy layer is deliberately separate from HTTP/cache mechanics. A
//! backend can fetch and gate candidates, while this module enforces the three
//! non-negotiable failure rules: Custom never falls back, Exact only accepts
//! the requested commit (including a matching bundle), and Latest falls back
//! to the bundle rather than reusing an older unpinned cache entry.

use super::gittree::Oid;
use super::snapshot::Snapshot;
use super::source::{
    LicenseStatus, Provenance, SourceIdentity, SourceKind, SourceSelection, StatusWarning,
    Transport, TypeshedRequest,
};
use super::warning::{TypeshedWarning, UnpinnedKind};

/// A redacted backend failure category.
///
/// No variant carries an arbitrary URL or transport error string. This error
/// is safe to expose through MCP/LSP; detailed adapter errors belong in
/// redacted tracing at the transport boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BackendError {
    /// The configured source was malformed or unusable.
    #[error("invalid typeshed source configuration")]
    InvalidConfiguration,
    /// Official `main` or commit metadata could not be resolved.
    #[error("typeshed metadata resolution failed")]
    Metadata,
    /// Archive download failed.
    #[error("typeshed archive download failed")]
    Download,
    /// Cached bytes or cache I/O failed.
    #[error("typeshed cache operation failed")]
    Cache,
    /// Safety, shape, or content verification rejected the candidate.
    #[error("typeshed archive validation failed")]
    Validation,
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

/// Transport/cache adapter consumed by the policy selector.
///
/// Implementations must return only fully gated, immutable [`Snapshot`]s. In
/// particular, `load_latest` resolves `main` once and must never substitute an
/// older cached commit; only [`select_snapshot`] is allowed to choose a bundle
/// after that operation fails.
pub trait AcquisitionBackend: Send + Sync {
    /// Load and validate a user-managed custom tree.
    ///
    /// # Errors
    ///
    /// Returns a redacted failure when the tree cannot become a snapshot.
    fn load_custom(&self, path: &str) -> Result<Snapshot, BackendError>;

    /// Load and validate one exact commit from cache or transport.
    ///
    /// # Errors
    ///
    /// Returns a redacted failure when the selected commit cannot activate.
    fn load_commit(&self, commit: Oid, request: &TypeshedRequest)
        -> Result<Snapshot, BackendError>;

    /// Resolve `main` once, then load and validate that exact resolved commit.
    ///
    /// # Errors
    ///
    /// Returns a redacted failure when metadata or candidate activation fails.
    fn load_latest(&self, request: &TypeshedRequest) -> Result<Snapshot, BackendError>;

    /// Load and validate the embedded offline bundle.
    ///
    /// # Errors
    ///
    /// Returns a redacted failure when embedded assets fail their gates.
    fn load_bundled(&self) -> Result<Snapshot, BackendError>;
}

/// A terminal source-selection failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    /// Custom is the sole step-3 source and could not activate.
    #[error("custom typeshed failed without fallback: {0}")]
    Custom(BackendError),
    /// Neither the exact commit nor an equal bundled commit was available.
    #[error("exact typeshed commit {commit} is unavailable: {reason}")]
    Exact {
        /// The full requested commit SHA.
        commit: Oid,
        /// The redacted primary failure.
        reason: BackendError,
    },
    /// Latest and the offline bundle both failed.
    #[error("latest typeshed and bundled fallback are unavailable ({latest}; {bundle})")]
    LatestAndBundle {
        /// The redacted Latest failure.
        latest: BackendError,
        /// The redacted bundle failure.
        bundle: BackendError,
    },
    /// A backend returned a source different from the requested candidate.
    #[error("typeshed backend returned an inconsistent source identity")]
    InconsistentIdentity,
}

/// Select exactly one complete step-3 source under the configured policy.
///
/// # Errors
///
/// Returns a redacted [`SelectionError`] when no eligible source activates.
pub fn select_snapshot(
    request: &TypeshedRequest,
    backend: &dyn AcquisitionBackend,
) -> Result<Snapshot, SelectionError> {
    match &request.selection {
        SourceSelection::Custom { path } => select_custom(path, backend),
        SourceSelection::ExactCommit { commit } => select_exact(*commit, request, backend),
        SourceSelection::Latest => select_latest(request, backend),
    }
}

fn select_custom(path: &str, backend: &dyn AcquisitionBackend) -> Result<Snapshot, SelectionError> {
    let mut snapshot = backend.load_custom(path).map_err(SelectionError::Custom)?;
    if !matches!(&snapshot.identity, SourceIdentity::Custom { .. })
        || !identity_matches_vfs(&snapshot)
    {
        return Err(SelectionError::InconsistentIdentity);
    }
    snapshot.status.active_source = SourceKind::Custom;
    snapshot.status.commit = None;
    snapshot.status.tree = None;
    snapshot.status.transport = Transport::CustomPath;
    snapshot.status.license_status = LicenseStatus::NotSupplied;
    snapshot.status.license_reference = None;
    snapshot.status.provenance = Provenance::UserManaged;
    set_warnings(
        &mut snapshot,
        &[
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
            TypeshedWarning::UserManaged,
        ],
    );
    Ok(snapshot)
}

fn select_exact(
    commit: Oid,
    request: &TypeshedRequest,
    backend: &dyn AcquisitionBackend,
) -> Result<Snapshot, SelectionError> {
    match backend.load_commit(commit, request) {
        Ok(mut snapshot) => {
            if !matches!(
                &snapshot.identity,
                SourceIdentity::Commit { commit: actual, .. } if *actual == commit
            ) || !identity_matches_vfs(&snapshot)
            {
                return Err(SelectionError::InconsistentIdentity);
            }
            snapshot.status.active_source = SourceKind::ExactCommit;
            snapshot.status.commit = Some(commit);
            normalize_download_verification(&mut snapshot, request.verify_content, &[])?;
            Ok(snapshot)
        }
        Err(reason) => {
            let mut bundle = backend
                .load_bundled()
                .map_err(|_bundle_failure| SelectionError::Exact { commit, reason })?;
            if !matches!(&bundle.identity, SourceIdentity::Bundled { commit: actual } if *actual == commit)
                || !identity_matches_vfs(&bundle)
                || bundle.status.provenance != Provenance::BundleVetted
            {
                return Err(SelectionError::Exact { commit, reason });
            }
            // The user explicitly pinned this exact commit. A matching embedded
            // bundle is deterministic and therefore suppresses UNPINNED.
            bundle.status.active_source = SourceKind::Bundled;
            bundle.status.commit = Some(commit);
            bundle.status.transport = Transport::EmbeddedZip;
            set_warnings(&mut bundle, &[]);
            Ok(bundle)
        }
    }
}

fn select_latest(
    request: &TypeshedRequest,
    backend: &dyn AcquisitionBackend,
) -> Result<Snapshot, SelectionError> {
    match backend.load_latest(request) {
        Ok(mut snapshot) => {
            if !matches!(&snapshot.identity, SourceIdentity::Commit { .. })
                || !identity_matches_vfs(&snapshot)
            {
                return Err(SelectionError::InconsistentIdentity);
            }
            snapshot.status.active_source = SourceKind::Latest;
            snapshot.status.commit = snapshot.identity.commit();
            normalize_download_verification(
                &mut snapshot,
                request.verify_content,
                &[TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled)],
            )?;
            Ok(snapshot)
        }
        Err(latest) => {
            let mut bundle = backend
                .load_bundled()
                .map_err(|bundle| SelectionError::LatestAndBundle { latest, bundle })?;
            let Some(commit) = bundle.identity.commit() else {
                return Err(SelectionError::InconsistentIdentity);
            };
            if !matches!(&bundle.identity, SourceIdentity::Bundled { .. })
                || !identity_matches_vfs(&bundle)
                || bundle.status.provenance != Provenance::BundleVetted
            {
                return Err(SelectionError::InconsistentIdentity);
            }
            let mut warnings = vec![
                TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled),
                TypeshedWarning::DownloadFailed {
                    bundled_sha: commit.to_hex(),
                },
            ];
            if latest == BackendError::LicenseChanged {
                warnings.push(TypeshedWarning::LicenseChanged);
            }
            bundle.status.active_source = SourceKind::Bundled;
            bundle.status.commit = Some(commit);
            bundle.status.transport = Transport::EmbeddedZip;
            set_warnings(&mut bundle, &warnings);
            Ok(bundle)
        }
    }
}

fn normalize_download_verification(
    snapshot: &mut Snapshot,
    verify_content: bool,
    base: &[TypeshedWarning],
) -> Result<(), SelectionError> {
    let mut warnings = base.to_vec();
    if verify_content {
        // Policy must never manufacture attestation from a request flag. The
        // backend earns this state only after trusted commit→tree metadata and
        // the Content gate bind the exact VFS bytes to that tree.
        if snapshot.status.provenance != Provenance::GithubTlsAttested
            || snapshot.status.tree.is_none()
        {
            return Err(SelectionError::InconsistentIdentity);
        }
    } else {
        snapshot.status.tree = None;
        snapshot.status.provenance = Provenance::Unverified;
        warnings.push(TypeshedWarning::Unverified);
    }
    set_warnings(snapshot, &warnings);
    Ok(())
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
