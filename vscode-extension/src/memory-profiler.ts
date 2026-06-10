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
import { isProfilingUiEnabled } from "./profiling-ui";
import { currentStoppedFrameId, evaluateInDebugSession } from "./dap-evaluate";
import { POLL_INTERVAL_MS, STARTUP_TIMEOUT_MS } from "./timeouts";
import {
  disposeMemoryDashboard,
  openMemoryDashboard,
  type MemoryDashboardSnapshot,
  type MemoryDiffData,
} from "./memory-dashboard";
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
/** Most recent snapshot, so a later "Compare" can show it alongside the diff. */
let lastDashboardSnapshot: MemoryDashboardSnapshot | undefined;
/** [PROFILE-UI-GATE] Whether the (imperative) memory indicator may be shown. */
let memoryUiEnabled = false;

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
  // Click the status-bar item to open the memory action menu (no palette needed).
  memoryStatusBarItem.command = "basilisk.memoryMenu";

  const disposables: vscode.Disposable[] = [
    memoryStatusBarItem,
    vscode.commands.registerCommand("basilisk.memoryMenu", async () =>
      handleMemoryMenu(),
    ),
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
    // Show/hide the memory status-bar entry as Basilisk debug sessions come and go.
    vscode.debug.onDidChangeActiveDebugSession(() => { refreshMemoryStatusBar(); }),
    vscode.debug.onDidStartDebugSession((session) => {
      refreshMemoryStatusBar();
      // "Run & Track Memory (Current File)" (#82): the launch stopped on
      // entry; inject tracemalloc there, then resume the program.
      if (session.type === "basilisk-debug" && session.configuration.memoryTrackOnLaunch === true) {
        void startMemoryTrackingOnLaunch(store);
      }
    }),
    vscode.debug.onDidTerminateDebugSession(() => { refreshMemoryStatusBar(); }),
  ];

  // [PROFILE-UI-GATE] The memory indicator is the one profiling surface no `when`
  // clause can reach, so it shares the single switch in code: shown under test,
  // hidden for shipped users (see refreshMemoryStatusBar).
  memoryUiEnabled = isProfilingUiEnabled(context);
  refreshMemoryStatusBar();
  return disposables;
}

/** Quick-pick menu of memory actions — the clickable alternative to the palette. */
async function handleMemoryMenu(): Promise<void> {
  const tracking = activeMemorySessionId !== undefined;
  const items: { label: string; command: string }[] = tracking
    ? [
        { label: "$(device-camera) Take Memory Snapshot", command: "basilisk.memorySnapshot" },
        { label: "$(diff) Compare Memory Snapshots", command: "basilisk.memoryDiff" },
        { label: "$(type-hierarchy) Show Reference Graph", command: "basilisk.memoryReferences" },
        { label: "$(trash) Force Garbage Collection", command: "basilisk.memoryGcCollect" },
        { label: "$(debug-stop) Stop Memory Tracking", command: "basilisk.memoryStop" },
      ]
    : [{ label: "$(database) Start Memory Tracking", command: "basilisk.memoryStart" }];
  const pick = await vscode.window.showQuickPick(items, {
    placeHolder: tracking ? "Basilisk memory profiling" : "Pause the debugger, then start memory tracking",
  });
  if (pick !== undefined) {
    await vscode.commands.executeCommand(pick.command);
  }
}

/** Clean up memory profiler resources. */
export function disposeMemoryProfiler(): void {
  clearMemoryDecorations();
  disposeMemoryDecorations();
  disposeRefGraph();
  disposeMemoryDashboard();
  lastDashboardSnapshot = undefined;
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

// ── Track-memory-on-launch ────────────────────────────────────────────────

/**
 * Auto-start flow for the "Run & Track Memory (Current File)" entry point
 * (#82). Memory tracking needs a paused debuggee ([PROFILE-MEMORY-HOWTO]), so
 * the launch config sets `stopOnEntry`; tracemalloc is injected at the entry
 * pause and the debuggee is resumed so the program runs with tracking on.
 * Implements [PROFILE-PROCESSES-LAUNCH-FILE] (memory leg).
 */
async function startMemoryTrackingOnLaunch(store: Store): Promise<void> {
  const frameId = await waitForStoppedFrame();
  if (frameId === null) {
    void vscode.window.showWarningMessage(
      "Basilisk: The debuggee did not pause at entry — pause at a breakpoint, then start memory tracking.",
    );
    return;
  }
  await handleMemoryStart(store);
  if (activeMemorySessionId !== undefined) {
    Logger.info("Memory tracking started on launch — resuming the debuggee");
    await vscode.commands.executeCommand("workbench.action.debug.continue");
  }
}

/** Poll for a stopped frame until the startup budget runs out. */
async function waitForStoppedFrame(): Promise<number | null> {
  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const frameId = await currentStoppedFrameId();
    if (frameId !== null) { return frameId; }
    await new Promise<void>((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
  }
  return null;
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
    refreshMemoryStatusBar();
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
    // Retain for a later "Compare" (the Basilisk leak-analysis dashboard).
    lastDashboardSnapshot = toDashboardSnapshot(result);
    Logger.info(`Memory snapshot: ${lastDashboardSnapshot.currentMemory} bytes current`);
    // Open the V8 .heapprofile in VS Code's built-in profile viewer (flame chart
    // + table, Self/Total size) — the same UI as Node.js heap profiles.
    const heapProfilePath = asString(result.heapProfilePath);
    if (heapProfilePath !== "") {
      await vscode.commands.executeCommand("vscode.open", vscode.Uri.file(heapProfilePath));
    } else {
      // Fall back to the Basilisk dashboard if the file wasn't produced.
      openMemoryDashboard(lastDashboardSnapshot);
    }
  }
}

async function handleMemoryDiff(store: Store): Promise<void> {
  const result = await runMemoryScript(store, LSP_MEM_CMD.diff);
  if (result?.kind === "diff") {
    applyLeakDecorations(result as unknown as MemoryDiffResult);
    const leaks = Array.isArray(result.suspectedLeaks) ? result.suspectedLeaks : [];
    Logger.info(`Memory diff: ${leaks.length} suspected leak(s)`);
    // Refresh the dashboard with the leak analysis (needs a prior snapshot).
    if (lastDashboardSnapshot !== undefined) {
      openMemoryDashboard(lastDashboardSnapshot, toDashboardDiff(result));
    }
    void vscode.window.showInformationMessage(
      `Basilisk: Compared snapshots — ${leaks.length} suspected leak(s)`,
    );
  }
}

/** Coerce an `unknown` JSON field to a string (never an object stringification). */
function asString(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

/** Coerce an `unknown` JSON field to a finite number. */
function asNumber(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

/** Map an ingest snapshot result to the dashboard's snapshot shape. */
function toDashboardSnapshot(result: MemoryIngestResult): MemoryDashboardSnapshot {
  return {
    memorySessionId: asString(result.memorySessionId),
    snapshotId: asString(result.snapshotId),
    currentMemory: asNumber(result.currentMemory),
    peakMemory: asNumber(result.peakMemory),
    gcObjects: asNumber(result.gcObjects),
    gcCounts: Array.isArray(result.gcCounts) ? (result.gcCounts as number[]) : [],
    topAllocations: (Array.isArray(result.topAllocations)
      ? result.topAllocations
      : []) as MemoryDashboardSnapshot["topAllocations"],
    timeline: [],
  };
}

/** Map an ingest diff result to the dashboard's diff shape (lowercasing confidence). */
function toDashboardDiff(result: MemoryIngestResult): MemoryDiffData {
  const leaks = Array.isArray(result.suspectedLeaks) ? result.suspectedLeaks : [];
  return {
    totalGrowth: asNumber(result.totalGrowth),
    totalFreed: asNumber(result.totalFreed),
    netGrowth: asNumber(result.netGrowth),
    grownAllocations: [],
    suspectedLeaks: leaks.map((raw) => {
      const leak = raw as Record<string, unknown>;
      return {
        file: asString(leak.file),
        line: asNumber(leak.line),
        sizeGrowth: asNumber(leak.sizeGrowth),
        countGrowth: asNumber(leak.countGrowth),
        currentSize: asNumber(leak.currentSize),
        currentCount: asNumber(leak.currentCount),
        confidence: asString(leak.confidence, "low").toLowerCase() as MemoryDiffData["suspectedLeaks"][number]["confidence"],
        reason: asString(leak.reason),
      };
    }),
  };
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
  lastDashboardSnapshot = undefined;
  refreshMemoryStatusBar();
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

/**
 * Show the memory status-bar entry whenever a Basilisk debug session is active
 * (or tracking is on) and click it to open the action menu. Hidden otherwise.
 */
function refreshMemoryStatusBar(): void {
  if (memoryStatusBarItem === undefined) { return; }
  // [PROFILE-UI-GATE] Same switch as the declarative surfaces, applied in code.
  if (!memoryUiEnabled) { memoryStatusBarItem.hide(); return; }

  const debugging = vscode.debug.activeDebugSession?.type === "basilisk-debug";
  const tracking = activeMemorySessionId !== undefined;
  if (!debugging && !tracking) {
    memoryStatusBarItem.hide();
    return;
  }

  if (tracking) {
    memoryStatusBarItem.text = "$(eye) Memory: tracking";
    memoryStatusBarItem.tooltip = "Basilisk: memory tracking active — click for snapshot/compare/stop";
    memoryStatusBarItem.backgroundColor = new vscode.ThemeColor("statusBarItem.warningBackground");
  } else {
    memoryStatusBarItem.text = "$(database) Memory";
    memoryStatusBarItem.tooltip = "Basilisk: click to start memory tracking (pause at a breakpoint first)";
    memoryStatusBarItem.backgroundColor = undefined;
  }
  memoryStatusBarItem.show();
}
