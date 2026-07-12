# typeDiagram model - https://typediagram.dev
# Source-of-truth wire model for the configuration editor
# ([CONFIGEDITOR-MODEL] / [LSPARCH-CONFIG-EDITOR-PROTOCOL]).
# Generate language DTOs during implementation:
#   typediagram --to rust models/configuration_editor.td
#   typediagram --to typescript models/configuration_editor.td
# Render the documentation diagram:
#   typediagram models/configuration_editor.td > docs/models/configuration_editor.svg

alias Uri = String
alias Revision = String
alias PreviewId = String

union RuleSeverity {
  Error
  Warning
  Info
  Disabled
}

# Inherit and Native are mutation intents, not persisted severity values.
# Inherit removes an explicit override. Native writes each selected rule's own
# default severity and therefore enables opt-in rules without promoting warnings.
union RuleSetting {
  Inherit
  Native
  Error
  Warning
  Info
  Disabled
}

union TagKind {
  Provenance
  PepCategory
  Descriptive
}

union ConfigurationFormat {
  PyprojectToml
  BasiliskJson
}

union RuleSelector {
  All
  Codes { codes: List<String> }
  Tags { tags: List<String>, matchAll: Bool }
  CurrentViolations
  SafeFixable
  WithoutSafeFix
}

union MutationScope {
  Project
  Path { pattern: String }
}

type RuleDescriptor {
  code: String
  title: String
  summary: String
  docsUrl: Uri
  tags: List<String>
  defaultSeverity: RuleSeverity
  defaultEnabled: Bool
}

type RuleState {
  descriptor: RuleDescriptor
  configuredSeverity: Option<RuleSeverity>
  effectiveSeverity: RuleSeverity
  inherited: Bool
  diagnosticCount: Int
  affectedFileCount: Int
  safeFixCount: Int
  unsafeFixCount: Int
  adoptionExceptionCount: Int
}

type TagState {
  name: String
  kind: TagKind
  ruleCount: Int
  diagnosticCount: Int
}

type ConfigurationSource {
  uri: Uri
  format: ConfigurationFormat
  exists: Bool
  readOnly: Bool
  shadowedSources: List<Uri>
}

type ConfigurationProblem {
  code: String
  message: String
  uri: Uri
  line: Int
  character: Int
}

type DebtSummary {
  remainingDiagnostics: Int
  adoptedFiles: Int
  adoptionExceptions: Int
  suppressionDiagnostics: Int
  disabledRules: Int
}

# Presets are LSP-owned, one-shot mutation recipes. They are never persisted as
# an ambient policy mode: applying one expands its mutations to explicit rule
# severities in the active configuration file.
type ConfigurationPreset {
  id: String
  name: String
  summary: String
  mutations: List<ConfigurationMutation>
  runSafeFixes: Bool
}

type ConfigurationSnapshot {
  rootUri: Uri
  revision: Revision
  source: ConfigurationSource
  rules: List<RuleState>
  tags: List<TagState>
  presets: List<ConfigurationPreset>
  debt: DebtSummary
  problems: List<ConfigurationProblem>
}

type ConfigurationMutation {
  selector: RuleSelector
  setting: RuleSetting
  scope: MutationScope
}

type PreviewConfigurationRequest {
  rootUri: Uri
  baseRevision: Revision
  mutations: List<ConfigurationMutation>
  runSafeFixes: Bool
}

type ConfigurationImpact {
  changedRules: Int
  enabledRules: Int
  disabledRules: Int
  diagnosticsBefore: Int
  diagnosticsAfter: Int
  errorsBefore: Int
  errorsAfter: Int
  warningsBefore: Int
  warningsAfter: Int
  filesChangedBySafeFixes: Int
}

type ConfigurationPreview {
  previewId: PreviewId
  baseRevision: Revision
  expandedRuleCodes: List<String>
  impact: ConfigurationImpact
  problems: List<ConfigurationProblem>
}

union FixSafety {
  Safe
  Unsafe
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
  ruleCode: String
  uri: Uri
  range: SourceRange
  effectiveSeverity: RuleSeverity
  fixSafety: Option<FixSafety>
  configurationSource: Uri
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

type ApplyConfigurationRequest {
  rootUri: Uri
  previewId: PreviewId
  baseRevision: Revision
}

type ConfigurationChanged {
  rootUri: Uri
  revision: Revision
  reason: String
}
