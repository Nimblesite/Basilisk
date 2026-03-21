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
import type { Result } from "./result";

/** Default timeout (ms) for waiting on the LSP client to become ready. */
export const DEFAULT_LSP_READY_TIMEOUT_MS = 1_000;

/** LSP lifecycle states exposed to consumers. */
export type LspState = "idle" | "starting" | "running" | "stopped";

/** Lifecycle promise handle for LSP client ready signaling. */
interface ReadyHandle {
  promise: Promise<void>;
  resolve: () => void;
}

export interface Store {
  // Read-only signals — consumers can .value but cannot assign.
  readonly client: ReadonlySignal<LanguageClient | undefined>;
  readonly serverCommands: ReadonlySignal<ReadonlySet<string>>;
  readonly clientCommands: ReadonlySignal<ReadonlySet<string>>;
  readonly statusBarItem: ReadonlySignal<vscode.StatusBarItem | undefined>;
  readonly outputChannel: ReadonlySignal<vscode.OutputChannel | undefined>;
  readonly logSink: ReadonlySignal<LogSink | undefined>;
  readonly lspState: ReadonlySignal<LspState>;
  readonly isServerReady: ReadonlySignal<boolean>;

  // Read-only access to the ready handle (for whenReady callers).
  readonly lspReadyPromise: ReadonlySignal<Promise<void> | undefined>;

  // Write actions — the only way to mutate state.
  setClient(context: vscode.ExtensionContext, c: LanguageClient): void;
  setStatusBarItem(item: vscode.StatusBarItem): void;
  setOutputChannel(ch: vscode.OutputChannel): void;
  setLogSink(sink: LogSink): void;
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
  outputChannel: Signal<vscode.OutputChannel | undefined>;
  logSink: Signal<LogSink | undefined>;
  lspState: Signal<LspState>;
  readyHandle: Signal<ReadyHandle | undefined>;
}

// ── Private helpers operating on StoreSignals ─────────────────────────────

/** Extract commands from the client's initializeResult. */
function syncServerCommands(signals: StoreSignals): void {
  const commands = signals.client.value?.initializeResult?.capabilities?.executeCommandProvider?.commands;
  if (!Array.isArray(commands)) {
    signals.serverCommands.value = new Set();
    return;
  }
  const next = new Set<string>();
  for (const cmd of commands) {
    if (typeof cmd === "string") {
      next.add(cmd);
    }
  }
  signals.serverCommands.value = next;
}

/** Resolve the ready handle and clear it. */
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

/** Register a client command with VS Code and track it. */
function registerCommand(
  reg: CommandRegistration,
  handler: (...args: unknown[]) => unknown
): void {
  if (reg.signals.clientCommands.value.has(reg.commandId)) {
    return;
  }
  const disposable = vscode.commands.registerCommand(reg.commandId, handler);
  reg.context.subscriptions.push(disposable);
  const next = new Set(reg.signals.clientCommands.value);
  next.add(reg.commandId);
  reg.signals.clientCommands.value = next;
}

/** Register all client-only commands. Called once when LSP reaches Running. */
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
      syncServerCommands(signals);
      registerClientCommands(signals, context);
      resolveLspReady(signals);
      return;
    }

    if (newState === State.Stopped) {
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
  const existing = signals.readyHandle.value;
  const ready = existing !== undefined ? existing.promise : createReadyHandle(signals).promise;
  const timeout = new Promise<"timeout">((resolve) => {
    setTimeout(() => { resolve("timeout"); }, timeoutMs);
  });
  const outcome = await Promise.race([ready.then(() => "ready" as const), timeout]);
  if (outcome === "timeout") {
    return { ok: false, error: new Error(`LSP client did not reach Running state within ${timeoutMs}ms`) };
  }
  const client = signals.client.value;
  if (client === undefined) {
    return { ok: false, error: new Error("LSP client resolved but is undefined") };
  }
  return { ok: true, value: client };
}

/** Reset all signals to their initial values. */
function resetSignals(signals: StoreSignals): void {
  signals.client.value = undefined;
  signals.serverCommands.value = new Set();
  signals.clientCommands.value = new Set();
  signals.statusBarItem.value = undefined;
  signals.outputChannel.value = undefined;
  signals.logSink.value = undefined;
  signals.lspState.value = "idle";
  signals.readyHandle.value = undefined;
}

// ── Factory ───────────────────────────────────────────────────────────────

export function createStore(): Store {
  const signals: StoreSignals = {
    client: signal<LanguageClient | undefined>(undefined),
    serverCommands: signal<ReadonlySet<string>>(new Set()),
    clientCommands: signal<ReadonlySet<string>>(new Set()),
    statusBarItem: signal<vscode.StatusBarItem | undefined>(undefined),
    outputChannel: signal<vscode.OutputChannel | undefined>(undefined),
    logSink: signal<LogSink | undefined>(undefined),
    lspState: signal<LspState>("idle"),
    readyHandle: signal<ReadyHandle | undefined>(undefined),
  };

  const isServerReady = computed(() => signals.client.value?.isRunning() === true);
  const lspReadyPromise = computed(async () => signals.readyHandle.value?.promise);

  return {
    client: signals.client as ReadonlySignal<LanguageClient | undefined>,
    serverCommands: signals.serverCommands as ReadonlySignal<ReadonlySet<string>>,
    clientCommands: signals.clientCommands as ReadonlySignal<ReadonlySet<string>>,
    statusBarItem: signals.statusBarItem as ReadonlySignal<vscode.StatusBarItem | undefined>,
    outputChannel: signals.outputChannel as ReadonlySignal<vscode.OutputChannel | undefined>,
    logSink: signals.logSink as ReadonlySignal<LogSink | undefined>,
    lspState: signals.lspState as ReadonlySignal<LspState>,
    lspReadyPromise,
    isServerReady,

    setClient(context: vscode.ExtensionContext, c: LanguageClient): void {
      signals.client.value = c;
      bindClientStateListener(signals, context, c);
    },
    setStatusBarItem(item: vscode.StatusBarItem): void {
      signals.statusBarItem.value = item;
    },
    setOutputChannel(ch: vscode.OutputChannel): void {
      signals.outputChannel.value = ch;
    },
    setLogSink(sink: LogSink): void {
      signals.logSink.value = sink;
    },
    isClientCommandRegistered(id: string): boolean {
      return signals.clientCommands.value.has(id);
    },
    isServerCommandAdvertised(id: string): boolean {
      return signals.serverCommands.value.has(id);
    },
    async ensureLspReadyPromise(timeoutMs = DEFAULT_LSP_READY_TIMEOUT_MS): Promise<Result<LanguageClient>> {
      return awaitLspReady(signals, timeoutMs);
    },
    reset(): void {
      resetSignals(signals);
    },
  };
}
