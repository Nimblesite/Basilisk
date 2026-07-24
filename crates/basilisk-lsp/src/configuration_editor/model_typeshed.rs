//! Typeshed portion of the generated configuration-editor wire model.

use serde::{Deserialize, Serialize};

use super::{ConfigurationSnapshot, Revision, TypeshedSettingKey, Uri};

/// The active source and the value that defines it. There are exactly two
/// sources ([LSPCFGED-TYPESHED]): a pinned commit or a custom folder. There is
/// no "track latest" source — freshness is the user-invoked download action.
/// An unset pin reports the bundled default commit (still `UNPINNED`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
pub enum TypeshedSource {
    ExactCommit { commit: String },
    CustomFolder { path: String },
}

/// Downloading is the only long-running state, and it is always user-invoked
/// ([LSPCFGED-TYPESHED-DOWNLOAD]). `NoSource` = the selected source is not on
/// this machine, so analysis does not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedLifecycle {
    Downloading,
    Ready,
    NoSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedAction {
    DownloadLatest,
    DownloadPinned,
    ViewLicense,
}

/// The active source is the whole trust story (custom = user-managed, bundled
/// = build-vetted, exact commit = attested at download, re-proven offline), so
/// there are no separate transport or provenance fields
/// ([STUBRES-TYPESHED-WARN]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedActiveSource {
    Custom,
    ExactCommit,
    Bundled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedLicenseStatus {
    Unavailable,
    Approved,
    Changed,
    NotSupplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedWarningSeverity {
    Advisory,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedWarningState {
    pub code: String,
    pub message: String,
    pub severity: TypeshedWarningSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedStatusState {
    pub lifecycle: TypeshedLifecycle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_source_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_source: Option<TypeshedActiveSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_identity: Option<String>,
    pub license_status: TypeshedLicenseStatus,
    pub warnings: Vec<TypeshedWarningState>,
}

/// Everything the editor needs, and nothing it can misrender: the one active
/// source, the store folder pins resolve from (none for a custom folder), and
/// whether a license document exists to open. Labels are client copy, not
/// server state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedConfigurationState {
    pub source: TypeshedSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_folder: Option<String>,
    pub license_available: bool,
    pub status: TypeshedStatusState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedSettingChange {
    pub key: TypeshedSettingKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedActionRequest {
    pub root_uri: Uri,
    pub base_revision: Revision,
    pub action: TypeshedAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedLicenseDocument {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<Uri>,
    pub content: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[expect(
    clippy::large_enum_variant,
    reason = "model-first wire variants stay inline and match the generated typeDiagram DTO"
)]
#[serde(tag = "kind")]
pub enum TypeshedActionResult {
    Snapshot { snapshot: ConfigurationSnapshot },
    License { license: TypeshedLicenseDocument },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedStatusChanged {
    pub root_uri: Uri,
    pub status: TypeshedStatusState,
}
