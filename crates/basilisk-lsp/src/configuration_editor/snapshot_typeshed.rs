//! Server-described Typeshed controls and typed lifecycle projection.

use basilisk_config::BasiliskConfig;
use basilisk_stubs::typeshed::source::{
    LicenseStatus, Provenance, SourceKind, Transport, TypeshedStatus,
};
use basilisk_stubs::typeshed::warning::WarningSeverity;

use super::model::{
    TypeshedActiveSource, TypeshedConfigurationState, TypeshedDownloadPolicy,
    TypeshedLicenseStatus, TypeshedLifecycle, TypeshedProvenance, TypeshedSource,
    TypeshedStatusState, TypeshedTransport, TypeshedWarningSeverity, TypeshedWarningState,
};
use crate::server::typeshed_status::TypeshedGeneration;

/// Project the one active source, carrying the value that defines it.
fn source(config: &BasiliskConfig) -> TypeshedSource {
    config.typeshed_path.as_ref().map_or_else(
        || {
            config
                .typeshed_commit
                .clone()
                .map_or(TypeshedSource::Latest, |commit| {
                    TypeshedSource::ExactCommit { commit }
                })
        },
        |path| TypeshedSource::CustomFolder {
            path: path.to_string_lossy().into_owned(),
        },
    )
}

/// The download policy of a downloaded source. A user-managed folder
/// downloads nothing, so it has none ([LSPCFGED-TYPESHED]).
fn downloads(config: &BasiliskConfig, source: &TypeshedSource) -> Option<TypeshedDownloadPolicy> {
    if matches!(source, TypeshedSource::CustomFolder { .. }) {
        return None;
    }
    Some(TypeshedDownloadPolicy {
        reuse_downloads: config.typeshed_cache.unwrap_or(true),
        verify_content: config.typeshed_verify.unwrap_or(true),
        archive_url: config.typeshed_url.clone(),
        cache_folder: config
            .typeshed_cache_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
    })
}

/// The commit `PinCurrent` would write, present only when pinning is possible:
/// the source must be an unpinned download and a gate-accepted commit must be
/// active. A source the user can select but never reach does not exist.
fn pinnable_commit(
    source: &TypeshedSource,
    acquiring: bool,
    status: Option<&TypeshedStatus>,
) -> Option<String> {
    if acquiring || !matches!(source, TypeshedSource::Latest) {
        return None;
    }
    status
        .and_then(|current| current.commit)
        .map(|commit| commit.to_hex())
}

fn license_available(
    config: &BasiliskConfig,
    source: &TypeshedSource,
    lifecycle: TypeshedLifecycle,
    status: Option<&TypeshedStatus>,
) -> bool {
    if matches!(source, TypeshedSource::CustomFolder { .. }) {
        return lifecycle != TypeshedLifecycle::Acquiring;
    }
    lifecycle == TypeshedLifecycle::Ready
        && status.is_some_and(|current| match source {
            TypeshedSource::ExactCommit { .. } => {
                config.typeshed_commit.as_deref().is_some_and(|commit| {
                    current
                        .commit
                        .is_some_and(|active| active.to_hex() == commit)
                })
            }
            TypeshedSource::Latest => matches!(
                current.active_source,
                SourceKind::Latest | SourceKind::Bundled
            ),
            TypeshedSource::CustomFolder { .. } => false,
        })
}

pub(super) fn typeshed_configuration(
    config: &BasiliskConfig,
    generation: Option<&TypeshedGeneration>,
) -> TypeshedConfigurationState {
    let source = source(config);
    let ready_status = generation.and_then(TypeshedGeneration::ready_status);
    let status =
        generation.map_or_else(|| status_projection(None), TypeshedGeneration::status_state);
    let acquiring = status.lifecycle == TypeshedLifecycle::Acquiring;
    TypeshedConfigurationState {
        downloads: downloads(config, &source),
        pinnable_commit: pinnable_commit(&source, acquiring, ready_status),
        license_available: license_available(config, &source, status.lifecycle, ready_status),
        source,
        status,
    }
}

pub(crate) fn status_projection(status: Option<&TypeshedStatus>) -> TypeshedStatusState {
    let Some(status) = status else {
        return TypeshedStatusState {
            lifecycle: TypeshedLifecycle::Acquiring,
            blocked_reason: None,
            active_source: None,
            commit_identity: None,
            transport: None,
            license_status: TypeshedLicenseStatus::Acquiring,
            provenance: TypeshedProvenance::Pending,
            signed_release: false,
            warnings: Vec::new(),
        };
    };
    TypeshedStatusState {
        lifecycle: if status.license_status == LicenseStatus::Changed {
            TypeshedLifecycle::Blocked
        } else {
            TypeshedLifecycle::Ready
        },
        blocked_reason: (status.license_status == LicenseStatus::Changed)
            .then(|| "Typeshed license identity changed; activation requires review".to_owned()),
        active_source: Some(match status.active_source {
            SourceKind::Custom => TypeshedActiveSource::Custom,
            SourceKind::ExactCommit => TypeshedActiveSource::ExactCommit,
            SourceKind::Latest => TypeshedActiveSource::Latest,
            SourceKind::Bundled => TypeshedActiveSource::Bundled,
        }),
        commit_identity: status.commit.map(|oid| oid.to_hex()),
        transport: Some(match status.transport {
            Transport::CustomPath => TypeshedTransport::CustomPath,
            Transport::EmbeddedZip => TypeshedTransport::EmbeddedZip,
            Transport::Codeload => TypeshedTransport::Codeload,
            Transport::Mirror => TypeshedTransport::Mirror,
        }),
        license_status: match status.license_status {
            LicenseStatus::Approved => TypeshedLicenseStatus::Approved,
            LicenseStatus::Changed => TypeshedLicenseStatus::Changed,
            LicenseStatus::NotSupplied => TypeshedLicenseStatus::NotSupplied,
        },
        provenance: match status.provenance {
            Provenance::GithubTlsAttested => TypeshedProvenance::GithubTlsAttested,
            Provenance::Unverified => TypeshedProvenance::Unverified,
            Provenance::BundleVetted => TypeshedProvenance::BundleVetted,
            Provenance::UserManaged => TypeshedProvenance::UserManaged,
        },
        signed_release: status.signed_release,
        warnings: status
            .warnings
            .iter()
            .map(|warning| TypeshedWarningState {
                code: warning.code.clone(),
                message: warning.message.clone(),
                severity: match warning.severity {
                    WarningSeverity::Advisory => TypeshedWarningSeverity::Advisory,
                    WarningSeverity::High => TypeshedWarningSeverity::High,
                },
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "snapshot_typeshed_tests.rs"]
mod tests;
