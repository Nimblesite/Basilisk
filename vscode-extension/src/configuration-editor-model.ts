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

export type TypeshedSettingKey =
  | { kind: "TypeshedPath" }
  | { kind: "TypeshedCommit" }
  | { kind: "TypeshedStorePath" };

export type TypeshedSettingValue =
  | { kind: "Text"; value: string }
  | { kind: "Boolean"; value: boolean };

export type EditorMutation =
  | { kind: "SetRule"; code: RuleCode; severity: RuleSeverity }
  | { kind: "RemoveRule"; code: RuleCode }
  | { kind: "SetTag"; tag: RuleTag; severity: RuleSeverity }
  | { kind: "RemoveTag"; tag: RuleTag }
  | { kind: "SetTypeshedSetting"; key: TypeshedSettingKey; value: TypeshedSettingValue }
  | { kind: "RemoveTypeshedSetting"; key: TypeshedSettingKey };

/**
 * There are exactly two sources, each carrying the value that defines it:
 * "a pinned commit plus a custom folder" is unrepresentable, and there is no
 * "track latest" source at all ([LSPCFGED-TYPESHED]).
 */
export type TypeshedSource =
  | { kind: "ExactCommit"; commit: string }
  | { kind: "CustomFolder"; path: string };

export type TypeshedLifecycle =
  | { kind: "Downloading" }
  | { kind: "Ready" }
  | { kind: "NoSource" };

export type TypeshedAction =
  | { kind: "DownloadLatest" }
  | { kind: "DownloadPinned" }
  | { kind: "ViewLicense" };

/**
 * The active source is the whole trust story (custom = user-managed, bundled =
 * build-vetted, exact commit = attested at download, re-proven offline), so
 * there are no separate transport or provenance fields ([STUBRES-TYPESHED-WARN]).
 */
export type TypeshedActiveSource =
  | { kind: "Custom" }
  | { kind: "ExactCommit" }
  | { kind: "Bundled" };

export type TypeshedLicenseStatus =
  | { kind: "Unavailable" }
  | { kind: "Approved" }
  | { kind: "Changed" }
  | { kind: "NotSupplied" };

export type TypeshedWarningSeverity =
  | { kind: "Advisory" }
  | { kind: "High" };

export interface TypeshedWarningState {
  code: string;
  message: string;
  severity: TypeshedWarningSeverity;
}

export interface TypeshedStatusState {
  lifecycle: TypeshedLifecycle;
  noSourceReason: string | undefined;
  activeSource: TypeshedActiveSource | undefined;
  commitIdentity: string | undefined;
  licenseStatus: TypeshedLicenseStatus;
  warnings: TypeshedWarningState[];
}

/**
 * Everything the editor needs and nothing it can misrender: the one active
 * source, the store folder pins resolve from (none for a custom folder), and
 * whether a license document exists to open.
 */
export interface TypeshedConfigurationState {
  source: TypeshedSource;
  storeFolder: string | undefined;
  licenseAvailable: boolean;
  status: TypeshedStatusState;
}

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

export interface ConfigurationSource {
  uri: Uri;
  exists: boolean;
  readOnly: boolean;
}

export interface ConfigurationProblem {
  code: RuleCode;
  message: string;
  uri: Uri;
  line: number;
  character: number;
}

export interface DebtSummary {
  remainingDiagnostics: number;
  errorDiagnostics: number;
  warningDiagnostics: number;
  infoDiagnostics: number;
  adoptedRules: number;
  disabledRules: number;
}

export interface PathRuleSetting {
  code: RuleCode;
  severity: RuleSeverity;
}

export interface PathTagSetting {
  tag: RuleTag;
  severity: RuleSeverity;
}

export interface PathOverrideState {
  path: string;
  configUri: Uri;
  rules: PathRuleSetting[];
  tags: PathTagSetting[];
}

export interface ConfigurationSnapshot {
  rootUri: Uri;
  configUri: Uri;
  revision: Revision;
  source: ConfigurationSource;
  rules: RuleState[];
  tags: TagState[];
  pathOverrides: PathOverrideState[];
  debt: DebtSummary;
  problems: ConfigurationProblem[];
  typeshed: TypeshedConfigurationState;
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
  typeshedChanges: TypeshedSettingChange[];
  impact: ConfigurationImpact;
}

export interface TypeshedSettingChange {
  key: TypeshedSettingKey;
  before: TypeshedSettingValue | undefined;
  after: TypeshedSettingValue | undefined;
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

export interface TypeshedActionRequest {
  rootUri: Uri;
  baseRevision: Revision;
  action: TypeshedAction;
}

export interface TypeshedLicenseDocument {
  title: string;
  uri: Uri | undefined;
  content: string;
  readOnly: boolean;
}

/**
 * Downloads return the refreshed snapshot immediately (lifecycle Downloading);
 * completion arrives as TypeshedStatusChanged + ConfigurationChanged. No action
 * returns a preview — a download is not a configuration edit.
 */
export type TypeshedActionResult =
  | { kind: "Snapshot"; snapshot: ConfigurationSnapshot }
  | { kind: "License"; license: TypeshedLicenseDocument };

export interface TypeshedStatusChanged {
  rootUri: Uri;
  status: TypeshedStatusState;
}
