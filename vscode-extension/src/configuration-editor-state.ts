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
  message: "",
  refreshRequested: false,
};

export interface ConfigurationEditorActions {
  beginConfigurationLoad(rootUri: string): void;
  acceptConfigurationSnapshot(snapshot: ConfigurationSnapshot): void;
  beginConfigurationPreview(): void;
  acceptConfigurationPreview(preview: ConfigurationPreview): void;
  beginConfigurationApply(): void;
  beginRuleOccurrences(): void;
  acceptRuleOccurrences(response: RuleOccurrencesResponse): void;
  failConfigurationEditor(message: string, conflict?: boolean): void;
  markConfigurationChanged(change: ConfigurationChanged): void;
  markConfigurationUnsupported(message: string): void;
  resetConfigurationEditor(): void;
}

export interface ConfigurationEditorStore extends ConfigurationEditorActions {
  readonly configurationEditor: ReadonlySignal<ConfigurationEditorState>;
}

/** Build copy-on-write actions over the store's one configuration-editor Signal. */
export function createConfigurationEditorActions(
  state: Signal<ConfigurationEditorState>,
): ConfigurationEditorActions {
  return {
    beginConfigurationLoad(rootUri: string): void {
      const sameRoot = state.value.rootUri === rootUri;
      state.value = {
        ...state.value,
        phase: "loading",
        rootUri,
        snapshot: sameRoot ? state.value.snapshot : undefined,
        preview: undefined,
        occurrences: sameRoot ? state.value.occurrences : undefined,
        message: "Reading the active project policy…",
        refreshRequested: false,
      };
    },
    acceptConfigurationSnapshot(snapshot: ConfigurationSnapshot): void {
      state.value = {
        ...state.value,
        phase: "ready",
        rootUri: snapshot.rootUri,
        snapshot,
        preview: undefined,
        message: "Configuration is up to date",
        refreshRequested: false,
      };
    },
    beginConfigurationPreview(): void {
      state.value = {
        ...state.value,
        phase: "previewing",
        preview: undefined,
        message: "Calculating exact workspace impact…",
      };
    },
    acceptConfigurationPreview(preview: ConfigurationPreview): void {
      state.value = {
        ...state.value,
        phase: "preview",
        preview,
        message: `Preview ready for ${preview.expandedRuleCodes.length} rule(s)`,
      };
    },
    beginConfigurationApply(): void {
      state.value = {
        ...state.value,
        phase: "applying",
        message: "Applying one validated workspace edit…",
      };
    },
    beginRuleOccurrences(): void {
      state.value = { ...state.value, occurrencesLoading: true };
    },
    acceptRuleOccurrences(response: RuleOccurrencesResponse): void {
      state.value = { ...state.value, occurrences: response, occurrencesLoading: false };
    },
    failConfigurationEditor(message: string, conflict = false): void {
      state.value = {
        ...state.value,
        phase: conflict ? "conflict" : "error",
        occurrencesLoading: false,
        message,
      };
    },
    markConfigurationChanged(change: ConfigurationChanged): void {
      if (state.value.rootUri !== change.rootUri) { return; }
      if (state.value.snapshot?.revision === change.revision) { return; }
      state.value = {
        ...state.value,
        message: change.reason,
        refreshRequested: true,
      };
    },
    markConfigurationUnsupported(message: string): void {
      state.value = { ...IDLE_CONFIGURATION_EDITOR, phase: "unsupported", message };
    },
    resetConfigurationEditor(): void {
      state.value = IDLE_CONFIGURATION_EDITOR;
    },
  };
}
