// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Memory profiler UI module for the Basilisk VS Code extension.
 *
 * Provides:
 * - Memory profiler command handlers (start/snapshot/stop/references)
 * - Reference graph webview (force-directed graph of object retention)
 * - Memory dashboard webview (summary cards, top allocations, leak badges)
 * - Memory status bar indicator
 *
 * All memory analysis logic lives in the LSP. This module handles
 * only the client-side UI and command routing.
 */

import * as vscode from "vscode";
import { Logger } from "./logger";
import type { Store } from "./store";
import { currentStoppedFrameId, evaluateInDebugSession } from "./dap-evaluate";
import {
  disposeRefGraph,
  openRefGraphWebview,
  type ReferenceGraphResult,
} from "./memory-ref-graph";
import {
  applyLeakDecorations,
  applyMemoryDecorations,
  clearMemoryDecorations,
  disposeMemoryDecorations,
  type MemoryDiffResult,
  type MemorySnapshotResult,
} from "./memory-decorations";

// ── Constants ─────────────────────────────────────────────────────────────

const LSP_MEM_CMD = {
  start: "basilisk.memory.start",
  snapshot: "basilisk.memory.snapshot",
  diff: "basilisk.memory.diff",
  references: "basilisk.memory.references",
  objectsByType: "basilisk.memory.objectsByType",
  gcCollect: "basilisk.memory.gcCollect",
  ingest: "basilisk.memory.ingest",
} as const;

/** tracemalloc traceback depth injected at start. */
const TRACEBACK_DEPTH = 25;
/** Reference-graph traversal bounds. */
const REF_GRAPH_MAX_DEPTH = 5;
const REF_GRAPH_MAX_NODES = 200;

/** A tagged ingest result returned by `basilisk.memory.ingest`. */
interface MemoryIngestResult {
  kind: "snapshot" | "diff" | "gc" | "refs" | "objects" | "ack";
  [field: string]: unknown;
}

// ── State ─────────────────────────────────────────────────────────────────

let memoryStatusBarItem: vscode.StatusBarItem | undefined;
let activeMemorySessionId: string | undefined;
let memDashboardPanel: vscode.WebviewPanel | undefined;

// ── Registration ──────────────────────────────────────────────────────────

/** Register memory profiler commands and UI components. */
export function registerMemoryProfiler(
  context: vscode.ExtensionContext,
  store: Store,
): vscode.Disposable[] {
  /** Status bar priority — lower than main Basilisk item. */
  const MEMORY_STATUS_BAR_PRIORITY = 98;

  memoryStatusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    MEMORY_STATUS_BAR_PRIORITY,
  );
  memoryStatusBarItem.command = "basilisk.memoryStop";

  const disposables: vscode.Disposable[] = [
    memoryStatusBarItem,
    vscode.commands.registerCommand("basilisk.memoryStart", async () =>
      handleMemoryStart(store),
    ),
    vscode.commands.registerCommand("basilisk.memorySnapshot", async () =>
      handleMemorySnapshot(store),
    ),
    vscode.commands.registerCommand("basilisk.memoryDiff", async () =>
      handleMemoryDiff(store),
    ),
    vscode.commands.registerCommand("basilisk.memoryGcCollect", async () =>
      handleMemoryGcCollect(store),
    ),
    vscode.commands.registerCommand("basilisk.memoryStop", () => {
      handleMemoryStop(store);
    }),
    vscode.commands.registerCommand("basilisk.memoryReferences", async () =>
      handleMemoryReferences(store),
    ),
  ];

  return disposables;
}

/** Clean up memory profiler resources. */
export function disposeMemoryProfiler(): void {
  clearMemoryDecorations();
  disposeMemoryDecorations();
  disposeRefGraph();
  if (memDashboardPanel !== undefined) {
    memDashboardPanel.dispose();
    memDashboardPanel = undefined;
  }
}

// ── Round-trip courier ──────────────────────────────────────────────────────

/**
 * Run one memory operation as the editor-as-courier round-trip:
 *   1. ask the LSP for the injection script (`command` → `{ script }`),
 *   2. run it in the paused debuggee via DAP `evaluate`,
 *   3. post the raw output back to `basilisk.memory.ingest`,
 *   4. return the LSP's structured, marker-dispatched result.
 *
 * Returns null (with an actionable message) when there is no session, nothing
 * is paused, or evaluation fails — memory profiling requires the debuggee to be
 * stopped at a breakpoint because debugpy cannot evaluate a running program.
 */
async function runMemoryScript(
  store: Store,
  command: string,
  extraArgs: Record<string, unknown> = {},
): Promise<MemoryIngestResult | null> {
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    void vscode.window.showErrorMessage("Basilisk LSP not connected");
    return null;
  }
  if (activeMemorySessionId === undefined) {
    void vscode.window.showWarningMessage("Basilisk: Start memory tracking first.");
    return null;
  }
  const frameId = await currentStoppedFrameId();
  if (frameId === null) {
    void vscode.window.showWarningMessage(
      "Basilisk: Pause the debugger at a breakpoint to inspect memory.",
    );
    return null;
  }

  try {
    const phase1 = await client.sendRequest<{ script?: string } | null>("workspace/executeCommand", {
      command,
      arguments: [{ memorySessionId: activeMemorySessionId, ...extraArgs }],
    });
    const script = phase1?.script;
    if (script === undefined || script === "") { return null; }

    const output = await evaluateInDebugSession(script, frameId);
    if (output === null) {
      void vscode.window.showWarningMessage("Basilisk: Could not run the memory script in the debuggee.");
      return null;
    }

    return await client.sendRequest<MemoryIngestResult | null>("workspace/executeCommand", {
      command: LSP_MEM_CMD.ingest,
      arguments: [{ memorySessionId: activeMemorySessionId, output }],
    });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.warn(`[Memory] ${command} round-trip failed: ${msg}`);
    void vscode.window.showWarningMessage(`Basilisk: ${msg}`);
    return null;
  }
}

// ── Command handlers ──────────────────────────────────────────────────────

async function handleMemoryStart(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    void vscode.window.showErrorMessage("Basilisk LSP not connected");
    return;
  }
  // tracemalloc must be injected into a paused debuggee, so require a stopped
  // frame before we even mint a session.
  const frameId = await currentStoppedFrameId();
  if (frameId === null) {
    void vscode.window.showWarningMessage(
      "Basilisk: Pause the debugger at a breakpoint, then start memory tracking.",
    );
    return;
  }

  try {
    const result = await client.sendRequest<{ memorySessionId?: string; script?: string } | null>("workspace/executeCommand", {
      command: LSP_MEM_CMD.start,
      arguments: [{ tracebackDepth: TRACEBACK_DEPTH }],
    });
    if (result?.memorySessionId === undefined || result.script === undefined) { return; }

    const ack = await evaluateInDebugSession(result.script, frameId);
    if (ack === null) {
      void vscode.window.showWarningMessage("Basilisk: Could not start tracemalloc in the debuggee.");
      return;
    }

    activeMemorySessionId = result.memorySessionId;
    updateMemoryStatusBar("tracking");
    Logger.info(`Memory tracking started: session ${result.memorySessionId}`);
    void vscode.window.showInformationMessage("Basilisk: Memory tracking started. Take a snapshot to inspect allocations.");
  } catch (err) {
    void vscode.window.showErrorMessage(
      `Memory tracking failed: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
}

async function handleMemorySnapshot(store: Store): Promise<void> {
  const result = await runMemoryScript(store, LSP_MEM_CMD.snapshot);
  if (result?.kind === "snapshot") {
    applyMemoryDecorations(result as unknown as MemorySnapshotResult);
    Logger.info(`Memory snapshot: ${String(result.currentMemory)} bytes current`);
    void vscode.window.showInformationMessage(
      `Basilisk: Snapshot — ${String(result.currentMemory)} bytes tracked`,
    );
  }
}

async function handleMemoryDiff(store: Store): Promise<void> {
  const result = await runMemoryScript(store, LSP_MEM_CMD.diff);
  if (result?.kind === "diff") {
    applyLeakDecorations(result as unknown as MemoryDiffResult);
    const leaks = Array.isArray(result.suspectedLeaks) ? result.suspectedLeaks : [];
    Logger.info(`Memory diff: ${leaks.length} suspected leak(s)`);
    void vscode.window.showInformationMessage(
      `Basilisk: Compared snapshots — ${leaks.length} suspected leak(s)`,
    );
  }
}

async function handleMemoryGcCollect(store: Store): Promise<void> {
  const result = await runMemoryScript(store, LSP_MEM_CMD.gcCollect);
  if (result?.kind === "gc") {
    const collected = Number(result.collected ?? 0);
    const uncollectable = Number(result.uncollectable ?? 0);
    Logger.info(`gc.collect(): ${collected} collected, ${uncollectable} uncollectable`);
    void vscode.window.showInformationMessage(
      `Basilisk: gc.collect() freed ${collected} object(s); ${uncollectable} uncollectable`,
    );
  }
}

function handleMemoryStop(_store: Store): void {
  activeMemorySessionId = undefined;
  updateMemoryStatusBar("idle");
  clearMemoryDecorations();
  Logger.info("Memory tracking stopped");
}

async function handleMemoryReferences(store: Store): Promise<void> {
  if (activeMemorySessionId === undefined) {
    void vscode.window.showWarningMessage("Basilisk: Start memory tracking first.");
    return;
  }

  const typeName = await vscode.window.showInputBox({
    prompt: "Object type to inspect (e.g. DataFrame, dict, MyClass)",
    placeHolder: "DataFrame",
  });
  if (typeName === undefined || typeName.trim() === "") {
    return;
  }

  const result = await runMemoryScript(store, LSP_MEM_CMD.references, {
    targetType: typeName.trim(),
    maxDepth: REF_GRAPH_MAX_DEPTH,
    maxNodes: REF_GRAPH_MAX_NODES,
  });
  if (result?.kind === "refs") {
    openRefGraphWebview({
      targetType: typeName.trim(),
      maxDepth: REF_GRAPH_MAX_DEPTH,
      maxNodes: REF_GRAPH_MAX_NODES,
      script: "",
      graph: result.graph as ReferenceGraphResult["graph"],
    });
  }
}

// ── Status bar ────────────────────────────────────────────────────────────

function updateMemoryStatusBar(state: "idle" | "tracking"): void {
  if (memoryStatusBarItem === undefined) { return; }

  if (state === "tracking") {
    memoryStatusBarItem.text = "$(eye) Memory Tracking";
    memoryStatusBarItem.tooltip =
      "Basilisk: Memory tracking active (click to stop)";
    memoryStatusBarItem.backgroundColor = new vscode.ThemeColor(
      "statusBarItem.warningBackground",
    );
    memoryStatusBarItem.show();
  } else {
    memoryStatusBarItem.hide();
  }
}
