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
    TypeshedSourceState, TypeshedStatusState, TypeshedTransport, TypeshedWarningSeverity,
    TypeshedWarningState, TypeshedWidget,
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

/// Project the one active source, carrying the value that defines it.
fn source_state(config: &BasiliskConfig) -> TypeshedSourceState {
    config.typeshed_path.as_ref().map_or_else(
        || {
            config.typeshed_commit.clone().map_or(
                TypeshedSourceState::Latest,
                |commit| TypeshedSourceState::ExactCommit { commit },
            )
        },
        |path| TypeshedSourceState::CustomFolder {
            path: path.to_string_lossy().into_owned(),
        },
    )
}

/// Download settings only. The source-defining values (`typeshed-path`,
/// `typeshed-commit`) travel in [`TypeshedSourceState`], and a user-managed
/// folder downloads nothing — so nothing inapplicable is ever described
/// ([LSPCFGED-TYPESHED]).
fn typeshed_settings(
    config: &BasiliskConfig,
    mode: TypeshedSourceMode,
    controls_enabled: bool,
) -> Vec<TypeshedSettingState> {
    if mode == TypeshedSourceMode::CustomFolder {
        return Vec::new();
    }
    vec![
        setting(
            TypeshedSettingKey::TypeshedCache,
            "Reuse downloads",
            "Reuse gate-accepted downloads; off validates and discards.",
            config.typeshed_cache.map(bool_value),
            Some(bool_value(true)),
            TypeshedWidget::Boolean,
            controls_enabled,
        ),
        setting(
            TypeshedSettingKey::TypeshedVerify,
            "Verify content",
            "Attest content to the selected Git tree; safety and license gates always run.",
            config.typeshed_verify.map(bool_value),
            Some(bool_value(true)),
            TypeshedWidget::Boolean,
            controls_enabled,
        ),
        setting(
            TypeshedSettingKey::TypeshedUrl,
            "Alternate archive URL",
            "HTTPS archive mirror template containing exactly one {sha}.",
            text_value(config.typeshed_url.clone()),
            None,
            TypeshedWidget::Text,
            controls_enabled,
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
            controls_enabled,
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
    let source = source_state(config);
    let mode = source.mode();
    let status = generation.and_then(TypeshedGeneration::ready_status);
    let status_state =
        generation.map_or_else(|| status_projection(None), TypeshedGeneration::status_state);
    let acquiring = status_state.lifecycle == TypeshedLifecycle::Acquiring;
    TypeshedConfigurationState {
        source,
        source_options: source_options(mode, acquiring, status.and_then(|current| current.commit)),
        settings: typeshed_settings(config, mode, !acquiring),
        actions: vec![
            TypeshedActionState {
                action: TypeshedAction::AcquireFresh,
                label: "Acquire fresh".to_owned(),
                enabled: !acquiring,
            },
            TypeshedActionState {
                action: TypeshedAction::ViewLicense,
                label: "View license".to_owned(),
                enabled: view_license_enabled(config, mode, status_state.lifecycle, status),
            },
        ],
        status: status_state,
    }
}

const ACQUIRING_REASON: &str = "A standard library is being acquired; the source is locked until it settles.";

/// Describe the three mutually exclusive sources. `ExactCommit` pins the
/// ACTIVE commit ([`TypeshedAction::PinCurrent`]), so it is offered only when
/// there is one to pin — a mode a user could select but never reach is worse
/// than a mode that explains itself.
fn source_options(
    mode: TypeshedSourceMode,
    acquiring: bool,
    active_commit: Option<basilisk_stubs::typeshed::gittree::Oid>,
) -> Vec<TypeshedSourceOption> {
    let lock = acquiring.then(|| ACQUIRING_REASON.to_owned());
    vec![
        source_option(
            TypeshedSourceMode::Latest,
            "Latest",
            "Track the newest python/typeshed commit on every acquisition.",
            lock.clone(),
        ),
        source_option(
            TypeshedSourceMode::ExactCommit,
            "Pinned commit",
            "Pin the active commit so every machine resolves the identical standard library.",
            lock.clone().or_else(|| pin_unavailable(mode, active_commit)),
        ),
        source_option(
            TypeshedSourceMode::CustomFolder,
            "Custom folder",
            "Use a canonical stdlib tree you manage yourself; nothing is downloaded.",
            lock,
        ),
    ]
}

/// Why pinning is not offered, or `None` when it is.
fn pin_unavailable(
    mode: TypeshedSourceMode,
    active_commit: Option<basilisk_stubs::typeshed::gittree::Oid>,
) -> Option<String> {
    match mode {
        TypeshedSourceMode::ExactCommit => None,
        TypeshedSourceMode::CustomFolder => Some(
            "A custom folder has no upstream commit to pin. Switch to Latest first.".to_owned(),
        ),
        TypeshedSourceMode::Latest if active_commit.is_some() => None,
        TypeshedSourceMode::Latest => {
            Some("Available once a downloaded standard library is active.".to_owned())
        }
    }
}

fn source_option(
    mode: TypeshedSourceMode,
    label: &str,
    description: &str,
    unavailable_reason: Option<String>,
) -> TypeshedSourceOption {
    TypeshedSourceOption {
        mode,
        label: label.to_owned(),
        description: description.to_owned(),
        enabled: unavailable_reason.is_none(),
        unavailable_reason,
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
