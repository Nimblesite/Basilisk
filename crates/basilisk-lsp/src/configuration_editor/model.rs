//! Generated from `models/configuration.td` + `models/configuration_editor.td`
//! (`cat models/configuration.td models/configuration_editor.td | typediagram --to rust`).
//!
//! Implements [CONFIGEDITOR-MODEL] / [LSPARCH-CONFIG-EDITOR-PROTOCOL]. The
//! declarations below preserve the typeDiagram shapes; derives and serde tags
//! are Rust transport integration applied to the generated declarations.

#![allow(clippy::struct_field_names, missing_docs)]

use serde::{Deserialize, Serialize};

pub type Uri = String;
pub type Revision = String;
pub type PreviewId = String;
pub type RuleCode = String;
pub type RuleTag = String;

/// The four values an entry can state ([CHKARCH-CONFIG-MODEL]). `Disabled`
/// can never apply to a `pep`-tagged rule.
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
pub enum TagKind {
    Provenance,
    PepCategory,
    Descriptive,
}

/// The only four things the editor can ask for — exactly the four things a
/// config file can express ([CHKARCH-CONFIG-MODEL]). Setting `Disabled` on a
/// `pep`-tagged rule is a request error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EditorMutation {
    SetRule {
        code: RuleCode,
        severity: RuleSeverity,
    },
    RemoveRule {
        code: RuleCode,
    },
    SetTag {
        tag: RuleTag,
        severity: RuleSeverity,
    },
    RemoveTag {
        tag: RuleTag,
    },
}

/// Read-side bulk selection for occurrence queries only; mutations never take
/// selectors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RuleSelector {
    All,
    Codes {
        codes: Vec<RuleCode>,
    },
    Tags {
        tags: Vec<RuleTag>,
        #[serde(rename = "matchAll")]
        match_all: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDescriptor {
    pub code: RuleCode,
    pub title: String,
    pub summary: String,
    pub docs_url: Uri,
    pub tags: Vec<RuleTag>,
}

/// `entry` mirrors the edited config file exactly (`None` = no per-rule
/// entry). `effective_severity` is what actually runs at the root scope after
/// tag-entry resolution: `Disabled` means "does not run" and never appears on
/// a `pep` rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleState {
    pub descriptor: RuleDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<RuleSeverity>,
    pub effective_severity: RuleSeverity,
    pub diagnostic_count: i64,
}

/// `entry` mirrors `[tool.basilisk.rule-tags]` exactly (`None` = no tag
/// entry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagState {
    pub name: RuleTag,
    pub kind: TagKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<RuleSeverity>,
    pub rule_count: i64,
    pub diagnostic_count: i64,
}

/// The active configuration source behind a snapshot ([CONFIGEDITOR-VSIX-EXPERIENCE]
/// Project view). `uri`, `exists`, and `read_only` come straight from the loaded
/// [`ConfigDocument`](basilisk_config::ConfigDocument) — never synthesized. There
/// is exactly one format (the root `pyproject.toml`), so no format discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSource {
    pub uri: Uri,
    pub exists: bool,
    pub read_only: bool,
}

/// One real configuration problem surfaced to the Project view — e.g. an entry
/// naming a rule code that is not in the catalog. Never a synthetic warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationProblem {
    pub code: RuleCode,
    pub message: String,
    pub uri: Uri,
    pub line: i64,
    pub character: i64,
}

/// Server-computed effective-state counters for the Overview and Adoption views.
/// Every field is a real count folded from the live diagnostic inventory and the
/// resolved rule severities — the exact effective state, never a synthetic score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebtSummary {
    /// Total emitted diagnostics across the root (the sum of the partitions).
    pub remaining_diagnostics: i64,
    /// Emitted `error`/safety diagnostics.
    pub error_diagnostics: i64,
    /// Emitted `warning` diagnostics.
    pub warning_diagnostics: i64,
    /// Emitted `info` diagnostics.
    pub info_diagnostics: i64,
    /// `pep` rules graded below `error` by a config entry (the adoption signature).
    pub adopted_rules: i64,
    /// Rules whose effective severity resolves to `Disabled`.
    pub disabled_rules: i64,
}

/// One per-rule entry inside a nested `[tool.basilisk.rules]` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathRuleSetting {
    pub code: RuleCode,
    pub severity: RuleSeverity,
}

/// One per-tag entry inside a nested `[tool.basilisk.rule-tags]` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathTagSetting {
    pub tag: RuleTag,
    pub severity: RuleSeverity,
}

/// One nested per-directory `[tool.basilisk]` table discovered under the root
/// ([CHKARCH-CONFIG-DISCOVERY]) for the Path Overrides view. `path` is the
/// directory (root-relative) whose `pyproject.toml` holds the table; the checker
/// honors these entries for code beneath that directory via the ancestor walk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathOverrideState {
    pub path: String,
    pub config_uri: Uri,
    pub rules: Vec<PathRuleSetting>,
    pub tags: Vec<PathTagSetting>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSnapshot {
    pub root_uri: Uri,
    pub config_uri: Uri,
    pub revision: Revision,
    pub source: ConfigurationSource,
    pub rules: Vec<RuleState>,
    pub tags: Vec<TagState>,
    pub path_overrides: Vec<PathOverrideState>,
    pub debt: DebtSummary,
    pub problems: Vec<ConfigurationProblem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewConfigurationRequest {
    pub root_uri: Uri,
    pub base_revision: Revision,
    pub mutations: Vec<EditorMutation>,
}

/// One rule's effective-severity change, fully resolved. `Disabled` = does
/// not run; a `pep` rule is never `Disabled` on either side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRuleChange {
    pub code: RuleCode,
    pub before: RuleSeverity,
    pub after: RuleSeverity,
}

/// A complete before/after partition by the three emitting severities; the
/// total diagnostic count is their sum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationImpact {
    pub errors_before: i64,
    pub errors_after: i64,
    pub warnings_before: i64,
    pub warnings_after: i64,
    pub infos_before: i64,
    pub infos_after: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationPreview {
    pub preview_id: PreviewId,
    pub base_revision: Revision,
    pub changes: Vec<ResolvedRuleChange>,
    pub impact: ConfigurationImpact,
}

/// `root_uri` + `preview_id` fully identify the cached preview, which already
/// pins its base revision; the server rejects the apply if that revision is
/// stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyConfigurationRequest {
    pub root_uri: Uri,
    pub preview_id: PreviewId,
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
    pub code: RuleCode,
    pub uri: Uri,
    pub range: SourceRange,
    pub severity: RuleSeverity,
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

/// Refresh signal: the snapshot for `root_uri` is stale; refetch at
/// `revision`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationChanged {
    pub root_uri: Uri,
    pub revision: Revision,
}
