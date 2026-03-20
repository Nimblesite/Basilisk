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

import { signal, computed, type ReadonlySignal } from "@preact/signals-core";
import type { LanguageClient } from "vscode-languageclient/node";
import type * as vscode from "vscode";
import type { LogSink } from "./logger";

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
  setClient(c: LanguageClient): void;
  setStatusBarItem(item: vscode.StatusBarItem): void;
  setOutputChannel(ch: vscode.OutputChannel): void;
  setLogSink(sink: LogSink): void;
  isClientCommandRegistered(id: string): boolean;
  isServerCommandAdvertised(id: string): boolean;
  ensureLspReadyPromise(): Promise<void>;
  reset(): void;
}

export function createStore(): Store {
  const _client = signal<LanguageClient | undefined>(undefined);
  const _serverCommands = signal<ReadonlySet<string>>(new Set());
  const _clientCommands = signal<ReadonlySet<string>>(new Set());
  const _statusBarItem = signal<vscode.StatusBarItem | undefined>(undefined);
  const _outputChannel = signal<vscode.OutputChannel | undefined>(undefined);
  const _logSink = signal<LogSink | undefined>(undefined);
  const _lspState = signal<LspState>("idle");
  const _readyHandle = signal<ReadyHandle | undefined>(undefined);

  const isServerReady = computed(() => _client.value?.isRunning() === true);
  const lspReadyPromise = computed(() => _readyHandle.value?.promise);

  /** Extract commands from the client's initializeResult. Private. */
  function syncServerCommands(): void {
    const commands = _client.value?.initializeResult?.capabilities?.executeCommandProvider?.commands;
    if (!Array.isArray(commands)) {
      _serverCommands.value = new Set();
      return;
    }
    const next = new Set<string>();
    for (const cmd of commands) {
      if (typeof cmd === "string") {
        next.add(cmd);
      }
    }
    _serverCommands.value = next;
  }

  /** Resolve the ready handle and clear it. Private. */
  function resolveLspReady(): void {
    const handle = _readyHandle.value;
    if (handle !== undefined) {
      _readyHandle.value = undefined;
      setTimeout(handle.resolve, 0);
    }
  }

  /** Create a fresh ready handle for this start cycle. Private. */
  function createReadyHandle(): ReadyHandle {
    let resolve: () => void;
    const promise = new Promise<void>((r) => { resolve = r; });
    const handle: ReadyHandle = { promise, resolve: resolve! };
    _readyHandle.value = handle;
    return handle;
  }

  /**
   * Wire up the onDidChangeState listener on the given client.
   * This is the ONLY place server commands get populated.
   */
  function bindClientStateListener(lspClient: LanguageClient): void {
    lspClient.onDidChangeState((event) => {
      // Dynamically import State to avoid pulling vscode-languageclient
      // into the module scope (it's already imported by the caller).
      // State enum values: Starting=1, Running=2, Stopped=3
      const newState = event.newState;

      // State.Running === 2
      if (newState === 2) {
        _lspState.value = "running";
        syncServerCommands();
        resolveLspReady();
        return;
      }

      // State.Stopped === 3
      if (newState === 3) {
        _lspState.value = "stopped";
        return;
      }

      // State.Starting === 1
      if (newState === 1) {
        _lspState.value = "starting";
        if (_readyHandle.value === undefined) {
          createReadyHandle();
        }
      }
    });
  }

  return {
    client: _client as ReadonlySignal<LanguageClient | undefined>,
    serverCommands: _serverCommands as ReadonlySignal<ReadonlySet<string>>,
    clientCommands: _clientCommands as ReadonlySignal<ReadonlySet<string>>,
    statusBarItem: _statusBarItem as ReadonlySignal<vscode.StatusBarItem | undefined>,
    outputChannel: _outputChannel as ReadonlySignal<vscode.OutputChannel | undefined>,
    logSink: _logSink as ReadonlySignal<LogSink | undefined>,
    lspState: _lspState as ReadonlySignal<LspState>,
    lspReadyPromise,
    isServerReady,

    setClient(c: LanguageClient): void {
      _client.value = c;
      bindClientStateListener(c);
    },
    setStatusBarItem(item: vscode.StatusBarItem): void {
      _statusBarItem.value = item;
    },
    setOutputChannel(ch: vscode.OutputChannel): void {
      _outputChannel.value = ch;
    },
    setLogSink(sink: LogSink): void {
      _logSink.value = sink;
    },
    isClientCommandRegistered(id: string): boolean {
      return _clientCommands.value.has(id);
    },
    isServerCommandAdvertised(id: string): boolean {
      return _serverCommands.value.has(id);
    },
    ensureLspReadyPromise(): Promise<void> {
      const existing = _readyHandle.value;
      return existing !== undefined ? existing.promise : createReadyHandle().promise;
    },
    reset(): void {
      _client.value = undefined;
      _serverCommands.value = new Set();
      _clientCommands.value = new Set();
      _statusBarItem.value = undefined;
      _outputChannel.value = undefined;
      _logSink.value = undefined;
      _lspState.value = "idle";
      _readyHandle.value = undefined;
    },
  };
}
