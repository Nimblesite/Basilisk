//! Generated from `models/configuration_editor.td`.
//!
//! Implements [CONFIGEDITOR-MODEL] / [LSPARCH-CONFIG-EDITOR-PROTOCOL]. The
//! declarations below preserve the typeDiagram shapes; derives and serde tags
//! are Rust transport integration applied to the generated declarations.

#![allow(clippy::struct_field_names, missing_docs)]

use serde::{Deserialize, Serialize};

pub type Uri = String;
pub type Revision = String;
pub type PreviewId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RuleSetting {
    Inherit,
    Native,
    Error,
    Warning,
    Info,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TagKind {
    Provenance,
    PepCategory,
    Descriptive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ConfigurationFormat {
    PyprojectToml,
    BasiliskJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RuleSelector {
    All,
    Codes {
        codes: Vec<String>,
    },
    Tags {
        tags: Vec<String>,
        #[serde(rename = "matchAll")]
        match_all: bool,
    },
    CurrentViolations,
    SafeFixable,
    WithoutSafeFix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum MutationScope {
    Project,
    Path { pattern: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDescriptor {
    pub code: String,
    pub title: String,
    pub summary: String,
    pub docs_url: Uri,
    pub tags: Vec<String>,
    pub default_severity: RuleSeverity,
    pub default_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleState {
    pub descriptor: RuleDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_severity: Option<RuleSeverity>,
    pub effective_severity: RuleSeverity,
    pub inherited: bool,
    pub diagnostic_count: i64,
    pub affected_file_count: i64,
    pub safe_fix_count: i64,
    pub unsafe_fix_count: i64,
    pub adoption_exception_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagState {
    pub name: String,
    pub kind: TagKind,
    pub rule_count: i64,
    pub diagnostic_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSource {
    pub uri: Uri,
    pub format: ConfigurationFormat,
    pub exists: bool,
    pub read_only: bool,
    pub shadowed_sources: Vec<Uri>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationProblem {
    pub code: String,
    pub message: String,
    pub uri: Uri,
    pub line: i64,
    pub character: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebtSummary {
    pub remaining_diagnostics: i64,
    pub adopted_files: i64,
    pub adoption_exceptions: i64,
    pub suppression_diagnostics: i64,
    pub disabled_rules: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSnapshot {
    pub root_uri: Uri,
    pub revision: Revision,
    pub source: ConfigurationSource,
    pub rules: Vec<RuleState>,
    pub tags: Vec<TagState>,
    pub debt: DebtSummary,
    pub problems: Vec<ConfigurationProblem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationMutation {
    pub selector: RuleSelector,
    pub setting: RuleSetting,
    pub scope: MutationScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewConfigurationRequest {
    pub root_uri: Uri,
    pub base_revision: Revision,
    pub mutations: Vec<ConfigurationMutation>,
    pub run_safe_fixes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationImpact {
    pub changed_rules: i64,
    pub enabled_rules: i64,
    pub disabled_rules: i64,
    pub diagnostics_before: i64,
    pub diagnostics_after: i64,
    pub errors_before: i64,
    pub errors_after: i64,
    pub warnings_before: i64,
    pub warnings_after: i64,
    pub files_changed_by_safe_fixes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationPreview {
    pub preview_id: PreviewId,
    pub base_revision: Revision,
    pub expanded_rule_codes: Vec<String>,
    pub impact: ConfigurationImpact,
    pub problems: Vec<ConfigurationProblem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FixSafety {
    Safe,
    Unsafe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePosition {
    pub line: i64,
    pub character: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleOccurrence {
    pub rule_code: String,
    pub uri: Uri,
    pub range: SourceRange,
    pub effective_severity: RuleSeverity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_safety: Option<FixSafety>,
    pub configuration_source: Uri,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleOccurrencesRequest {
    pub root_uri: Uri,
    pub selector: RuleSelector,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleOccurrencesResponse {
    pub items: Vec<RuleOccurrence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyConfigurationRequest {
    pub root_uri: Uri,
    pub preview_id: PreviewId,
    pub base_revision: Revision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationChanged {
    pub root_uri: Uri,
    pub revision: Revision,
    pub reason: String,
}
