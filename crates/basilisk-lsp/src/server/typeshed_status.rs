//! Implements [STUBRES-TYPESHED-WARN] LSP routing.
//!
//! Typeshed source status is editor metadata. It is merged into the
//! initialize payload for persistent Service Info and elevated warnings are
//! sent with `window/showMessage`; none of it is a Python diagnostic.
//!
//! There is **no acquiring state**: resolution is a local read
//! ([STUBRES-TYPESHED-OFFLINE]), so a root's generation is always terminal —
//! `Ready` or `NoSource`. The only long-running lifecycle is a user-invoked
//! download, which never replaces the generation until it has finished and
//! re-resolved locally ([LSPCFGED-TYPESHED-DOWNLOAD]).

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
use crate::configuration_editor::snapshot_typeshed::ready_projection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeshedFailureKind {
    LicenseChanged,
    CustomUnavailable,
    NoSource,
    ResolutionFailed,
}

/// One redacted terminal resolution failure with the selection category kept
/// intact for typed RPC/status projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeshedFailure {
    kind: TypeshedFailureKind,
    reason: String,
}

impl TypeshedFailure {
    #[must_use]
    pub(crate) fn resolution(reason: impl Into<String>) -> Self {
        Self {
            kind: TypeshedFailureKind::ResolutionFailed,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub(crate) fn from_selection(error: &SelectionError) -> Self {
        let kind = match error {
            // Custom sources provide user-managed terms, so they never project
            // the official Typeshed license as changed.
            SelectionError::Custom(_) => TypeshedFailureKind::CustomUnavailable,
            SelectionError::NoSource { reason, .. } if *reason == BackendError::LicenseChanged => {
                TypeshedFailureKind::LicenseChanged
            }
            SelectionError::PyPIPackage { reason, .. }
                if *reason == BackendError::LicenseChanged =>
            {
                TypeshedFailureKind::LicenseChanged
            }
            SelectionError::NoSource { .. } | SelectionError::PyPIPackage { .. } => {
                TypeshedFailureKind::NoSource
            }
            SelectionError::InconsistentIdentity => TypeshedFailureKind::ResolutionFailed,
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
            TypeshedFailureKind::NoSource => "typeshedNoSource",
            TypeshedFailureKind::ResolutionFailed => "typeshedResolutionFailed",
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

/// One workspace root's terminal resolution generation.
///
/// A candidate is never exposed as Ready until every activation gate passes.
/// A configuration change replaces this value atomically with the *next*
/// terminal generation — there is no intermediate state to render, which is
/// what keeps the editor free of blocking overlays ([LSPCFGED-TYPESHED]).
#[derive(Debug, Clone)]
pub(crate) enum TypeshedGeneration {
    /// One complete immutable source is active.
    Ready(Arc<Snapshot>),
    /// The selected source is not on this machine; analysis does not run.
    /// The reason is redacted and safe for UI/MCP.
    NoSource { failure: TypeshedFailure },
}

impl TypeshedGeneration {
    /// The active immutable snapshot, only in the Ready state.
    #[must_use]
    pub(crate) fn ready_snapshot(&self) -> Option<&Arc<Snapshot>> {
        match self {
            Self::Ready(snapshot) => Some(snapshot),
            Self::NoSource { .. } => None,
        }
    }

    /// The terminal source status, only in the Ready state.
    #[must_use]
    pub(crate) fn ready_status(&self) -> Option<&TypeshedStatus> {
        self.ready_snapshot().map(|snapshot| &snapshot.status)
    }

    /// The terminal failure, only in the `NoSource` state — the seam the LSP uses
    /// to raise an immediate, client-visible error when analysis will not run.
    #[must_use]
    pub(crate) const fn no_source_failure(&self) -> Option<&TypeshedFailure> {
        match self {
            Self::Ready(_) => None,
            Self::NoSource { failure } => Some(failure),
        }
    }

    /// Project this generation into the one typed editor/status DTO.
    #[must_use]
    pub(crate) fn status_state(&self) -> TypeshedStatusState {
        match self {
            Self::Ready(snapshot) => ready_projection(&snapshot.status),
            Self::NoSource { failure } => TypeshedStatusState {
                lifecycle: TypeshedLifecycle::NoSource,
                no_source_reason: Some(failure.reason().to_owned()),
                active_source: None,
                commit_identity: None,
                license_status: failure.license_status(),
                warnings: Vec::new(),
            },
        }
    }
}

/// The transient status shown while a user-invoked download runs. The
/// generation map is untouched — the previous source keeps serving analysis —
/// so this state can only ever appear on the invoking control, never as a
/// panel-blocking mode ([LSPCFGED-TYPESHED-DOWNLOAD]).
#[must_use]
pub(crate) fn downloading_state(commit: Option<&str>) -> TypeshedStatusState {
    TypeshedStatusState {
        lifecycle: TypeshedLifecycle::Downloading,
        no_source_reason: None,
        active_source: None,
        commit_identity: commit.map(str::to_owned),
        license_status: TypeshedLicenseStatus::Unavailable,
        warnings: Vec::new(),
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

/// Typed terminal/downloading status notification.
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
    notify_status(client, root, generation.status_state()).await;
}

/// Notify clients of one root's status DTO (terminal or download-transient).
pub(crate) async fn notify_status(client: &Client, root: &Path, status: TypeshedStatusState) {
    let Ok(root_uri) = Url::from_file_path(root) else {
        tracing::warn!(root = %root.display(), "cannot publish Typeshed status for non-file root");
        return;
    };
    client
        .send_notification::<TypeshedStatusChangedNotification>(TypeshedStatusChanged {
            root_uri: root_uri.to_string(),
            status,
        })
        .await;
}

/// Surface elevated source warnings outside the diagnostic channel.
pub(crate) async fn show_high_warnings(client: &Client, status: &TypeshedStatus) {
    for message in high_warning_messages(status) {
        client.show_message(MessageType::WARNING, message).await;
    }
}

/// Raise an immediate, client-visible error for a root whose typeshed did not
/// resolve. The LSP DRIVES this signal with `window/showMessage` rather than
/// trusting each editor to render the status payload: a `NoSource` root runs no
/// analysis at all, so a trace-log line alone would let a broken deployment
/// masquerade as a clean workspace. Every client — VS Code, Neovim, Zed — gets
/// the same unmissable error. [STUBRES-TYPESHED-WARN]
pub(crate) async fn show_no_source_error(client: &Client, root: &Path, failure: &TypeshedFailure) {
    client
        .show_message(MessageType::ERROR, no_source_error_message(root, failure))
        .await;
}

/// The actionable `NoSource` error text: what broke, what it means, and the fix.
fn no_source_error_message(root: &Path, failure: &TypeshedFailure) -> String {
    format!(
        "Basilisk: type checking is disabled for {} — the typeshed type stubs could not be \
loaded ({}). Run `basilisk typeshed download` to materialise the pinned source, or correct the \
typeshed configuration; until then no Python diagnostics are produced for this folder.",
        root.display(),
        failure.reason()
    )
}

fn high_warning_messages(status: &TypeshedStatus) -> Vec<String> {
    status
        .warnings
        .iter()
        .filter(|warning| warning.severity == WarningSeverity::High)
        .map(|warning| {
            format!(
                "Typeshed [{}]: {} — see {}",
                warning.code, warning.message, warning.docs_url
            )
        })
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
    use basilisk_stubs::typeshed::source::{LicenseStatus, SourceKind, StatusWarning};
    use basilisk_stubs::typeshed::warning::{TypeshedWarning, UnpinnedKind};

    use super::*;

    fn bundled_default_status() -> TypeshedStatus {
        TypeshedStatus {
            active_source: SourceKind::Bundled,
            commit: Oid::from_hex("83c2518a9e6abbda0c44592c3483de459198f887").ok(),
            tree: Oid::from_hex("66408ffce2750980efc6da09e8a6652733f852e4").ok(),
            license_status: LicenseStatus::Approved,
            license_reference: Some(
                "https://github.com/python/typeshed/blob/83c2518a9e6abbda0c44592c3483de459198f887/LICENSE"
                    .to_owned(),
            ),
            warnings: StatusWarning::list(&[
                TypeshedWarning::LicenseChanged,
                TypeshedWarning::Unpinned(UnpinnedKind::BundledDefault),
            ]),
        }
    }

    #[test]
    fn payload_merges_and_uses_lsp_field_names() {
        let Ok(mut snapshot) = basilisk_stubs::typeshed::bundle::bundled_snapshot() else {
            return;
        };
        snapshot.status = bundled_default_status();
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
            Some(&Value::String("typeshed_source_unpinned".to_owned()))
        );
        assert_eq!(
            payload.pointer("/basilisk/typeshedStatuses/0/status/warnings/1/code"),
            Some(&Value::String("typeshed_source_license_changed".to_owned()))
        );
        // The retired trust-bijection fields must never reappear on the wire:
        // the active source IS the trust story ([STUBRES-TYPESHED-WARN]).
        for retired in ["transport", "provenance", "signedRelease", "blockedReason"] {
            assert_eq!(
                payload.pointer(&format!("/basilisk/typeshedStatuses/0/status/{retired}")),
                None,
                "retired wire field: {retired}"
            );
        }
    }

    /// [LSPCFGED-TYPESHED]: the lifecycle union has NO acquiring/blocked
    /// panel state — a generation is always terminal, so there is nothing a
    /// client could render as a blocking overlay between config changes.
    #[test]
    fn generation_states_are_terminal_ready_or_no_source() {
        let no_source = TypeshedGeneration::NoSource {
            failure: TypeshedFailure::resolution("NO SOURCE — pin is not on this machine"),
        }
        .status_state();
        assert_eq!(no_source.lifecycle, TypeshedLifecycle::NoSource);
        assert_eq!(
            no_source.no_source_reason.as_deref(),
            Some("NO SOURCE — pin is not on this machine")
        );
        assert_eq!(no_source.license_status, TypeshedLicenseStatus::Unavailable);
        assert!(no_source.active_source.is_none());
        assert!(no_source.commit_identity.is_none());

        // Serde-level proof the retired states are gone from the wire union.
        for retired in ["Acquiring", "Blocked"] {
            let json = format!("{{\"kind\":\"{retired}\"}}");
            assert!(
                serde_json::from_str::<TypeshedLifecycle>(&json).is_err(),
                "retired lifecycle must not deserialize: {retired}"
            );
        }
    }

    /// [LSPCFGED-TYPESHED-DOWNLOAD]: the transient download status carries the
    /// requested pin but never a source — it is button state, not a panel mode.
    #[test]
    fn downloading_state_is_transient_button_state() {
        let state = downloading_state(Some("83c2518a9e6abbda0c44592c3483de459198f887"));
        assert_eq!(state.lifecycle, TypeshedLifecycle::Downloading);
        assert_eq!(
            state.commit_identity.as_deref(),
            Some("83c2518a9e6abbda0c44592c3483de459198f887")
        );
        assert!(state.active_source.is_none());
        assert!(state.no_source_reason.is_none());
    }

    #[test]
    fn no_source_license_drift_projects_changed_while_custom_failure_does_not() {
        let Ok(commit) = basilisk_stubs::typeshed::gittree::Oid::from_hex(
            "0123456789012345678901234567890123456789",
        ) else {
            return;
        };
        let drifted = TypeshedFailure::from_selection(&SelectionError::NoSource {
            commit,
            reason: BackendError::LicenseChanged,
        });
        assert_eq!(drifted.rpc_code(), "typeshedLicenseChanged");
        let drifted_state = TypeshedGeneration::NoSource { failure: drifted }.status_state();
        assert_eq!(drifted_state.license_status, TypeshedLicenseStatus::Changed);

        let missing = TypeshedFailure::from_selection(&SelectionError::NoSource {
            commit,
            reason: BackendError::Missing,
        });
        assert_eq!(missing.rpc_code(), "typeshedNoSource");
        assert!(missing.reason().contains("NO SOURCE"));

        let custom =
            TypeshedFailure::from_selection(&SelectionError::Custom(BackendError::LicenseChanged));
        assert_eq!(custom.rpc_code(), "typeshedCustomUnavailable");
        let custom_state = TypeshedGeneration::NoSource { failure: custom }.status_state();
        assert_eq!(
            custom_state.license_status,
            TypeshedLicenseStatus::Unavailable
        );
    }

    #[test]
    fn show_message_projection_contains_only_high_warnings() {
        let messages = high_warning_messages(&bundled_default_status());
        assert_eq!(messages.len(), 1);
        assert!(messages.first().is_some_and(|message| {
            message.contains("typeshed_source_license_changed")
                && message.contains(
                    "https://www.basilisk-python.dev/errors/typeshed_source_license_changed",
                )
        }));
        assert!(messages
            .iter()
            .all(|message| !message.contains("typeshed_source_unpinned")));

        let source = include_str!("typeshed_status.rs");
        assert!(source.contains("client.show_message"));
        let diagnostic_method = ["publish", "diagnostics"].join("_");
        assert!(!source.contains(&diagnostic_method));
    }

    /// A `NoSource` generation must expose its failure so the LSP can raise an
    /// immediate error; a Ready generation must not (there is nothing to raise).
    #[test]
    fn no_source_generation_exposes_its_failure_but_ready_does_not() {
        let failure = TypeshedFailure::resolution("NO SOURCE — pin is not on this machine");
        let no_source = TypeshedGeneration::NoSource { failure };
        assert!(no_source.no_source_failure().is_some());

        let Ok(snapshot) = basilisk_stubs::typeshed::bundle::bundled_snapshot() else {
            return;
        };
        let ready = TypeshedGeneration::Ready(Arc::new(snapshot));
        assert!(ready.no_source_failure().is_none());
    }

    /// [STUBRES-TYPESHED-WARN]: a root whose typeshed did not resolve runs NO
    /// analysis, so the user must be told at once — with the impact, the reason,
    /// and the fix. A silent trace-log line (the pre-fix behaviour) let a broken
    /// deployment masquerade as a clean workspace.
    #[test]
    fn no_source_error_message_is_loud_and_actionable() {
        let Ok(commit) = Oid::from_hex("0123456789012345678901234567890123456789") else {
            return;
        };
        let failure = TypeshedFailure::from_selection(&SelectionError::NoSource {
            commit,
            reason: BackendError::Missing,
        });
        let message = no_source_error_message(Path::new("/work/project"), &failure);
        assert!(
            message.contains("/work/project"),
            "must name the affected root: {message}"
        );
        assert!(
            message.contains("type checking is disabled"),
            "must state the impact: {message}"
        );
        assert!(
            message.contains("basilisk typeshed download"),
            "must give the remedy: {message}"
        );
        assert!(
            message.contains(failure.reason()),
            "must carry the underlying reason: {message}"
        );
    }

    /// The LSP DRIVES the `NoSource` error itself ([STUBRES-TYPESHED-WARN] —
    /// editors only react to LSP signals), so BOTH activation seams must call
    /// `show_no_source_error`: the initialize/folder-add path in `init.rs` and
    /// the shared `StagedResolution::publish` seam that the config-editor
    /// transaction, the watched on-disk config edit, and post-download
    /// re-resolution all route through. This guards against a regression back
    /// to a trace-only warning on either seam; the end-to-end message contract
    /// is pinned behaviorally in `ws_test_typeshed_no_source.rs`
    /// (`missing_pin_*_raises_an_error_toast`).
    #[test]
    fn every_no_source_activation_seam_raises_an_immediate_error() {
        assert!(
            include_str!("init.rs").contains("show_no_source_error"),
            "resolve_typeshed_for_roots must surface NoSource as a window/showMessage error"
        );
        assert!(
            include_str!("../configuration_editor/typeshed_resolution.rs")
                .contains("show_no_source_error"),
            "StagedResolution::publish must surface NoSource as a window/showMessage error — \
config edits and post-download re-resolution land here, not in init.rs"
        );
    }
}
