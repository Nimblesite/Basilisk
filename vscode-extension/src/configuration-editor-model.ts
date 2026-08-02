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
  | { kind: "TypeshedPackage" }
  | { kind: "TypeshedStorePath" };

/**
 * The persistent result cache's two [tool.basilisk] keys ([LSPCFGED-CACHE]).
 * The in-session Salsa layer is deliberately absent: it is always on and has
 * no key, so there is nothing here to set.
 */
export type CacheSettingKey =
  | { kind: "CacheEnabled" }
  | { kind: "CacheDir" };

export type EditorMutation =
  | { kind: "SetRule"; code: RuleCode; severity: RuleSeverity }
  | { kind: "RemoveRule"; code: RuleCode }
  | { kind: "SetTag"; tag: RuleTag; severity: RuleSeverity }
  | { kind: "RemoveTag"; tag: RuleTag }
  | { kind: "SetTypeshedSetting"; key: TypeshedSettingKey; value: string }
  | { kind: "RemoveTypeshedSetting"; key: TypeshedSettingKey }
  | { kind: "SetCacheSetting"; key: CacheSettingKey; value: string }
  | { kind: "RemoveCacheSetting"; key: CacheSettingKey };

/**
 * There are exactly three sources, each carrying the value that defines it:
 * only one may be active at a time, and there is no "track latest" source
 * ([LSPCFGED-TYPESHED], [STUBRES-TYPESHED-PYPI]).
 */
export type TypeshedSource =
  | { kind: "ExactCommit"; commit: string }
  | { kind: "CustomFolder"; path: string }
  | { kind: "PyPIPackage"; name: string; sha256: string };

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
  | { kind: "Bundled" }
  | { kind: "PyPIPackage" };

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

/**
 * The persistent, cross-session result cache ([CHKCACHE]). `folder` is the
 * effective location the next run uses, so the editor never shows a folder the
 * run would not use; `folderConfigured` separates the default from a project's
 * own choice.
 */
export interface PersistentCacheState {
  enabled: boolean;
  folder: string;
  folderConfigured: boolean;
}

/**
 * The in-session incremental engine ([CHKARCH-INCREMENTAL-SALSA]): always on,
 * no configuration key, so its one real value is the live memo count.
 */
export interface InSessionCacheState {
  trackedFiles: number;
}

export interface CacheConfigurationState {
  persistent: PersistentCacheState;
  inSession: InSessionCacheState;
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
  cache: CacheConfigurationState;
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
  cacheChanges: CacheSettingChange[];
  impact: ConfigurationImpact;
}

export interface TypeshedSettingChange {
  key: TypeshedSettingKey;
  before: string | undefined;
  after: string | undefined;
}

/** Rendered TOML text on each side; undefined = the key is absent there. */
export interface CacheSettingChange {
  key: CacheSettingKey;
  before: string | undefined;
  after: string | undefined;
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
