# typeDiagram model - https://typediagram.dev
# Source-of-truth wire model for the configuration editor
# ([CONFIGEDITOR-MODEL] / [LSPARCH-CONFIG-EDITOR-PROTOCOL]).
# Regenerate the committed language DTO shapes:
#   typediagram --to rust models/configuration_editor.td
#   typediagram --to typescript models/configuration_editor.td
# Render the documentation diagram:
#   typediagram models/configuration_editor.td > docs/models/configuration_editor.svg

alias Uri = String
alias Revision = String
alias PreviewId = String

# The only persisted values. A rule with no entry in any config file on the
# ancestor chain is disabled: there are no default severities, no inherited
# state, and no mutation intents ([CONFIGEDITOR-SEVERITY]). Disabled exists as
# an explicit entry so a child folder's config can switch off a rule an
# ancestor enables ([CONFIGEDITOR-OVERRIDES]).
union RuleSeverity {
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

# Bulk selection for mutations and occurrence queries ([CONFIGEDITOR-TAGS]).
# Selectors expand at preview time to explicit per-rule entries.
union RuleSelector {
  All
  Codes { codes: List<String> }
  Tags { tags: List<String>, matchAll: Bool }
}

# Set writes an explicit severity entry. Remove deletes the entry, returning
# the rule to disabled (or to the nearest ancestor entry, under a child
# folder's config). Nothing else can be requested.
union ConfigurationMutation {
  Set { selector: RuleSelector, severity: RuleSeverity }
  Remove { selector: RuleSelector }
}

type RuleDescriptor {
  code: String
  title: String
  summary: String
  docsUrl: Uri
  tags: List<String>
}

# severity mirrors the edited config file exactly: None means no entry, and a
# rule with no entry does not run.
type RuleState {
  descriptor: RuleDescriptor
  severity: Option<RuleSeverity>
  diagnosticCount: Int
}

type TagState {
  name: String
  kind: TagKind
  ruleCount: Int
  diagnosticCount: Int
}

type ConfigurationSnapshot {
  rootUri: Uri
  configUri: Uri
  revision: Revision
  rules: List<RuleState>
  tags: List<TagState>
}

type PreviewConfigurationRequest {
  rootUri: Uri
  baseRevision: Revision
  mutations: List<ConfigurationMutation>
}

# One real config entry change, fully expanded and normalized.
# None on either side means "no entry".
type ResolvedConfigurationChange {
  ruleCode: String
  previousSeverity: Option<RuleSeverity>
  newSeverity: Option<RuleSeverity>
}

type ConfigurationImpact {
  diagnosticsBefore: Int
  diagnosticsAfter: Int
  errorsBefore: Int
  errorsAfter: Int
  warningsBefore: Int
  warningsAfter: Int
}

type ConfigurationPreview {
  previewId: PreviewId
  baseRevision: Revision
  changes: List<ResolvedConfigurationChange>
  impact: ConfigurationImpact
}

type ApplyConfigurationRequest {
  rootUri: Uri
  previewId: PreviewId
  baseRevision: Revision
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

type ConfigurationChanged {
  rootUri: Uri
  revision: Revision
  reason: String
}
