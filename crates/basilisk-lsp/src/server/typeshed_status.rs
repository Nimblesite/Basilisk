//! Implements [STUBRES-TYPESHED-WARN] LSP routing.
//!
//! Typeshed transport status is editor metadata. It is merged into the
//! initialize payload for persistent Service Info and elevated warnings are
//! sent with `window/showMessage`; none of it is a Python diagnostic.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use basilisk_stubs::typeshed::selector::{BackendError, SelectionError};
use basilisk_stubs::typeshed::snapshot::Snapshot;
use basilisk_stubs::typeshed::source::TypeshedStatus;
use basilisk_stubs::typeshed::warning::WarningSeverity;
use serde_json::{Map, Value};
use tower_lsp::lsp_types::notification::Notification;
use tower_lsp::lsp_types::{MessageType, Url};
use tower_lsp::Client;

use crate::configuration_editor::model::{
    TypeshedLicenseStatus, TypeshedLifecycle, TypeshedStatusChanged, TypeshedStatusState,
};
use crate::configuration_editor::snapshot_typeshed::status_projection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeshedFailureKind {
    LicenseChanged,
    CustomUnavailable,
    ExactUnavailable,
    LatestUnavailable,
    InconsistentIdentity,
    AcquisitionFailed,
}

/// One redacted terminal acquisition failure with the selection category kept
/// intact for typed RPC/status projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeshedFailure {
    kind: TypeshedFailureKind,
    reason: String,
}

impl TypeshedFailure {
    #[must_use]
    pub(crate) fn acquisition(reason: impl Into<String>) -> Self {
        Self {
            kind: TypeshedFailureKind::AcquisitionFailed,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub(crate) fn from_selection(error: &SelectionError) -> Self {
        let kind = match error {
            // Custom sources provide user-managed terms, so they never project
            // the official Typeshed license as changed.
            SelectionError::Custom(_) => TypeshedFailureKind::CustomUnavailable,
            SelectionError::Exact { reason, .. } if *reason == BackendError::LicenseChanged => {
                TypeshedFailureKind::LicenseChanged
            }
            SelectionError::Exact { .. } => TypeshedFailureKind::ExactUnavailable,
            SelectionError::LatestAndBundle { latest, bundle }
                if *latest == BackendError::LicenseChanged
                    || *bundle == BackendError::LicenseChanged =>
            {
                TypeshedFailureKind::LicenseChanged
            }
            SelectionError::LatestAndBundle { .. } => TypeshedFailureKind::LatestUnavailable,
            SelectionError::InconsistentIdentity => TypeshedFailureKind::InconsistentIdentity,
        };
        Self {
            kind,
            reason: error.to_string(),
        }
    }

    #[must_use]
    pub(crate) const fn rpc_code(&self) -> &'static str {
        match self.kind {
            TypeshedFailureKind::LicenseChanged => "typeshedLicenseChanged",
            TypeshedFailureKind::CustomUnavailable => "typeshedCustomUnavailable",
            TypeshedFailureKind::ExactUnavailable => "typeshedExactUnavailable",
            TypeshedFailureKind::LatestUnavailable => "typeshedLatestUnavailable",
            TypeshedFailureKind::InconsistentIdentity => "typeshedInconsistentIdentity",
            TypeshedFailureKind::AcquisitionFailed => "typeshedAcquisitionFailed",
        }
    }

    #[must_use]
    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }

    const fn license_status(&self) -> TypeshedLicenseStatus {
        if matches!(self.kind, TypeshedFailureKind::LicenseChanged) {
            TypeshedLicenseStatus::Changed
        } else {
            TypeshedLicenseStatus::Unavailable
        }
    }
}

/// One workspace root's acquisition generation.
///
/// A candidate is never exposed as Ready until every activation gate passes.
/// Reacquisition replaces this value atomically; in-flight requests may keep
/// their old [`Arc<Snapshot>`], while new analysis observes only the terminal
/// generation selected for its root.
#[derive(Debug, Clone)]
pub(crate) enum TypeshedGeneration {
    /// Acquisition is in progress; analysis for this root must not start.
    Acquiring,
    /// One complete immutable source is active.
    Ready(Arc<Snapshot>),
    /// No candidate activated. The reason is redacted and safe for UI/MCP.
    Blocked { failure: TypeshedFailure },
}

impl TypeshedGeneration {
    /// The active immutable snapshot, only in the Ready state.
    #[must_use]
    pub(crate) fn ready_snapshot(&self) -> Option<&Arc<Snapshot>> {
        match self {
            Self::Ready(snapshot) => Some(snapshot),
            Self::Acquiring | Self::Blocked { .. } => None,
        }
    }

    /// The terminal source status, only in the Ready state.
    #[must_use]
    pub(crate) fn ready_status(&self) -> Option<&TypeshedStatus> {
        self.ready_snapshot().map(|snapshot| &snapshot.status)
    }

    /// Project this generation into the one typed editor/status DTO.
    #[must_use]
    pub(crate) fn status_state(&self) -> TypeshedStatusState {
        match self {
            Self::Acquiring => status_projection(None),
            Self::Ready(snapshot) => status_projection(Some(&snapshot.status)),
            Self::Blocked { failure } => {
                let mut state = status_projection(None);
                state.lifecycle = TypeshedLifecycle::Blocked;
                state.blocked_reason = Some(failure.reason().to_owned());
                state.license_status = failure.license_status();
                state
            }
        }
    }
}

/// Root-keyed generation map shared by the LSP's analysis and status surfaces.
pub(crate) type TypeshedGenerations = BTreeMap<PathBuf, TypeshedGeneration>;

/// Merge root-keyed typed lifecycle states into the initialize capability.
#[must_use]
pub(super) fn experimental_payload(
    base: Option<Value>,
    generations: &TypeshedGenerations,
) -> Value {
    let mut root = object_or_empty(base);
    let basilisk = root
        .entry("basilisk".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if !basilisk.is_object() {
        *basilisk = Value::Object(Map::new());
    }
    if let Some(basilisk) = basilisk.as_object_mut() {
        let statuses = generations
            .iter()
            .filter_map(|(path, generation)| {
                let root_uri = Url::from_file_path(path).ok()?;
                Some(serde_json::json!({
                    "rootUri": root_uri,
                    "status": generation.status_state(),
                }))
            })
            .collect::<Vec<_>>();
        drop(basilisk.insert("typeshedStatuses".to_owned(), Value::Array(statuses)));
    }
    Value::Object(root)
}

/// Typed terminal/acquiring status notification.
pub(crate) enum TypeshedStatusChangedNotification {}

impl Notification for TypeshedStatusChangedNotification {
    type Params = TypeshedStatusChanged;
    const METHOD: &'static str = basilisk_common::configuration_editor::TYPESHED_STATUS_CHANGED;
}

/// Notify clients after one root's generation changes.
pub(crate) async fn notify_generation(
    client: &Client,
    root: &Path,
    generation: &TypeshedGeneration,
) {
    let Ok(root_uri) = Url::from_file_path(root) else {
        tracing::warn!(root = %root.display(), "cannot publish Typeshed status for non-file root");
        return;
    };
    client
        .send_notification::<TypeshedStatusChangedNotification>(TypeshedStatusChanged {
            root_uri: root_uri.to_string(),
            status: generation.status_state(),
        })
        .await;
}

/// Surface elevated source warnings outside the diagnostic channel.
pub(crate) async fn show_high_warnings(client: &Client, status: &TypeshedStatus) {
    for message in high_warning_messages(status) {
        client.show_message(MessageType::WARNING, message).await;
    }
}

fn high_warning_messages(status: &TypeshedStatus) -> Vec<String> {
    status
        .warnings
        .iter()
        .filter(|warning| warning.severity == WarningSeverity::High)
        .map(|warning| format!("Typeshed: {}", warning.message))
        .collect()
}

fn object_or_empty(value: Option<Value>) -> Map<String, Value> {
    value
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use basilisk_stubs::typeshed::gittree::Oid;
    use basilisk_stubs::typeshed::source::{
        LicenseStatus, Provenance, SourceKind, StatusWarning, Transport,
    };
    use basilisk_stubs::typeshed::warning::{TypeshedWarning, UnpinnedKind};

    use super::*;

    fn fallback_status() -> TypeshedStatus {
        TypeshedStatus {
            active_source: SourceKind::Bundled,
            commit: Oid::from_hex("83c2518a9e6abbda0c44592c3483de459198f887").ok(),
            tree: Oid::from_hex("66408ffce2750980efc6da09e8a6652733f852e4").ok(),
            transport: Transport::EmbeddedZip,
            license_status: LicenseStatus::Approved,
            license_reference: Some(
                "https://github.com/python/typeshed/blob/83c2518a9e6abbda0c44592c3483de459198f887/LICENSE"
                    .to_owned(),
            ),
            provenance: Provenance::BundleVetted,
            signed_release: false,
            warnings: StatusWarning::list(&[
                TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled),
                TypeshedWarning::DownloadFailed {
                    bundled_sha: "83c2518a9e6abbda0c44592c3483de459198f887".to_owned(),
                },
                TypeshedWarning::Unverified,
            ]),
        }
    }

    #[test]
    fn payload_merges_and_uses_lsp_field_names() {
        let Ok(mut snapshot) = basilisk_stubs::typeshed::bundle::bundled_snapshot() else {
            return;
        };
        snapshot.status = fallback_status();
        let generations = BTreeMap::from([(
            PathBuf::from("/workspace"),
            TypeshedGeneration::Ready(Arc::new(snapshot)),
        )]);
        let payload = experimental_payload(
            Some(serde_json::json!({"basilisk": {"configurationEditor": true}})),
            &generations,
        );
        assert_eq!(
            payload.pointer("/basilisk/configurationEditor"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            payload.pointer("/basilisk/typeshedStatuses/0/status/activeSource/kind"),
            Some(&Value::String("Bundled".to_owned()))
        );
        assert_eq!(
            payload.pointer("/basilisk/typeshedStatuses/0/status/warnings/0/code"),
            Some(&Value::String("UNPINNED".to_owned()))
        );
        assert_eq!(
            payload.pointer("/basilisk/typeshedStatuses/0/status/warnings/1/code"),
            Some(&Value::String("DOWNLOAD FAILED".to_owned()))
        );
        assert_eq!(
            payload.pointer("/basilisk/typeshedStatuses/0/status/warnings/2/code"),
            Some(&Value::String("UNVERIFIED".to_owned()))
        );
        assert_eq!(
            payload.pointer("/basilisk/typeshedStatuses/0/status/provenance/kind"),
            Some(&Value::String("BundleVetted".to_owned()))
        );
        assert_eq!(
            payload.pointer("/basilisk/typeshedStatuses/0/status/transport/kind"),
            Some(&Value::String("EmbeddedZip".to_owned()))
        );
        assert_eq!(
            payload.pointer("/basilisk/typeshedStatuses/0/status/signedRelease"),
            Some(&Value::Bool(false))
        );
    }

    #[test]
    fn blocked_generation_has_no_candidate_source_or_provenance() {
        let state = TypeshedGeneration::Blocked {
            failure: TypeshedFailure::acquisition("exact commit unavailable"),
        }
        .status_state();
        assert_eq!(state.lifecycle, TypeshedLifecycle::Blocked);
        assert_eq!(
            state.blocked_reason.as_deref(),
            Some("exact commit unavailable")
        );
        assert_eq!(state.license_status, TypeshedLicenseStatus::Unavailable);
        assert!(state.active_source.is_none());
        assert!(state.commit_identity.is_none());
    }

    #[test]
    fn exact_license_drift_projects_changed_while_custom_failure_does_not() {
        let Ok(commit) = basilisk_stubs::typeshed::gittree::Oid::from_hex(
            "0123456789012345678901234567890123456789",
        ) else {
            return;
        };
        let exact = TypeshedFailure::from_selection(&SelectionError::Exact {
            commit,
            reason: BackendError::LicenseChanged,
        });
        let exact_state = TypeshedGeneration::Blocked { failure: exact }.status_state();
        assert_eq!(exact_state.license_status, TypeshedLicenseStatus::Changed);

        let custom =
            TypeshedFailure::from_selection(&SelectionError::Custom(BackendError::LicenseChanged));
        let custom_state = TypeshedGeneration::Blocked { failure: custom }.status_state();
        assert_eq!(
            custom_state.license_status,
            TypeshedLicenseStatus::Unavailable
        );
    }

    #[test]
    fn show_message_projection_contains_only_high_warnings() {
        let messages = high_warning_messages(&fallback_status());
        assert_eq!(messages.len(), 2);
        assert!(messages
            .first()
            .is_some_and(|message| message.contains("DOWNLOAD FAILED")));
        assert!(messages
            .get(1)
            .is_some_and(|message| message.contains("UNVERIFIED")));
        assert!(messages.iter().all(|message| !message.contains("UNPINNED")));

        let source = include_str!("typeshed_status.rs");
        assert!(source.contains("client.show_message"));
        let diagnostic_method = ["publish", "diagnostics"].join("_");
        assert!(!source.contains(&diagnostic_method));
    }
}
