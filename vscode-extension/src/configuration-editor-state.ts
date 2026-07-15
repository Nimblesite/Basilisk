// Implements [VSIX-CONFIGURATION-EDITOR-THIN-SHELL].
/** Central immutable state and explicit actions for the configuration editor. */

import type { ReadonlySignal, Signal } from "@preact/signals-core";
import type {
  ConfigurationChanged,
  ConfigurationPreview,
  ConfigurationSnapshot,
  RuleOccurrencesResponse,
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
};

export interface ConfigurationEditorActions {
  beginConfigurationLoad(rootUri: string): void;
  acceptConfigurationSnapshot(snapshot: ConfigurationSnapshot): void;
  beginConfigurationPreview(): void;
  acceptConfigurationPreview(preview: ConfigurationPreview): void;
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

function beginLoad(state: Signal<ConfigurationEditorState>, rootUri: string): void {
  const sameRoot = state.value.rootUri === rootUri;
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
    beginConfigurationLoad(rootUri): void { beginLoad(state, rootUri); },
    acceptConfigurationSnapshot(snapshot): void { acceptSnapshot(state, snapshot); },
    beginConfigurationPreview(): void { beginPreview(state); },
    acceptConfigurationPreview(preview): void { acceptPreview(state, preview); },
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
