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

import { signal, computed, type ReadonlySignal, type Signal } from "@preact/signals-core";
import { type LanguageClient, State } from "vscode-languageclient/node";
import * as vscode from "vscode";
import { Logger, type LogSink } from "./logger";
import { createServerCommandHandler } from "./lsp-client";
import type { Result } from "./result";
import { POLL_INTERVAL_MS, WAIT_MS } from "./timeouts";
import {
  createProfilerActions,
  isProfilerBusy,
  IDLE_PROFILER_SESSION,
  type ProfilerActions,
  type ProfilerSession,
} from "./profiler-state";

/** LSP lifecycle states exposed to consumers. */
export type LspState = "idle" | "starting" | "running" | "stopped";

/** Runtime binary selected by Shipwright during activation. */
export interface RuntimeResolution {
  readonly componentId: string;
  readonly path: string;
  readonly source: string;
  readonly version: string | undefined;
}

/** Lifecycle promise handle for LSP client ready signaling. */
interface ReadyHandle {
  promise: Promise<void>;
  resolve: () => void;
}

export interface Store extends ProfilerActions {
  // Read-only signals — consumers can .value but cannot assign.
  readonly client: ReadonlySignal<LanguageClient | undefined>;
  readonly serverCommands: ReadonlySignal<ReadonlySet<string>>;
  readonly clientCommands: ReadonlySignal<ReadonlySet<string>>;
  readonly statusBarItem: ReadonlySignal<vscode.StatusBarItem | undefined>;
  readonly outputChannel: ReadonlySignal<vscode.LogOutputChannel | undefined>;
  readonly logSink: ReadonlySignal<LogSink | undefined>;
  readonly lspState: ReadonlySignal<LspState>;
  readonly isServerReady: ReadonlySignal<boolean>;
  /**
   * Monotonic counter that bumps whenever fresh analysis may be available:
   * the server reaches Running, `basilisk/moduleChanged` fires, or
   * diagnostics change (debounced). Panels subscribe via a signals `effect`
   * so they refresh automatically — never via per-panel polling
   * ([EXTACT-REACTIVE-STATE], issue #58).
   */
  readonly analysisRevision: ReadonlySignal<number>;
  readonly runtimeResolution: ReadonlySignal<RuntimeResolution | undefined>;
  /** Map of VS Code debug session id → debuggee OS process id (from the DAP `process` event). */
  readonly sessionIdToPid: ReadonlySignal<ReadonlyMap<string, number>>;
  /**
   * Canonical reactive profiling state ([PROFILE-PROCESSES-REACTIVE]). The CPU
   * status bar, the Python Processes panel, and the gating context keys all
   * subscribe to this one signal — mutate it only through the ProfilerActions.
   */
  readonly profiler: ReadonlySignal<ProfilerSession>;
  /** True while any CPU or memory profiling activity is starting or running. */
  readonly profilerBusy: ReadonlySignal<boolean>;

  // Read-only access to the ready handle (for whenReady callers).
  readonly lspReadyPromise: ReadonlySignal<Promise<void> | undefined>;

  // Write actions — the only way to mutate state.
  setClient(context: vscode.ExtensionContext, c: LanguageClient): void;
  setStatusBarItem(item: vscode.StatusBarItem): void;
  setOutputChannel(ch: vscode.LogOutputChannel): void;
  setLogSink(sink: LogSink): void;
  setRuntimeResolution(resolution: RuntimeResolution): void;
  /** Record the debuggee PID captured from a debug session's DAP `process` event. */
  setDebuggeeProcessId(sessionId: string, pid: number): void;
  /** Look up the debuggee PID for a debug session, or undefined if not yet known. */
  getDebuggeeProcessId(sessionId: string): number | undefined;
  /** Forget a debug session's PID mapping (called when the session terminates). */
  clearDebuggeeProcessId(sessionId: string): void;
  /** Signal that fresh analysis may be available ([EXTACT-REACTIVE-STATE]). */
  bumpAnalysisRevision(): void;
  isClientCommandRegistered(id: string): boolean;
  isServerCommandAdvertised(id: string): boolean;
  ensureLspReadyPromise(timeoutMs?: number): Promise<Result<LanguageClient>>;
  reset(): void;
}

/** Internal mutable signals backing the store. */
interface StoreSignals {
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
  readyHandle: Signal<ReadyHandle | undefined>;
  analysisRevision: Signal<number>;
  /** Trailing-debounce timer for diagnostics-driven analysisRevision bumps. */
  diagnosticsDebounce: ReturnType<typeof setTimeout> | undefined;
  /** Whether the global diagnostics listener has been registered. */
  diagnosticsListenerBound: boolean;
  /** Disposables for client-registered commands — disposed on LSP stop/restart. */
  commandDisposables: vscode.Disposable[];
  /** Disposables for server-advertised command registrations — disposed on LSP stop/restart. */
  serverCommandDisposables: vscode.Disposable[];
}

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

/** Resolve the ready handle and clear it.
 *  Resolution MUST be async (next tick) so callers' .then() handlers
 *  are attached before the promise settles. */
function resolveLspReady(signals: StoreSignals): void {
  const handle = signals.readyHandle.value;
  if (handle !== undefined) {
    signals.readyHandle.value = undefined;
    setTimeout(handle.resolve, 0);
  }
}

/** Create a fresh ready handle for this start cycle. */
function createReadyHandle(signals: StoreSignals): ReadyHandle {
  let resolve: (() => void) | undefined;
  const promise = new Promise<void>((r) => { resolve = r; });
  const handle: ReadyHandle = { promise, resolve: resolve as () => void };
  signals.readyHandle.value = handle;
  return handle;
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
      registerClientCommands(signals, context);
      // Re-analysis completion: registered after disposeAllCommands each
      // Running cycle so restarts re-bind it ([EXTACT-REACTIVE-STATE]).
      signals.commandDisposables.push(
        lspClient.onNotification("basilisk/moduleChanged", () => {
          bumpAnalysisRevision(signals);
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

/** Wait for the LSP ready handle with a timeout, returning Result. */
async function awaitLspReady(signals: StoreSignals, timeoutMs: number): Promise<Result<LanguageClient>> {
  // Fast path: client already running.
  const client = signals.client.value;
  if (client?.isRunning() === true) {
    return { ok: true, value: client };
  }
  // Also check our own state signal (catches post-restart where isRunning()
  // lags behind the onDidChangeState callback that set lspState = "running").
  if (signals.lspState.value === "running" && client !== undefined) {
    return { ok: true, value: client };
  }

  const existing = signals.readyHandle.value;
  const ready = existing !== undefined ? existing.promise : createReadyHandle(signals).promise;

  // Poll for the client becoming ready via both isRunning() and our own
  // lspState signal. The double check catches cases where the readyHandle
  // was resolved before this function was called (e.g. after a deactivate/
  // activate cycle where the state listener already fired).
  const poll = new Promise<"poll">((resolve) => {
    const interval = setInterval(() => {
      const c = signals.client.value;
      if (c?.isRunning() === true || (signals.lspState.value === "running" && c !== undefined)) {
        clearInterval(interval);
        resolve("poll");
      }
    }, POLL_INTERVAL_MS);
    setTimeout(() => { clearInterval(interval); }, timeoutMs);
  });

  const timeout = new Promise<"timeout">((resolve) => {
    setTimeout(() => { resolve("timeout"); }, timeoutMs);
  });
  const outcome = await Promise.race([ready.then(() => "ready" as const), poll, timeout]);
  if (outcome === "timeout") {
    return { ok: false, error: new Error(`LSP client did not reach Running state within ${timeoutMs}ms`) };
  }
  const resolved = signals.client.value;
  if (resolved === undefined) {
    return { ok: false, error: new Error("LSP client resolved but is undefined") };
  }
  return { ok: true, value: resolved };
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
    lspReadyPromise,
    isServerReady,
    analysisRevision: signals.analysisRevision,
    ...createProfilerActions(signals.profiler),

    setClient(context: vscode.ExtensionContext, c: LanguageClient): void {
      signals.client.value = c;
      bindClientStateListener(signals, context, c);
      bindDiagnosticsListener(signals, context);
    },
    bumpAnalysisRevision(): void {
      bumpAnalysisRevision(signals);
    },
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
      disposeAllCommands(signals);
      resetSignals(signals);
      onReset?.();
    },
  };
}
