//! Server-described Typeshed controls and typed lifecycle projection.

use basilisk_config::BasiliskConfig;
use basilisk_stubs::typeshed::source::{
    LicenseStatus, Provenance, SourceKind, Transport, TypeshedStatus,
};
use basilisk_stubs::typeshed::warning::WarningSeverity;

use super::model::{
    TypeshedAction, TypeshedActionState, TypeshedActiveSource, TypeshedConfigurationState,
    TypeshedLicenseStatus, TypeshedLifecycle, TypeshedProvenance, TypeshedSettingKey,
    TypeshedSettingState, TypeshedSettingValue, TypeshedSourceMode, TypeshedSourceOption,
    TypeshedStatusState, TypeshedTransport, TypeshedWarningSeverity, TypeshedWarningState,
    TypeshedWidget,
};
use crate::server::typeshed_status::TypeshedGeneration;

fn text_value(value: Option<String>) -> Option<TypeshedSettingValue> {
    value.map(|value| TypeshedSettingValue::Text { value })
}

fn bool_value(value: bool) -> TypeshedSettingValue {
    TypeshedSettingValue::Boolean { value }
}

fn setting(
    key: TypeshedSettingKey,
    label: &str,
    description: &str,
    value: Option<TypeshedSettingValue>,
    default_value: Option<TypeshedSettingValue>,
    widget: TypeshedWidget,
    enabled: bool,
) -> TypeshedSettingState {
    TypeshedSettingState {
        key,
        label: label.to_owned(),
        description: description.to_owned(),
        value,
        default_value,
        widget,
        enabled,
    }
}

fn source_mode(config: &BasiliskConfig) -> TypeshedSourceMode {
    if config.typeshed_path.is_some() {
        TypeshedSourceMode::CustomFolder
    } else if config.typeshed_commit.is_some() {
        TypeshedSourceMode::ExactCommit
    } else {
        TypeshedSourceMode::Latest
    }
}

fn typeshed_settings(
    config: &BasiliskConfig,
    mode: TypeshedSourceMode,
    download_enabled: bool,
    controls_enabled: bool,
) -> Vec<TypeshedSettingState> {
    vec![
        setting(
            TypeshedSettingKey::TypeshedPath,
            "Custom folder",
            "Canonical user-managed stdlib tree containing stdlib/.",
            text_value(
                config
                    .typeshed_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
            ),
            None,
            TypeshedWidget::Directory,
            controls_enabled && mode == TypeshedSourceMode::CustomFolder,
        ),
        setting(
            TypeshedSettingKey::TypeshedCommit,
            "Exact commit",
            "Full 40-character python/typeshed commit SHA.",
            text_value(config.typeshed_commit.clone()),
            None,
            TypeshedWidget::Text,
            controls_enabled && mode == TypeshedSourceMode::ExactCommit,
        ),
        setting(
            TypeshedSettingKey::TypeshedUrl,
            "Alternate archive URL",
            "HTTPS archive mirror template containing exactly one {sha}.",
            text_value(config.typeshed_url.clone()),
            None,
            TypeshedWidget::Text,
            controls_enabled && download_enabled,
        ),
        setting(
            TypeshedSettingKey::TypeshedCachePath,
            "Cache folder",
            "Directory for immutable, gate-accepted Typeshed ZIPs.",
            text_value(
                config
                    .typeshed_cache_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
            ),
            None,
            TypeshedWidget::Directory,
            controls_enabled && download_enabled,
        ),
        setting(
            TypeshedSettingKey::TypeshedCache,
            "Reuse downloads",
            "Reuse gate-accepted downloads; off validates and discards.",
            config.typeshed_cache.map(bool_value),
            Some(bool_value(true)),
            TypeshedWidget::Boolean,
            controls_enabled && download_enabled,
        ),
        setting(
            TypeshedSettingKey::TypeshedVerify,
            "Verify content",
            "Attest content to the selected Git tree; safety and license gates always run.",
            config.typeshed_verify.map(bool_value),
            Some(bool_value(true)),
            TypeshedWidget::Boolean,
            controls_enabled && download_enabled,
        ),
    ]
}

fn view_license_enabled(
    config: &BasiliskConfig,
    mode: TypeshedSourceMode,
    lifecycle: TypeshedLifecycle,
    status: Option<&TypeshedStatus>,
) -> bool {
    if mode == TypeshedSourceMode::CustomFolder {
        return lifecycle != TypeshedLifecycle::Acquiring;
    }
    lifecycle == TypeshedLifecycle::Ready
        && status.is_some_and(|current| match mode {
            TypeshedSourceMode::ExactCommit => {
                config.typeshed_commit.as_deref().is_some_and(|commit| {
                    current
                        .commit
                        .is_some_and(|active| active.to_hex() == commit)
                })
            }
            TypeshedSourceMode::Latest => matches!(
                current.active_source,
                SourceKind::Latest | SourceKind::Bundled
            ),
            TypeshedSourceMode::CustomFolder => false,
        })
}

pub(super) fn typeshed_configuration(
    config: &BasiliskConfig,
    generation: Option<&TypeshedGeneration>,
) -> TypeshedConfigurationState {
    let mode = source_mode(config);
    let download_enabled = mode != TypeshedSourceMode::CustomFolder;
    let status = generation.and_then(TypeshedGeneration::ready_status);
    let status_state =
        generation.map_or_else(|| status_projection(None), TypeshedGeneration::status_state);
    let acquiring = status_state.lifecycle == TypeshedLifecycle::Acquiring;
    let settings = typeshed_settings(config, mode, download_enabled, !acquiring);
    TypeshedConfigurationState {
        source_mode: mode,
        source_options: vec![
            source_option(TypeshedSourceMode::Latest, "Latest", !acquiring),
            source_option(TypeshedSourceMode::ExactCommit, "Exact commit", !acquiring),
            source_option(
                TypeshedSourceMode::CustomFolder,
                "Custom folder",
                !acquiring,
            ),
        ],
        settings,
        actions: vec![
            TypeshedActionState {
                action: TypeshedAction::PinCurrent,
                label: "Pin current".to_owned(),
                enabled: !acquiring
                    && mode == TypeshedSourceMode::Latest
                    && status.and_then(|current| current.commit).is_some(),
            },
            TypeshedActionState {
                action: TypeshedAction::AcquireFresh,
                label: "Acquire fresh".to_owned(),
                enabled: !acquiring,
            },
            TypeshedActionState {
                action: TypeshedAction::ViewLicense,
                label: "View License".to_owned(),
                enabled: view_license_enabled(config, mode, status_state.lifecycle, status),
            },
        ],
        status: status_state,
    }
}

fn source_option(mode: TypeshedSourceMode, label: &str, enabled: bool) -> TypeshedSourceOption {
    TypeshedSourceOption {
        mode,
        label: label.to_owned(),
        enabled,
    }
}

pub(crate) fn status_projection(status: Option<&TypeshedStatus>) -> TypeshedStatusState {
    let Some(status) = status else {
        return TypeshedStatusState {
            lifecycle: TypeshedLifecycle::Acquiring,
            blocked_reason: None,
            active_source: None,
            commit_identity: None,
            tree_identity: None,
            transport: None,
            license_status: TypeshedLicenseStatus::Acquiring,
            license_reference: None,
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
        tree_identity: status.tree.map(|oid| oid.to_hex()),
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
        license_reference: status.license_reference.clone(),
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
