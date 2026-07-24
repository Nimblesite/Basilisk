//! Server-described Typeshed controls and typed lifecycle projection.

use basilisk_config::BasiliskConfig;
use basilisk_stubs::typeshed::bundle::bundled_commit_sha;
use basilisk_stubs::typeshed::source::{LicenseStatus, SourceKind, TypeshedStatus};
use basilisk_stubs::typeshed::warning::WarningSeverity;

use super::model::{
    TypeshedActiveSource, TypeshedConfigurationState, TypeshedLicenseStatus, TypeshedLifecycle,
    TypeshedSource, TypeshedStatusState, TypeshedWarningSeverity, TypeshedWarningState,
};
use crate::server::typeshed_status::TypeshedGeneration;

/// Project the one active source, carrying the value that defines it. An
/// unset pin IS the bundled commit ([STUBRES-TYPESHED]): the picker shows the
/// effective SHA and the `typeshed_source_unpinned` warning says it is not yet reproducible.
fn source(config: &BasiliskConfig) -> TypeshedSource {
    config.typeshed_path.as_ref().map_or_else(
        || TypeshedSource::ExactCommit {
            commit: config
                .typeshed_commit
                .clone()
                .unwrap_or_else(|| bundled_commit_sha().to_owned()),
        },
        |path| TypeshedSource::CustomFolder {
            path: path.to_string_lossy().into_owned(),
        },
    )
}

/// The store directory pins resolve from. A custom folder resolves nothing
/// from the store, so it has none ([STUBRES-TYPESHED-STORE]).
fn store_folder(config: &BasiliskConfig, source: &TypeshedSource) -> Option<String> {
    if matches!(source, TypeshedSource::CustomFolder { .. }) {
        return None;
    }
    config
        .typeshed_store_path
        .clone()
        .or_else(basilisk_stubs::typeshed::runtime::default_store_path)
        .map(|path| path.to_string_lossy().into_owned())
}

fn license_available(source: &TypeshedSource, generation: Option<&TypeshedGeneration>) -> bool {
    match source {
        // A custom folder always answers `ViewLicense` — with "not supplied".
        TypeshedSource::CustomFolder { .. } => true,
        TypeshedSource::ExactCommit { commit } => generation
            .and_then(TypeshedGeneration::ready_status)
            .and_then(|status| status.commit)
            .is_some_and(|active| active.to_hex() == *commit),
    }
}

pub(super) fn typeshed_configuration(
    config: &BasiliskConfig,
    generation: Option<&TypeshedGeneration>,
) -> TypeshedConfigurationState {
    let source = source(config);
    let status = generation.map_or_else(
        || TypeshedStatusState {
            lifecycle: TypeshedLifecycle::NoSource,
            no_source_reason: Some("typeshed resolution has not run for this root".to_owned()),
            active_source: None,
            commit_identity: None,
            license_status: TypeshedLicenseStatus::Unavailable,
            warnings: Vec::new(),
        },
        TypeshedGeneration::status_state,
    );
    TypeshedConfigurationState {
        store_folder: store_folder(config, &source),
        license_available: license_available(&source, generation),
        source,
        status,
    }
}

/// Project an active snapshot's status. A snapshot only exists after every
/// activation gate passed, so its lifecycle is always `Ready` — a blocked or
/// missing source is a [`TypeshedGeneration::NoSource`], never a snapshot.
pub(crate) fn ready_projection(status: &TypeshedStatus) -> TypeshedStatusState {
    TypeshedStatusState {
        lifecycle: TypeshedLifecycle::Ready,
        no_source_reason: None,
        active_source: Some(match status.active_source {
            SourceKind::Custom => TypeshedActiveSource::Custom,
            SourceKind::ExactCommit => TypeshedActiveSource::ExactCommit,
            SourceKind::Bundled => TypeshedActiveSource::Bundled,
        }),
        commit_identity: status.commit.map(|oid| oid.to_hex()),
        license_status: match status.license_status {
            LicenseStatus::Approved => TypeshedLicenseStatus::Approved,
            LicenseStatus::Changed => TypeshedLicenseStatus::Changed,
            LicenseStatus::NotSupplied => TypeshedLicenseStatus::NotSupplied,
        },
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
