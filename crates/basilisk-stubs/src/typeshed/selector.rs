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
    /// A PyPI package source is not installed or failed SHA-256 verification
    /// ([STUBRES-TYPESHED-PYPI], issue #312).
    #[error("pypi typeshed package is unavailable or failed verification")]
    PyPIPackage,
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

    /// Load and offline-verify a PyPI-distributed typeshed package,
    /// content-addressed by the distribution's SHA-256
    /// ([STUBRES-TYPESHED-PYPI], issue #312).
    ///
    /// Implementations must verify the on-disk contents match `sha256` and
    /// return a [`Snapshot`] whose identity is
    /// [`SourceIdentity::PyPIPackage`] with the same `name` and `sha256`.
    ///
    /// # Errors
    ///
    /// Returns a redacted failure when the package is not installed or its
    /// contents fail SHA-256 verification.
    fn load_pypi_package(
        &self,
        name: &str,
        sha256: &str,
    ) -> Result<Snapshot, BackendError>;
}

/// A terminal source-selection failure. Analysis does not run
/// ([STUBRES-TYPESHED-OFFLINE]): there is no substitute source and no
/// degraded untyped mode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    /// Custom is the sole step-3 source and could not activate.
    #[error("custom typeshed failed without fallback: {0}")]
    Custom(BackendError),
    /// A PyPI package source is not installed or failed SHA-256 verification
    /// ([STUBRES-TYPESHED-PYPI], issue #312).
    #[error("pypi typeshed package failed without fallback: {0}")]
    PyPIPackage(BackendError),
    /// The pinned commit is not on this machine, or the entry that IS on this
    /// machine failed a gate. The message is the matching spec status line
    /// ([STUBRES-TYPESHED-WARN]) — the two persistent statuses stay distinct,
    /// see [`terminal_status_line`].
    #[error("{}", terminal_status_line(.commit, *.reason))]
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

/// The spec's persistent status line for a terminal pinned-source failure
/// ([STUBRES-TYPESHED-WARN] lists the two as separate rows). Drift of the
/// build-approved license identity is its own status: those bytes ARE on this
/// machine and downloading them again changes nothing, so it must never
/// masquerade as `NO SOURCE`. The full SHA rides along either way — every
/// surface shows it when it is known.
fn terminal_status_line(commit: &Oid, reason: BackendError) -> String {
    match reason {
        BackendError::LicenseChanged => format!(
            "{} (commit {commit})",
            TypeshedWarning::LicenseChanged.message()
        ),
        BackendError::InvalidConfiguration
        | BackendError::Missing
        | BackendError::Corrupt
        | BackendError::Custom
        | BackendError::PyPIPackage
        | BackendError::Bundle => format!(
            "NO SOURCE — {commit} is not on this machine; run Download latest or basilisk typeshed download --commit {commit}"
        ),
    }
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
        SourceSelection::Pinned { commit, explicit } => select_pinned(*commit, *explicit, backend),
        SourceSelection::PyPIPackage { name, sha256 } => {
            select_pypi_package(name, sha256, backend)
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
/// pin is deterministic and suppresses `typeshed_source_unpinned`; the bundled default keeps
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

/// Select a SHA-256-addressed PyPI package as a PINNED source
/// ([STUBRES-TYPESHED-PYPI], issue #312). The registry attests the contents by
/// hash, so — unlike a custom folder — the source is neither `unpinned` nor
/// `user-managed`: it emits no source-status advisories. The backend is the
/// sole authority on whether the package is installed and whether its bytes
/// match the pin; this policy layer only validates the returned identity and
/// clears the advisory list.
fn select_pypi_package(
    name: &str,
    sha256: &str,
    backend: &dyn SourceBackend,
) -> Result<Snapshot, SelectionError> {
    let mut snapshot = backend
        .load_pypi_package(name, sha256)
        .map_err(SelectionError::PyPIPackage)?;
    if !matches!(
        &snapshot.identity,
        SourceIdentity::PyPIPackage {
            name: actual_name,
            sha256: actual_sha256,
        } if actual_name == name && actual_sha256 == sha256
    ) || !identity_matches_vfs(&snapshot)
        || snapshot.status.active_source != SourceKind::PyPIPackage
    {
        return Err(SelectionError::InconsistentIdentity);
    }
    // A content-addressed PyPI package is pinned: no `unpinned`, no
    // `user-managed` advisory.
    set_warnings(&mut snapshot, &[]);
    Ok(snapshot)
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

/// [STUBRES-TYPESHED-WARN]: the status table's two terminal rows — license
/// drift and an absent/unverifiable pin — never collapse into one message.
#[cfg(test)]
mod status_line_tests {
    use super::{terminal_status_line, BackendError, Oid, SelectionError};

    const SHA: &str = "0123456789012345678901234567890123456789";

    #[test]
    fn license_drift_reports_its_own_status_line_never_no_source() {
        let rendered = Oid::from_hex(SHA).map(|commit| {
            SelectionError::NoSource {
                commit,
                reason: BackendError::LicenseChanged,
            }
            .to_string()
        });
        assert_eq!(
            rendered.ok(),
            Some(format!(
                "the bundled typeshed's approved LICENSE/NOTICE changed and needs \
                 review; update Basilisk before relying on these stubs (commit {SHA})"
            )),
            "license drift is the spec's typeshed_source_license_changed status, not NO SOURCE"
        );
    }

    #[test]
    fn every_other_category_reports_the_no_source_recovery_line() {
        for reason in [
            BackendError::Missing,
            BackendError::Corrupt,
            BackendError::InvalidConfiguration,
            BackendError::Custom,
            BackendError::Bundle,
        ] {
            let rendered = Oid::from_hex(SHA).map(|commit| terminal_status_line(&commit, reason));
            assert_eq!(
                rendered.ok(),
                Some(format!(
                    "NO SOURCE — {SHA} is not on this machine; run Download latest or basilisk typeshed download --commit {SHA}"
                )),
                "{reason:?} must keep the loud NO SOURCE recovery line"
            );
        }
    }
}
