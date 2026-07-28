// Implements [VSIX-ARCHITECTURE]. See docs/specs/VSIX-SPEC.md#VSIX-ARCHITECTURE
/** Public and mutable backing types for the centralized extension store. */

import type { ReadonlySignal, Signal } from "@preact/signals-core";
import type { LanguageClient } from "vscode-languageclient/node";
import type * as vscode from "vscode";
import type {
  ConfigurationEditorActions,
  ConfigurationEditorState,
} from "./configuration-editor-state";
import type { TypeshedStatusState } from "./configuration-editor-model";
import type { LogSink } from "./logger";
import type { ProcessPanelActions, ProcessPanelState } from "./processes-state";
import type { ProfilerActions, ProfilerSession } from "./profiler-state";
import type { Result } from "./result";
import type { LspState, ReadyHandle } from "./store-ready";

/**
 * The slice of the extension context a collaborator needs to own disposables.
 *
 * Depending on the whole `ExtensionContext` makes every caller — tests
 * included — responsible for producing all seventeen of its members, none of
 * which this code path reads. Naming the one member that is actually used
 * keeps the dependency honest and lets a caller hand over exactly that.
 */
export interface DisposableSink {
  readonly subscriptions: { dispose(): unknown }[];
}

/**
 * The slice of the extension context that persists per-workspace UI state.
 *
 * `get` returns `unknown`, not a caller-chosen `T`. The value comes back from
 * storage a previous version of the extension wrote, so a `get<ViewMode>(…)`
 * would be an unchecked assertion dressed as a generic — the compiler would
 * vouch for a shape nothing verified. Callers narrow what they read.
 */
export interface WorkspaceStateStore {
  readonly workspaceState: {
    get(key: string): unknown;
    update(key: string, value: unknown): Thenable<void>;
  };
}

/** Runtime binary selected by Shipwright during activation. */
export interface RuntimeResolution {
  readonly componentId: string;
  readonly path: string;
  readonly source: string;
  readonly version: string | undefined;
}

export interface Store extends ProfilerActions, ProcessPanelActions, ConfigurationEditorActions {
  readonly client: ReadonlySignal<LanguageClient | undefined>;
  readonly serverCommands: ReadonlySignal<ReadonlySet<string>>;
  readonly clientCommands: ReadonlySignal<ReadonlySet<string>>;
  readonly statusBarItem: ReadonlySignal<vscode.StatusBarItem | undefined>;
  readonly outputChannel: ReadonlySignal<vscode.LogOutputChannel | undefined>;
  readonly logSink: ReadonlySignal<LogSink | undefined>;
  readonly lspState: ReadonlySignal<LspState>;
  readonly isServerReady: ReadonlySignal<boolean>;
  readonly analysisRevision: ReadonlySignal<number>;
  readonly runtimeResolution: ReadonlySignal<RuntimeResolution | undefined>;
  readonly sessionIdToPid: ReadonlySignal<ReadonlyMap<string, number>>;
  readonly profiler: ReadonlySignal<ProfilerSession>;
  readonly profilerBusy: ReadonlySignal<boolean>;
  readonly cpuBusy: ReadonlySignal<boolean>;
  readonly memoryBusy: ReadonlySignal<boolean>;
  readonly processes: ReadonlySignal<ProcessPanelState>;
  readonly processesRevision: ReadonlySignal<number>;
  readonly configurationEditor: ReadonlySignal<ConfigurationEditorState>;
  readonly typeshedStatuses: ReadonlySignal<ReadonlyMap<string, TypeshedStatusState>>;
  readonly lspReadyPromise: ReadonlySignal<Promise<void> | undefined>;

  setClient(context: DisposableSink, client: LanguageClient): void;
  setStatusBarItem(item: vscode.StatusBarItem): void;
  setOutputChannel(channel: vscode.LogOutputChannel): void;
  setLogSink(sink: LogSink): void;
  setRuntimeResolution(resolution: RuntimeResolution): void;
  setDebuggeeProcessId(sessionId: string, pid: number): void;
  getDebuggeeProcessId(sessionId: string): number | undefined;
  clearDebuggeeProcessId(sessionId: string): void;
  bumpAnalysisRevision(): void;
  isClientCommandRegistered(id: string): boolean;
  isServerCommandAdvertised(id: string): boolean;
  ensureLspReadyPromise(timeoutMs?: number): Promise<Result<LanguageClient>>;
  reset(): void;
}

/** Internal mutable signals backing one store instance. */
export interface StoreSignals {
  client: Signal<LanguageClient | undefined>;
  serverCommands: Signal<ReadonlySet<string>>;
  clientCommands: Signal<ReadonlySet<string>>;
  statusBarItem: Signal<vscode.StatusBarItem | undefined>;
  outputChannel: Signal<vscode.LogOutputChannel | undefined>;
  logSink: Signal<LogSink | undefined>;
  lspState: Signal<LspState>;
  runtimeResolution: Signal<RuntimeResolution | undefined>;
  sessionIdToPid: Signal<Map<string, number>>;
  profiler: Signal<ProfilerSession>;
  processes: Signal<ProcessPanelState>;
  configurationEditor: Signal<ConfigurationEditorState>;
  typeshedStatuses: Signal<ReadonlyMap<string, TypeshedStatusState>>;
  readyHandle: Signal<ReadyHandle | undefined>;
  analysisRevision: Signal<number>;
  diagnosticsDebounce: ReturnType<typeof setTimeout> | undefined;
  diagnosticsListenerBound: boolean;
  commandDisposables: vscode.Disposable[];
  serverCommandDisposables: vscode.Disposable[];
}
