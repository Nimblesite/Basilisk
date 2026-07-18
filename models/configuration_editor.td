# typeDiagram model - https://typediagram.dev
# Editor wire protocol for the configuration editor
# ([CONFIGEDITOR-MODEL] / [LSPARCH-CONFIG-EDITOR-PROTOCOL]).
#
# This file is the LSP editor protocol ONLY. It builds on the core
# configuration model in models/configuration.td (RuleCode, RuleTag,
# RuleSeverity) and MUST NOT add anything to it: the editor reads and writes
# plain rule and tag entries, nothing more.
#
# Regenerate the committed DTO shapes (concatenation is required: codegen
# rejects undeclared names, so the core model must come first). Reconcile the
# stdout into the committed files:
#   cat models/configuration.td models/configuration_editor.td | typediagram --to rust
#     -> crates/basilisk-lsp/src/configuration_editor/model.rs
#   cat models/configuration.td models/configuration_editor.td | typediagram --to typescript
#     -> vscode-extension/src/configuration-editor-model.ts
# Codegen only — no SVG rendering.

alias Uri = String
alias Revision = String
alias PreviewId = String

union TagKind {
  Provenance
  PepCategory
  Descriptive
}

# The only four things the editor can ask for - exactly the four things a
# config file can express: a rule entry, a tag entry, or the absence of
# either ([CHKARCH-CONFIG-MODEL]). Setting Disabled on a pep-tagged rule is a
# request error.
union EditorMutation {
  SetRule { code: RuleCode, severity: RuleSeverity }
  RemoveRule { code: RuleCode }
  SetTag { tag: RuleTag, severity: RuleSeverity }
  RemoveTag { tag: RuleTag }
}

# Read-side bulk selection for occurrence queries only; mutations never take
# selectors.
union RuleSelector {
  All
  Codes { codes: List<RuleCode> }
  Tags { tags: List<RuleTag>, matchAll: Bool }
}

type RuleDescriptor {
  code: RuleCode
  title: String
  summary: String
  docsUrl: Uri
  tags: List<RuleTag>
}

# entry mirrors the edited config file exactly (None = no per-rule entry).
# effectiveSeverity is what actually runs at the root scope after tag-entry
# resolution: Disabled means "does not run" and never appears on a pep rule.
type RuleState {
  descriptor: RuleDescriptor
  entry: Option<RuleSeverity>
  effectiveSeverity: RuleSeverity
  diagnosticCount: Int
}

# entry mirrors [tool.basilisk.rule-tags] exactly (None = no tag entry).
type TagState {
  name: RuleTag
  kind: TagKind
  entry: Option<RuleSeverity>
  ruleCount: Int
  diagnosticCount: Int
}

# The active configuration source behind a snapshot (Project view). uri/exists/
# readOnly are real filesystem metadata from the loaded ConfigDocument; there is
# exactly one format (the root pyproject.toml), so no format discriminator.
type ConfigurationSource {
  uri: Uri
  exists: Bool
  readOnly: Bool
}

# One real configuration problem for the Project view - e.g. an entry naming a
# rule code that is not in the catalog. Never a synthetic warning.
type ConfigurationProblem {
  code: RuleCode
  message: String
  uri: Uri
  line: Int
  character: Int
}

# Server-computed effective-state counters for the Overview and Adoption views.
# Every field is a real count folded from the live diagnostic inventory and the
# resolved rule severities - the exact effective state, never a synthetic score.
type DebtSummary {
  remainingDiagnostics: Int
  errorDiagnostics: Int
  warningDiagnostics: Int
  infoDiagnostics: Int
  adoptedRules: Int
  disabledRules: Int
}

# One per-rule entry inside a nested [tool.basilisk.rules] table.
type PathRuleSetting {
  code: RuleCode
  severity: RuleSeverity
}

# One per-tag entry inside a nested [tool.basilisk.rule-tags] table.
type PathTagSetting {
  tag: RuleTag
  severity: RuleSeverity
}

# One nested per-directory [tool.basilisk] table discovered under the root
# ([CHKARCH-CONFIG-DISCOVERY]) for the Path Overrides view. path is the
# directory (root-relative) whose pyproject.toml holds the table; the checker
# honors these entries for code beneath that directory via the ancestor walk.
type PathOverrideState {
  path: String
  configUri: Uri
  rules: List<PathRuleSetting>
  tags: List<PathTagSetting>
}

type ConfigurationSnapshot {
  rootUri: Uri
  configUri: Uri
  revision: Revision
  source: ConfigurationSource
  rules: List<RuleState>
  tags: List<TagState>
  pathOverrides: List<PathOverrideState>
  debt: DebtSummary
  problems: List<ConfigurationProblem>
}

type PreviewConfigurationRequest {
  rootUri: Uri
  baseRevision: Revision
  mutations: List<EditorMutation>
}

# One rule's effective-severity change, fully resolved. Disabled = does not
# run; a pep rule is never Disabled on either side.
type ResolvedRuleChange {
  code: RuleCode
  before: RuleSeverity
  after: RuleSeverity
}

# A complete before/after partition by the three emitting severities; the
# total diagnostic count is their sum.
type ConfigurationImpact {
  errorsBefore: Int
  errorsAfter: Int
  warningsBefore: Int
  warningsAfter: Int
  infosBefore: Int
  infosAfter: Int
}

type ConfigurationPreview {
  previewId: PreviewId
  baseRevision: Revision
  changes: List<ResolvedRuleChange>
  impact: ConfigurationImpact
}

# rootUri + previewId fully identify the cached preview, which already pins
# its base revision; the server rejects the apply if that revision is stale.
type ApplyConfigurationRequest {
  rootUri: Uri
  previewId: PreviewId
}

type SourcePosition {
  line: Int
  character: Int
}

type SourceRange {
  start: SourcePosition
  end: SourcePosition
}

type RuleOccurrence {
  code: RuleCode
  uri: Uri
  range: SourceRange
  severity: RuleSeverity
}

type RuleOccurrencesRequest {
  rootUri: Uri
  selector: RuleSelector
  cursor: Option<String>
  limit: Int
}

type RuleOccurrencesResponse {
  items: List<RuleOccurrence>
  nextCursor: Option<String>
}

# Refresh signal: the snapshot for rootUri is stale; refetch at revision.
type ConfigurationChanged {
  rootUri: Uri
  revision: Revision
}
