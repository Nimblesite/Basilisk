//! Typeshed portion of the generated configuration-editor wire model.

use serde::{Deserialize, Serialize};

use super::{
    ConfigurationPreview, ConfigurationSnapshot, Revision, TypeshedSettingKey,
    TypeshedSettingValue, Uri,
};

/// The active source and the value that defines it. A pinned commit and a
/// custom folder cannot coexist, and `Latest` cannot carry a pin — the wire
/// model makes those states unrepresentable ([LSPCFGED-TYPESHED]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
pub enum TypeshedSource {
    Latest,
    ExactCommit { commit: String },
    CustomFolder { path: String },
}

/// Download policy of a downloaded source. A user-managed folder downloads
/// nothing, so it has none at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedDownloadPolicy {
    pub reuse_downloads: bool,
    pub verify_content: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_folder: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedLifecycle {
    Acquiring,
    Ready,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedAction {
    PinCurrent,
    AcquireFresh,
    ViewLicense,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedActiveSource {
    Custom,
    ExactCommit,
    Latest,
    Bundled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedTransport {
    CustomPath,
    EmbeddedZip,
    Codeload,
    Mirror,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedLicenseStatus {
    Acquiring,
    Unavailable,
    Approved,
    Changed,
    NotSupplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedProvenance {
    Pending,
    GithubTlsAttested,
    Unverified,
    BundleVetted,
    UserManaged,
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
    pub blocked_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_source: Option<TypeshedActiveSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<TypeshedTransport>,
    pub license_status: TypeshedLicenseStatus,
    pub provenance: TypeshedProvenance,
    pub signed_release: bool,
    pub warnings: Vec<TypeshedWarningState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Everything the editor needs and nothing it can misrender: the one active
/// source, the download policy that source has, the commit
/// [`TypeshedAction::PinCurrent`] would write when pinning is possible, and
/// whether a license document exists to open. Labels are client copy.
pub struct TypeshedConfigurationState {
    pub source: TypeshedSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloads: Option<TypeshedDownloadPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinnable_commit: Option<String>,
    pub license_available: bool,
    pub status: TypeshedStatusState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedSettingChange {
    pub key: TypeshedSettingKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<TypeshedSettingValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<TypeshedSettingValue>,
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
    Preview { preview: ConfigurationPreview },
    Snapshot { snapshot: ConfigurationSnapshot },
    License { license: TypeshedLicenseDocument },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedStatusChanged {
    pub root_uri: Uri,
    pub status: TypeshedStatusState,
}
