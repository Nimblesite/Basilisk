//! Typeshed portion of the generated configuration-editor wire model.

use serde::{Deserialize, Serialize};

use super::{
    ConfigurationPreview, ConfigurationSnapshot, Revision, TypeshedSettingKey,
    TypeshedSettingValue, Uri,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedSourceMode {
    Latest,
    ExactCommit,
    CustomFolder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeshedWidget {
    Directory,
    Text,
    Boolean,
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
pub struct TypeshedSourceOption {
    pub mode: TypeshedSourceMode,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedSettingState {
    pub key: TypeshedSettingKey,
    pub label: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<TypeshedSettingValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<TypeshedSettingValue>,
    pub widget: TypeshedWidget,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedActionState {
    pub action: TypeshedAction,
    pub label: String,
    pub enabled: bool,
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
    pub tree_identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<TypeshedTransport>,
    pub license_status: TypeshedLicenseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_reference: Option<String>,
    pub provenance: TypeshedProvenance,
    pub signed_release: bool,
    pub warnings: Vec<TypeshedWarningState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeshedConfigurationState {
    pub source_mode: TypeshedSourceMode,
    pub source_options: Vec<TypeshedSourceOption>,
    pub settings: Vec<TypeshedSettingState>,
    pub actions: Vec<TypeshedActionState>,
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
