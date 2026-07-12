// Generated from models/configuration_editor.td.
// Implements [CONFIGEDITOR-MODEL] / [LSPARCH-CONFIG-EDITOR-PROTOCOL].
// Do not hand-maintain a second rule/configuration domain in the VSIX.

export type Uri = string;

export type Revision = string;

export type PreviewId = string;

export type RuleSeverity =
  | { kind: "Error" }
  | { kind: "Warning" }
  | { kind: "Info" }
  | { kind: "Disabled" };

export type RuleSetting =
  | { kind: "Inherit" }
  | { kind: "Native" }
  | { kind: "Error" }
  | { kind: "Warning" }
  | { kind: "Info" }
  | { kind: "Disabled" };

export type TagKind =
  | { kind: "Provenance" }
  | { kind: "PepCategory" }
  | { kind: "Descriptive" };

export type ConfigurationFormat =
  | { kind: "PyprojectToml" }
  | { kind: "BasiliskJson" };

export type RuleSelector =
  | { kind: "All" }
  | { kind: "Codes"; codes: Array<string> }
  | { kind: "Tags"; tags: Array<string>; matchAll: boolean }
  | { kind: "CurrentViolations" }
  | { kind: "SafeFixable" }
  | { kind: "WithoutSafeFix" };

export type MutationScope =
  | { kind: "Project" }
  | { kind: "Path"; pattern: string };

export interface RuleDescriptor {
  code: string;
  title: string;
  summary: string;
  docsUrl: Uri;
  tags: Array<string>;
  defaultSeverity: RuleSeverity;
  defaultEnabled: boolean;
}

export interface RuleState {
  descriptor: RuleDescriptor;
  configuredSeverity: RuleSeverity | undefined;
  effectiveSeverity: RuleSeverity;
  inherited: boolean;
  diagnosticCount: number;
  affectedFileCount: number;
  safeFixCount: number;
  unsafeFixCount: number;
  adoptionExceptionCount: number;
}

export interface TagState {
  name: string;
  kind: TagKind;
  ruleCount: number;
  diagnosticCount: number;
}

export interface ConfigurationSource {
  uri: Uri;
  format: ConfigurationFormat;
  exists: boolean;
  readOnly: boolean;
  shadowedSources: Array<Uri>;
}

export interface ConfigurationProblem {
  code: string;
  message: string;
  uri: Uri;
  line: number;
  character: number;
}

export interface DebtSummary {
  remainingDiagnostics: number;
  adoptedFiles: number;
  adoptionExceptions: number;
  suppressionDiagnostics: number;
  disabledRules: number;
}

export interface ConfigurationPreset {
  id: string;
  name: string;
  summary: string;
  mutations: Array<ConfigurationMutation>;
  runSafeFixes: boolean;
}

export interface ConfigurationSnapshot {
  rootUri: Uri;
  revision: Revision;
  source: ConfigurationSource;
  rules: Array<RuleState>;
  tags: Array<TagState>;
  presets: Array<ConfigurationPreset>;
  debt: DebtSummary;
  problems: Array<ConfigurationProblem>;
}

export interface ConfigurationMutation {
  selector: RuleSelector;
  setting: RuleSetting;
  scope: MutationScope;
}

export interface PreviewConfigurationRequest {
  rootUri: Uri;
  baseRevision: Revision;
  mutations: Array<ConfigurationMutation>;
  runSafeFixes: boolean;
}

export interface ConfigurationImpact {
  changedRules: number;
  enabledRules: number;
  disabledRules: number;
  diagnosticsBefore: number;
  diagnosticsAfter: number;
  errorsBefore: number;
  errorsAfter: number;
  warningsBefore: number;
  warningsAfter: number;
  filesChangedBySafeFixes: number;
}

export interface ConfigurationPreview {
  previewId: PreviewId;
  baseRevision: Revision;
  expandedRuleCodes: Array<string>;
  impact: ConfigurationImpact;
  problems: Array<ConfigurationProblem>;
}

export type FixSafety =
  | { kind: "Safe" }
  | { kind: "Unsafe" };

export interface SourcePosition {
  line: number;
  character: number;
}

export interface SourceRange {
  start: SourcePosition;
  end: SourcePosition;
}

export interface RuleOccurrence {
  ruleCode: string;
  uri: Uri;
  range: SourceRange;
  effectiveSeverity: RuleSeverity;
  fixSafety: FixSafety | undefined;
  configurationSource: Uri;
}

export interface RuleOccurrencesRequest {
  rootUri: Uri;
  selector: RuleSelector;
  cursor: string | undefined;
  limit: number;
}

export interface RuleOccurrencesResponse {
  items: Array<RuleOccurrence>;
  nextCursor: string | undefined;
}

export interface ApplyConfigurationRequest {
  rootUri: Uri;
  previewId: PreviewId;
  baseRevision: Revision;
}

export interface ConfigurationChanged {
  rootUri: Uri;
  revision: Revision;
  reason: string;
}
