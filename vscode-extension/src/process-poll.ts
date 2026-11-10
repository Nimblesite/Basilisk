// Implements [PROFILE-PROCESSES-REACTIVE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-REACTIVE
/**
 * The store-side source of the Python Processes list (#148).
 *
 * The OS has no push event for "a Python process appeared", so the list is
 * genuinely poll-sourced — but per the architecture the poll belongs HERE,
 * feeding the store's `processes` Signal, never inside the panel: the tree
 * provider stays a pure projection of centralised state and owns no timer.
 * Polling runs only while the panel is visible, so navigating away stops the
 * LSP round-trips.
 */

import * as vscode from "vscode";
import { effect } from "@preact/signals-core";
import { Logger } from "./logger";
import type { Store } from "./store";
import type { ProcessInfo } from "./process-explorer-rows";

/** LSP command name (must match basilisk-common constants). */
const LSP_CMD_PROCESSES = "basilisk.profiler.processes";

/** Wire shape of the LSP process listing. */
interface ProcessesResponse {
  readonly processes: readonly ProcessInfo[];
}

/** Default poll interval (ms) when the setting is absent. */
const DEFAULT_REFRESH_MS = 2000;

/** Context key gating the Python Processes welcome states (#147). */
const PROCESSES_STATE_CONTEXT_KEY = "basilisk.processesState";

/**
 * Fetch the process table from the LSP into the store's `processes` Signal.
 * Every outcome lands honestly ([PROFILE-PROCESSES-PANEL], #147): no running
 * client stays "loading" (the serverState welcome owns the copy), a failure is
 * "error", and only a successful fetch may render "No Python processes running".
 */
export async function fetchProcessesIntoStore(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    store.processesLoading();
    return;
  }
  try {
    const result = await client.sendRequest<ProcessesResponse>(
      "workspace/executeCommand",
      { command: LSP_CMD_PROCESSES, arguments: [{}] },
    );
    store.processesLoaded(result?.processes ?? []);
  } catch (err: unknown) {
    Logger.error(`Python Processes fetch failed: ${err instanceof Error ? err.message : String(err)}`);
    store.processesFetchFailed();
  }
}

/**
 * Poll the LSP into the store while the panel is visible: an immediate fetch on
 * becoming visible (no blank 2-second first paint), then one per interval. The
 * returned disposable stops the timer and the visibility subscription.
 */
export function bindProcessPolling(
  store: Store,
  treeView: Pick<vscode.TreeView<unknown>, "visible" | "onDidChangeVisibility">,
): vscode.Disposable {
  let timer: ReturnType<typeof setInterval> | undefined;

  function start(): void {
    if (timer !== undefined) { return; }
    const intervalMs = vscode.workspace
      .getConfiguration("basilisk")
      .get<number>("profiler.processRefreshMs", DEFAULT_REFRESH_MS);
    void fetchProcessesIntoStore(store);
    timer = setInterval(() => { void fetchProcessesIntoStore(store); }, intervalMs);
  }
  function stop(): void {
    if (timer !== undefined) { clearInterval(timer); timer = undefined; }
  }

  const subscription = treeView.onDidChangeVisibility((e) => {
    if (e.visible) { start(); } else { stop(); }
  });
  if (treeView.visible) { start(); }

  return {
    dispose(): void {
      subscription.dispose();
      stop();
    },
  };
}

/**
 * Mirror the store's fetch state to the `basilisk.processesState` context key
 * that gates the welcome's loading/error/empty copy (#147). Reactive: the
 * effect re-fires whenever the store's process state changes, including the
 * initial "loading" seed.
 */
export function bindProcessesContextKey(store: Store): vscode.Disposable {
  const dispose = effect(() => {
    void vscode.commands.executeCommand(
      "setContext",
      PROCESSES_STATE_CONTEXT_KEY,
      store.processes.value.fetch,
    );
  });
  return { dispose };
}
