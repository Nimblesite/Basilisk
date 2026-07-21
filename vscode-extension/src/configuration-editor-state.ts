// Implements [VSIX-CONFIGURATION-EDITOR-THIN-SHELL].
/** Central immutable state and explicit actions for the configuration editor. */

import type { ReadonlySignal, Signal } from "@preact/signals-core";
import type {
  ConfigurationChanged,
  ConfigurationPreview,
  ConfigurationSnapshot,
  RuleOccurrencesResponse,
  TypeshedStatusChanged,
} from "./configuration-editor-model";

export type ConfigurationEditorPhase =
  | "idle"
  | "loading"
  | "ready"
  | "previewing"
  | "preview"
  | "applying"
  | "error"
  | "conflict"
  | "unsupported";

export interface ConfigurationEditorState {
  readonly phase: ConfigurationEditorPhase;
  readonly rootUri: string | undefined;
  readonly snapshot: ConfigurationSnapshot | undefined;
  readonly preview: ConfigurationPreview | undefined;
  readonly occurrences: RuleOccurrencesResponse | undefined;
  readonly occurrencesLoading: boolean;
  readonly repairUri: string | undefined;
  readonly message: string;
  readonly refreshRequested: boolean;
  /**
   * Rule code the webview should focus once per webview lifetime — set when
   * the editor is opened from a diagnostic's Configure Severity hover link
   * ([CONFIGEDITOR-VSIX-EXPERIENCE]).
   */
  readonly focusRule: string | undefined;
}

export const IDLE_CONFIGURATION_EDITOR: ConfigurationEditorState = {
  phase: "idle",
  rootUri: undefined,
  snapshot: undefined,
  preview: undefined,
  occurrences: undefined,
  occurrencesLoading: false,
  repairUri: undefined,
  message: "",
  refreshRequested: false,
  focusRule: undefined,
};

export interface ConfigurationEditorActions {
  /**
   * `focusRule` semantics: a string sets the pending focus target, `null`
   * clears it (a plain open with no target), and `undefined` (internal
   * refreshes) preserves any pending same-root focus.
   */
  beginConfigurationLoad(rootUri: string, focusRule?: string | null): void;
  acceptConfigurationSnapshot(snapshot: ConfigurationSnapshot): void;
  beginConfigurationPreview(): void;
  acceptConfigurationPreview(preview: ConfigurationPreview): void;
  /** Drop an unapplied preview and return to the snapshot as it stands. */
  cancelConfigurationPreview(): void;
  beginConfigurationApply(): void;
  beginRuleOccurrences(reset: boolean): void;
  acceptRuleOccurrences(response: RuleOccurrencesResponse, append: boolean): void;
  failRuleOccurrences(message: string): void;
  failConfigurationEditor(message: string, conflict?: boolean, repairUri?: string): void;
  markConfigurationChanged(change: ConfigurationChanged): void;
  markConfigurationUnsupported(message: string): void;
  resetConfigurationEditor(): void;
}

export interface ConfigurationEditorStore extends ConfigurationEditorActions {
  readonly configurationEditor: ReadonlySignal<ConfigurationEditorState>;
}

/** Validate the small server-pushed invalidation before it touches shared state. */
export function decodeConfigurationChanged(value: unknown): ConfigurationChanged | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) { return undefined; }
  const candidate = value as Record<string, unknown>;
  return typeof candidate.rootUri === "string" && typeof candidate.revision === "string"
    ? { rootUri: candidate.rootUri, revision: candidate.revision }
    : undefined;
}

function hasKind(value: unknown, allowed: readonly string[]): boolean {
  if (typeof value !== "object" || value === null || Array.isArray(value)) { return false; }
  const kind = (value as Record<string, unknown>).kind;
  return typeof kind === "string" && allowed.includes(kind);
}

function hasOptionalKind(value: unknown, allowed: readonly string[]): boolean {
  return value === undefined || hasKind(value, allowed);
}

function isOptionalString(value: unknown): boolean {
  return value === undefined || typeof value === "string";
}

function isTypeshedWarning(value: unknown): boolean {
  if (typeof value !== "object" || value === null || Array.isArray(value)) { return false; }
  const fields = value as Record<string, unknown>;
  return typeof fields.code === "string" && typeof fields.message === "string"
    && hasKind(fields.severity, ["Advisory", "High"]);
}

function hasTypeshedStateKinds(fields: Record<string, unknown>): boolean {
  return hasKind(fields.lifecycle, ["Downloading", "Ready", "NoSource"])
    && hasKind(fields.licenseStatus, ["Unavailable", "Approved", "Changed", "NotSupplied"])
    && hasOptionalKind(fields.activeSource, ["Custom", "ExactCommit", "Bundled"]);
}

function hasTypeshedIdentityFields(fields: Record<string, unknown>): boolean {
  return isOptionalString(fields.noSourceReason)
    && isOptionalString(fields.commitIdentity);
}

function isTypeshedStatus(value: unknown): boolean {
  if (typeof value !== "object" || value === null || Array.isArray(value)) { return false; }
  const fields = value as Record<string, unknown>;
  return hasTypeshedStateKinds(fields)
    && hasTypeshedIdentityFields(fields)
    && Array.isArray(fields.warnings)
    && fields.warnings.every(isTypeshedWarning);
}

/** Validate the typed Typeshed lifecycle notification before using it as an invalidation. */
export function decodeTypeshedStatusChanged(value: unknown): TypeshedStatusChanged | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) { return undefined; }
  const candidate = value as Record<string, unknown>;
  if (typeof candidate.rootUri !== "string" || !isTypeshedStatus(candidate.status)) {
    return undefined;
  }
  return candidate as unknown as TypeshedStatusChanged;
}

/** Mark an open root stale without replacing the snapshot beneath an active preview. */
export function requestConfigurationRefresh(
  state: Signal<ConfigurationEditorState>,
  change: ConfigurationChanged,
): void {
  if (state.value.rootUri !== change.rootUri) { return; }
  if (state.value.snapshot?.revision === change.revision) { return; }
  state.value = {
    ...state.value,
    message: "The project configuration changed; refreshing…",
    refreshRequested: true,
  };
}

/** A status generation can change while the TOML revision remains identical. */
export function requestTypeshedStatusRefresh(
  state: Signal<ConfigurationEditorState>,
  change: TypeshedStatusChanged,
): void {
  if (state.value.rootUri !== change.rootUri) { return; }
  state.value = {
    ...state.value,
    message: "Typeshed source status changed; refreshing…",
    refreshRequested: true,
  };
}

function beginLoad(
  state: Signal<ConfigurationEditorState>,
  rootUri: string,
  focusRule?: string | null,
): void {
  const sameRoot = state.value.rootUri === rootUri;
  // string = set, null = clear (plain open), undefined = internal refresh —
  // keep any pending same-root focus so it survives open()'s load chain.
  const nextFocus = focusRule === undefined
    ? (sameRoot ? state.value.focusRule : undefined)
    : (focusRule ?? undefined);
  state.value = {
    ...state.value,
    phase: "loading",
    rootUri,
    snapshot: sameRoot ? state.value.snapshot : undefined,
    preview: undefined,
    occurrences: sameRoot ? state.value.occurrences : undefined,
    occurrencesLoading: false,
    repairUri: undefined,
    message: "Reading the active project policy…",
    refreshRequested: false,
    focusRule: nextFocus,
  };
}

function acceptSnapshot(state: Signal<ConfigurationEditorState>, snapshot: ConfigurationSnapshot): void {
  state.value = {
    ...state.value,
    phase: "ready",
    rootUri: snapshot.rootUri,
    snapshot,
    preview: undefined,
    occurrences: undefined,
    occurrencesLoading: false,
    repairUri: undefined,
    message: "Configuration is up to date",
    refreshRequested: false,
  };
}

function beginPreview(state: Signal<ConfigurationEditorState>): void {
  state.value = {
    ...state.value,
    phase: "previewing",
    preview: undefined,
    message: "Calculating exact workspace impact…",
  };
}

function acceptPreview(state: Signal<ConfigurationEditorState>, preview: ConfigurationPreview): void {
  state.value = {
    ...state.value,
    phase: "preview",
    preview,
    message: `Preview ready: ${preview.changes.length} rule(s) change effective severity`,
  };
}

/**
 * Nothing was written, so the snapshot still describes the truth: return to it
 * and let every control re-render from it ([CONFIGEDITOR-VSIX-EXPERIENCE]).
 */
function cancelPreview(state: Signal<ConfigurationEditorState>): void {
  if (state.value.snapshot === undefined) { return; }
  if (state.value.phase !== "preview" && state.value.phase !== "previewing") { return; }
  state.value = {
    ...state.value,
    phase: "ready",
    preview: undefined,
    message: "Change discarded; configuration is unchanged",
  };
}

function failEditor(
  state: Signal<ConfigurationEditorState>,
  failure: { readonly message: string; readonly conflict: boolean; readonly repairUri: string | undefined },
): void {
  state.value = {
    ...state.value,
    phase: failure.conflict ? "conflict" : "error",
    occurrencesLoading: false,
    repairUri: failure.repairUri,
    message: failure.message,
  };
}

/** Build copy-on-write actions over the store's one configuration-editor Signal. */
export function createConfigurationEditorActions(
  state: Signal<ConfigurationEditorState>,
): ConfigurationEditorActions {
  return {
    beginConfigurationLoad(rootUri, focusRule): void { beginLoad(state, rootUri, focusRule); },
    acceptConfigurationSnapshot(snapshot): void { acceptSnapshot(state, snapshot); },
    beginConfigurationPreview(): void { beginPreview(state); },
    acceptConfigurationPreview(preview): void { acceptPreview(state, preview); },
    cancelConfigurationPreview(): void { cancelPreview(state); },
    beginConfigurationApply(): void {
      state.value = { ...state.value, phase: "applying", message: "Applying one validated workspace edit…" };
    },
    beginRuleOccurrences(reset): void {
      state.value = {
        ...state.value,
        occurrences: reset ? undefined : state.value.occurrences,
        occurrencesLoading: true,
      };
    },
    acceptRuleOccurrences(response, append): void {
      const items = append
        ? [...(state.value.occurrences?.items ?? []), ...response.items]
        : response.items;
      state.value = {
        ...state.value,
        occurrences: { items, nextCursor: response.nextCursor },
        occurrencesLoading: false,
      };
    },
    failRuleOccurrences(message): void {
      state.value = { ...state.value, occurrencesLoading: false, message };
    },
    failConfigurationEditor(message, conflict = false, repairUri): void {
      failEditor(state, { message, conflict, repairUri });
    },
    markConfigurationChanged(change): void { requestConfigurationRefresh(state, change); },
    markConfigurationUnsupported(message): void {
      state.value = {
        ...IDLE_CONFIGURATION_EDITOR,
        phase: "unsupported",
        rootUri: state.value.rootUri,
        message,
      };
    },
    resetConfigurationEditor(): void { state.value = IDLE_CONFIGURATION_EDITOR; },
  };
}
