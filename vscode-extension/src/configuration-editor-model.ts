// Generated from models/configuration.td + models/configuration_editor.td:
//   cat models/configuration.td models/configuration_editor.td | typediagram --to typescript
// Implements [CONFIGEDITOR-MODEL] / [LSPARCH-CONFIG-EDITOR-PROTOCOL] / [CHKARCH-CONFIG-MODEL].
// Do not hand-maintain a second rule/configuration domain in the VSIX.

export type RuleCode = string;

export type RuleTag = string;

export type RuleSeverity =
  | { kind: "Error" }
  | { kind: "Warning" }
  | { kind: "Info" }
  | { kind: "Disabled" };

export interface RuleEntry {
  code: RuleCode;
  severity: RuleSeverity;
}

export interface TagEntry {
  tag: RuleTag;
  severity: RuleSeverity;
}

export interface RulesConfig {
  rules: RuleEntry[];
  ruleTags: TagEntry[];
}

export type Uri = string;

export type Revision = string;

export type PreviewId = string;

export type TagKind =
  | { kind: "Provenance" }
  | { kind: "PepCategory" }
  | { kind: "Descriptive" };

export type EditorMutation =
  | { kind: "SetRule"; code: RuleCode; severity: RuleSeverity }
  | { kind: "RemoveRule"; code: RuleCode }
  | { kind: "SetTag"; tag: RuleTag; severity: RuleSeverity }
  | { kind: "RemoveTag"; tag: RuleTag };

export type RuleSelector =
  | { kind: "All" }
  | { kind: "Codes"; codes: RuleCode[] }
  | { kind: "Tags"; tags: RuleTag[]; matchAll: boolean };

export interface RuleDescriptor {
  code: RuleCode;
  title: string;
  summary: string;
  docsUrl: Uri;
  tags: RuleTag[];
}

export interface RuleState {
  descriptor: RuleDescriptor;
  entry: RuleSeverity | undefined;
  effectiveSeverity: RuleSeverity;
  diagnosticCount: number;
}

export interface TagState {
  name: RuleTag;
  kind: TagKind;
  entry: RuleSeverity | undefined;
  ruleCount: number;
  diagnosticCount: number;
}

export interface ConfigurationSnapshot {
  rootUri: Uri;
  configUri: Uri;
  revision: Revision;
  rules: RuleState[];
  tags: TagState[];
}

export interface PreviewConfigurationRequest {
  rootUri: Uri;
  baseRevision: Revision;
  mutations: EditorMutation[];
}

export interface ResolvedRuleChange {
  code: RuleCode;
  before: RuleSeverity;
  after: RuleSeverity;
}

export interface ConfigurationImpact {
  errorsBefore: number;
  errorsAfter: number;
  warningsBefore: number;
  warningsAfter: number;
  infosBefore: number;
  infosAfter: number;
}

export interface ConfigurationPreview {
  previewId: PreviewId;
  baseRevision: Revision;
  changes: ResolvedRuleChange[];
  impact: ConfigurationImpact;
}

export interface ApplyConfigurationRequest {
  rootUri: Uri;
  previewId: PreviewId;
}

export interface SourcePosition {
  line: number;
  character: number;
}

export interface SourceRange {
  start: SourcePosition;
  end: SourcePosition;
}

export interface RuleOccurrence {
  code: RuleCode;
  uri: Uri;
  range: SourceRange;
  severity: RuleSeverity;
}

export interface RuleOccurrencesRequest {
  rootUri: Uri;
  selector: RuleSelector;
  cursor: string | undefined;
  limit: number;
}

export interface RuleOccurrencesResponse {
  items: RuleOccurrence[];
  nextCursor: string | undefined;
}

export interface ConfigurationChanged {
  rootUri: Uri;
  revision: Revision;
}
