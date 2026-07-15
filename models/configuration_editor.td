# typeDiagram model - https://typediagram.dev
# Editor wire protocol for the configuration editor
# ([CONFIGEDITOR-MODEL] / [LSPARCH-CONFIG-EDITOR-PROTOCOL]).
#
# This file is the LSP editor protocol ONLY. It builds on the core
# configuration model in models/configuration.td (RuleCode, RuleSeverity) and
# MUST NOT add anything to it: the editor reads and writes plain rule entries,
# nothing more.
#
# Regenerate the committed DTO shapes (concatenation is required: codegen
# rejects undeclared names, so the core model must come first). Reconcile the
# stdout into the committed files:
#   cat models/configuration.td models/configuration_editor.td | typediagram --to rust
#     -> crates/basilisk-lsp/src/configuration_editor/model.rs
#   cat models/configuration.td models/configuration_editor.td | typediagram --to typescript
#     -> vscode-extension/src/configuration-editor-model.ts
# Render the documentation diagram:
#   typediagram models/configuration_editor.td > docs/models/configuration_editor.svg

alias Uri = String
alias Revision = String
alias PreviewId = String

union TagKind {
  Provenance
  PepCategory
  Descriptive
}

# Bulk selection for mutations and occurrence queries ([CONFIGEDITOR-TAGS]).
# Selectors expand server-side, at preview time, to plain rule entries.
union RuleSelector {
  All
  Codes { codes: List<RuleCode> }
  Tags { tags: List<String>, matchAll: Bool }
}

# The only two things the editor can ask for. Set writes explicit entries;
# Remove deletes entries, and a rule without an entry is disabled
# ([CHKARCH-CONFIG-MODEL]).
union EditorMutation {
  Set { selector: RuleSelector, severity: RuleSeverity }
  Remove { selector: RuleSelector }
}

type RuleDescriptor {
  code: RuleCode
  title: String
  summary: String
  docsUrl: Uri
  tags: List<String>
}

# severity mirrors the edited config file exactly: None means no entry.
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
  mutations: List<EditorMutation>
}

# One real entry change, fully expanded and normalized.
# None on either side means "no entry".
type ResolvedEntryChange {
  code: RuleCode
  previousSeverity: Option<RuleSeverity>
  newSeverity: Option<RuleSeverity>
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
  changes: List<ResolvedEntryChange>
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
