// Implements [VSIX-ARCHITECTURE]. See docs/specs/VSIX-SPEC.md#VSIX-ARCHITECTURE
/**
 * Centralized, immutable-by-default application state for the Basilisk
 * VS Code extension.
 *
 * All mutable state lives here. Consumers receive ReadonlySignals and
 * can only mutate through explicit actions on the Store. Call reset()
 * to blow away all state (deactivation, test teardown, server restart).
 *
 * The LSP state listener lives inside the store — server commands can
 * only be populated from the onDidChangeState callback. No external
 * code can add or replace server commands.
 *
 * Factory function — not a singleton. Tests create their own store,
 * production creates one in activate().
 */

import { arrayField, recordField } from "./unknown-shape";
import { signal, computed } from "@preact/signals-core";
import { type LanguageClient, State } from "vscode-languageclient/node";
import * as vscode from "vscode";
import { Logger, type LogSink } from "./logger";
import { createServerCommandHandler } from "./lsp-client";
import type { Result } from "./result";
import { WAIT_MS } from "./timeouts";
import {
  createProfilerActions,
  isProfilerBusy,
  isCpuBusy,
  isMemoryBusy,
  IDLE_PROFILER_SESSION,
  type ProfilerSession,
} from "./profiler-state";
import {
  createProcessPanelActions,
  IDLE_PROCESS_PANEL,
  type ProcessPanelState,
} from "./processes-state";
import {
  createConfigurationEditorActions,
  decodeConfigurationChanged,
  decodeTypeshedStatusChanged,
  IDLE_CONFIGURATION_EDITOR,
  requestConfigurationRefresh,
  requestTypeshedStatusRefresh,
  type ConfigurationEditorState,
} from "./configuration-editor-state";
import type { TypeshedStatusState } from "./configuration-editor-model";
import {
  awaitLspReady,
  createReadyHandle,
  resolveLspReady,
  type LspState,
  type ReadyHandle,
} from "./store-ready";
import type { RuntimeResolution, Store, StoreSignals } from "./store-types";

// Re-exported so consumers keep importing the LSP lifecycle type from the store.
export { type LspState } from "./store-ready";
export type { RuntimeResolution, Store } from "./store-types";

// ── Private helpers operating on StoreSignals ─────────────────────────────

/** Dispose all server-advertised command registrations. */
function disposeServerCommands(signals: StoreSignals): void {
  for (const d of signals.serverCommandDisposables) {
    d.dispose();
  }
  signals.serverCommandDisposables = [];
}

/**
 * Extract commands from the client's initializeResult and register them
 * with VS Code so they appear in getCommands() and the command palette.
 *
 * Each command handler executes the command through the LSP client via
 * workspace/executeCommand. This replaces the vscode-languageclient
 * ExecuteCommandFeature which was removed to prevent double-registration.
 */
function syncServerCommands(signals: StoreSignals): void {
  disposeServerCommands(signals);

  const client = signals.client.value;
  const commands = client?.initializeResult?.capabilities?.executeCommandProvider?.commands;
  if (!Array.isArray(commands) || client === undefined) {
    signals.serverCommands.value = new Set();
    return;
  }

  const next = new Set<string>();
  for (const cmd of commands) {
    if (typeof cmd === "string") {
      next.add(cmd);
      const handler = createServerCommandHandler(client, cmd);
      const disposable = vscode.commands.registerCommand(cmd, handler);
      signals.serverCommandDisposables.push(disposable);
    }
  }
  signals.serverCommands.value = next;
}

interface CommandRegistration {
  signals: StoreSignals;
  context: vscode.ExtensionContext;
  commandId: string;
}

/**
 * Register a client command with VS Code and track it.
 *
 * The disposable is stored ONLY in commandDisposables — NOT in
 * context.subscriptions. Rationale: disposeClientCommands() calls
 * dispose() on every entry when the LSP restarts or stops. If the
 * same disposable also lived in context.subscriptions, deactivate()
 * would dispose it a second time (double-dispose). Per the VS Code
 * API, registerCommand returns a Disposable whose dispose() method
 * unregisters the command — calling it twice is undefined behaviour.
 */
function registerCommand(
  reg: CommandRegistration,
  handler: (...args: unknown[]) => unknown
): void {
  const disposable = vscode.commands.registerCommand(reg.commandId, handler);
  reg.signals.commandDisposables.push(disposable);
  const next = new Set(reg.signals.clientCommands.value);
  next.add(reg.commandId);
  reg.signals.clientCommands.value = next;
}

/** Dispose all registered commands (client AND server) so they can be re-registered fresh. */
function disposeAllCommands(signals: StoreSignals): void {
  for (const d of signals.commandDisposables) {
    d.dispose();
  }
  signals.commandDisposables = [];
  signals.clientCommands.value = new Set();
  disposeServerCommands(signals);
  signals.serverCommands.value = new Set();
}

// Implements [VSIX-COMMANDS] — registers the two client-only commands declared in
// vscode-extension/package.json (contributes.commands): basilisk.restartServer
// ([VSIX-ERROR-RECOVERY] manual recovery) and basilisk.showOutput
// ([VSIX-OUTPUT-CHANNELS]). All other contributed commands are server-advertised
// and auto-registered via syncServerCommands (per the Command Registration Rule).
/** Register all client-only commands. Called when LSP reaches Running. */
function registerClientCommands(signals: StoreSignals, context: vscode.ExtensionContext): void {
  registerCommand({ signals, context, commandId: "basilisk.restartServer" }, async () => {
    const lspClient = signals.client.value;
    if (!lspClient) {
      vscode.window.showWarningMessage("Basilisk: No language server to restart.");
      return;
    }
    try {
      Logger.info("Restarting Basilisk language server...");
      await lspClient.stop();
      await lspClient.start();
      Logger.info("Basilisk language server restarted.");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      Logger.error(`Restart failed: ${msg}`);
      vscode.window.showErrorMessage(`Basilisk: Failed to restart server: ${msg}`);
    }
  });

  registerCommand({ signals, context, commandId: "basilisk.showOutput" }, () => {
    signals.outputChannel.value?.show();
  });
}

/** Bump the analysis revision ([EXTACT-REACTIVE-STATE], issue #58). */
function bumpAnalysisRevision(signals: StoreSignals): void {
  signals.analysisRevision.value += 1;
}

function replaceInitialTypeshedStatuses(
  signals: StoreSignals,
  client: LanguageClient,
): void {
  const basilisk = recordField(client.initializeResult?.capabilities.experimental, "basilisk");
  const next = new Map<string, TypeshedStatusState>();
  for (const value of arrayField(basilisk, "typeshedStatuses")) {
    const change = decodeTypeshedStatusChanged(value);
    if (change !== undefined) { next.set(change.rootUri, change.status); }
  }
  signals.typeshedStatuses.value = next;
}

function upsertTypeshedStatus(signals: StoreSignals, value: unknown): void {
  const change = decodeTypeshedStatusChanged(value);
  if (change === undefined) { return; }
  const next = new Map(signals.typeshedStatuses.value);
  next.set(change.rootUri, change.status);
  signals.typeshedStatuses.value = next;
  requestTypeshedStatusRefresh(signals.configurationEditor, change);
}

/** Debounce window for diagnostics-driven refreshes (diagnostics fire per file). */
const DIAGNOSTICS_BUMP_DEBOUNCE_MS = 300;

/**
 * Register the global diagnostics listener exactly once per store: any
 * diagnostics change bumps the analysis revision (debounced) so panels that
 * render health rollups stay live ([EXTACT-HEALTH-REFRESH]).
 */
function bindDiagnosticsListener(signals: StoreSignals, context: vscode.ExtensionContext): void {
  if (signals.diagnosticsListenerBound) { return; }
  signals.diagnosticsListenerBound = true;
  context.subscriptions.push(
    vscode.languages.onDidChangeDiagnostics(() => {
      if (signals.diagnosticsDebounce !== undefined) {
        clearTimeout(signals.diagnosticsDebounce);
      }
      signals.diagnosticsDebounce = setTimeout(() => {
        signals.diagnosticsDebounce = undefined;
        bumpAnalysisRevision(signals);
      }, DIAGNOSTICS_BUMP_DEBOUNCE_MS);
    }),
  );
}

/**
 * Wire up the onDidChangeState listener on the given client.
 * This is the ONLY place commands get registered or populated.
 */
function bindClientStateListener(
  signals: StoreSignals,
  context: vscode.ExtensionContext,
  lspClient: LanguageClient
): void {
  lspClient.onDidChangeState((event) => {
    const newState = event.newState;

    if (newState === State.Running) {
      signals.lspState.value = "running";
      disposeAllCommands(signals);
      syncServerCommands(signals);
      replaceInitialTypeshedStatuses(signals, lspClient);
      registerClientCommands(signals, context);
      // Implements the client side of [EXTACT-LSP-COMMANDS-MODULE-CHANGED] —
      // consumes the server's `basilisk/moduleChanged` notification (re-analysis
      // complete) to bump the analysis revision. Registered after
      // disposeAllCommands each Running cycle so restarts re-bind it
      // ([EXTACT-REACTIVE-STATE]).
      signals.commandDisposables.push(
        lspClient.onNotification("basilisk/moduleChanged", () => {
          bumpAnalysisRevision(signals);
        }),
        // Implements the client side of [EXTACT-LSP-COMMANDS-SCAN-COMPLETE]
        // (#144): the workspace scan finishing must repaint panels even when
        // it published nothing (a genuinely empty workspace produces no
        // diagnostics events), so the loading state can settle into the
        // honest empty-state.
        lspClient.onNotification("basilisk/scanComplete", () => {
          bumpAnalysisRevision(signals);
        }),
        // An applied configuration change rewrites the severity of every
        // affected diagnostic, so panels rendering health rollups are stale the
        // moment the server confirms it. The debounced diagnostics listener
        // cannot be relied on here: a severity-only recheck can republish
        // nothing the client observes, leaving the Modules panel showing the
        // PREVIOUS configuration ([EXTACT-REACTIVE-STATE] / [LSPARCH-CONFIG]).
        lspClient.onNotification("basilisk/configurationChanged", (value: unknown) => {
          const change = decodeConfigurationChanged(value);
          if (change !== undefined) {
            requestConfigurationRefresh(signals.configurationEditor, change);
            bumpAnalysisRevision(signals);
          }
        }),
        lspClient.onNotification("basilisk/typeshedStatusChanged", (value: unknown) => {
          upsertTypeshedStatus(signals, value);
        }),
      );
      resolveLspReady(signals);
      // Initial analysis becomes available once the server runs.
      bumpAnalysisRevision(signals);
      return;
    }

    if (newState === State.Stopped) {
      disposeAllCommands(signals);
      signals.lspState.value = "stopped";
      signals.typeshedStatuses.value = new Map();
      return;
    }

    if (newState === State.Starting) {
      signals.lspState.value = "starting";
      if (signals.readyHandle.value === undefined) {
        createReadyHandle(signals);
      }
    }
  });
}

/** Reset all signals to their initial values. */
function resetSignals(signals: StoreSignals): void {
  // Per-session client state — cleared on every reset.
  signals.client.value = undefined;
  signals.serverCommands.value = new Set();
  signals.clientCommands.value = new Set();
  signals.statusBarItem.value = undefined;
  signals.lspState.value = "idle";
  signals.runtimeResolution.value = undefined;
  signals.sessionIdToPid.value = new Map();
  signals.profiler.value = IDLE_PROFILER_SESSION;
  signals.processes.value = IDLE_PROCESS_PANEL;
  signals.configurationEditor.value = IDLE_CONFIGURATION_EDITOR;
  signals.typeshedStatuses.value = new Map();
  signals.readyHandle.value = undefined;
  // outputChannel and logSink are stable logging infrastructure created once by
  // initLogging (and owned by context.subscriptions) — they are deliberately
  // NOT cleared here. The onReset → startRuntime restart path reuses the store
  // without re-running initLogging, so a restarted LanguageClient must still
  // receive the existing channel. Clearing it would hand the client
  // `outputChannel: undefined`, making vscode-languageclient 10 create and OWN
  // an internal LogOutputChannel and dispose it on the next stop() — after
  // which the server's stderr readline (which the client never tears down)
  // drains a final line into the disposed channel and throws
  // "Channel has been closed".
}

/** Plain-value setters for logging/runtime infrastructure — extracted to keep createStore small. */
function infrastructureSetters(signals: StoreSignals): Pick<
  Store,
  "setStatusBarItem" | "setOutputChannel" | "setLogSink" | "setRuntimeResolution"
> {
  return {
    setStatusBarItem(item: vscode.StatusBarItem): void {
      signals.statusBarItem.value = item;
    },
    setOutputChannel(ch: vscode.LogOutputChannel): void {
      signals.outputChannel.value = ch;
    },
    setLogSink(sink: LogSink): void {
      signals.logSink.value = sink;
    },
    setRuntimeResolution(resolution: RuntimeResolution): void {
      signals.runtimeResolution.value = resolution;
    },
  };
}

/** Debuggee PID actions (copy-on-write Map) — extracted to keep createStore small. */
function debuggeePidActions(signals: StoreSignals): Pick<
  Store,
  "setDebuggeeProcessId" | "getDebuggeeProcessId" | "clearDebuggeeProcessId"
> {
  return {
    setDebuggeeProcessId(sessionId: string, pid: number): void {
      const next = new Map(signals.sessionIdToPid.value);
      next.set(sessionId, pid);
      signals.sessionIdToPid.value = next;
    },
    getDebuggeeProcessId(sessionId: string): number | undefined {
      return signals.sessionIdToPid.value.get(sessionId);
    },
    clearDebuggeeProcessId(sessionId: string): void {
      if (!signals.sessionIdToPid.value.has(sessionId)) { return; }
      const next = new Map(signals.sessionIdToPid.value);
      next.delete(sessionId);
      signals.sessionIdToPid.value = next;
    },
  };
}

// ── Factory ───────────────────────────────────────────────────────────────

/** Build the fresh, mutable signal bag backing a store. */
function createStoreSignals(): StoreSignals {
  return {
    client: signal<LanguageClient | undefined>(undefined),
    serverCommands: signal<ReadonlySet<string>>(new Set()),
    clientCommands: signal<ReadonlySet<string>>(new Set()),
    statusBarItem: signal<vscode.StatusBarItem | undefined>(undefined),
    outputChannel: signal<vscode.LogOutputChannel | undefined>(undefined),
    logSink: signal<LogSink | undefined>(undefined),
    lspState: signal<LspState>("idle"),
    runtimeResolution: signal<RuntimeResolution | undefined>(undefined),
    sessionIdToPid: signal<Map<string, number>>(new Map()),
    profiler: signal<ProfilerSession>(IDLE_PROFILER_SESSION),
    processes: signal<ProcessPanelState>(IDLE_PROCESS_PANEL),
    configurationEditor: signal<ConfigurationEditorState>(IDLE_CONFIGURATION_EDITOR),
    typeshedStatuses: signal(new Map()),
    readyHandle: signal<ReadyHandle | undefined>(undefined),
    analysisRevision: signal<number>(0),
    diagnosticsDebounce: undefined,
    diagnosticsListenerBound: false,
    commandDisposables: [],
    serverCommandDisposables: [],
  };
}

export function createStore(onReset?: () => void): Store {
  const signals: StoreSignals = createStoreSignals();

  const isServerReady = computed(() => signals.client.value?.isRunning() === true);
  const lspReadyPromise = computed(async () => signals.readyHandle.value?.promise);
  const profilerBusy = computed(() => isProfilerBusy(signals.profiler.value));
  return {
    client: signals.client,
    serverCommands: signals.serverCommands,
    clientCommands: signals.clientCommands,
    statusBarItem: signals.statusBarItem,
    outputChannel: signals.outputChannel,
    logSink: signals.logSink,
    lspState: signals.lspState,
    runtimeResolution: signals.runtimeResolution,
    sessionIdToPid: signals.sessionIdToPid,
    profiler: signals.profiler,
    profilerBusy,
    cpuBusy: computed(() => isCpuBusy(signals.profiler.value)),
    memoryBusy: computed(() => isMemoryBusy(signals.profiler.value)),
    processes: signals.processes,
    processesRevision: computed(() => signals.processes.value.revision),
    configurationEditor: signals.configurationEditor,
    typeshedStatuses: signals.typeshedStatuses,
    lspReadyPromise,
    isServerReady,
    analysisRevision: signals.analysisRevision,
    ...createProfilerActions(signals.profiler),
    ...createProcessPanelActions(signals.processes),
    ...createConfigurationEditorActions(signals.configurationEditor),

    setClient(context: vscode.ExtensionContext, c: LanguageClient): void {
      signals.client.value = c;
      bindClientStateListener(signals, context, c);
      bindDiagnosticsListener(signals, context);
    },
    bumpAnalysisRevision: (): void => { bumpAnalysisRevision(signals); },
    ...infrastructureSetters(signals),
    ...debuggeePidActions(signals),
    isClientCommandRegistered(id: string): boolean {
      return signals.clientCommands.value.has(id);
    },
    isServerCommandAdvertised(id: string): boolean {
      return signals.serverCommands.value.has(id);
    },
    async ensureLspReadyPromise(timeoutMs = WAIT_MS): Promise<Result<LanguageClient>> {
      return awaitLspReady(signals, timeoutMs);
    },
    reset(): void {
      // Stop the client being replaced BEFORE resetSignals drops the
      // reference. A reset that only forgets the client leaves it fully
      // alive — a zombie that keeps forwarding didOpen/didClose to its own
      // server process and publishing into its own diagnostics collection,
      // which VS Code merges into getDiagnostics(). Its late republishes
      // then resurrect diagnostics the real server already cleared
      // (GitHub #264). dispose() also tears the collection down; skip it
      // when a stop is already in flight (the deactivate() path).
      const dyingClient = signals.client.value;
      if (dyingClient?.isRunning() === true) {
        dyingClient.dispose().catch((err: unknown) => {
          Logger.warn(`Failed to dispose replaced LSP client: ${String(err)}`);
        });
      }
      disposeAllCommands(signals);
      resetSignals(signals);
      onReset?.();
    },
  };
}
