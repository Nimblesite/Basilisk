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
import { acquireStoppedFrame, evaluateInDebugSession, waitForStoppedFrame } from "./dap-evaluate";
import { withUserProgress } from "./progress-ops";
import {
  toDashboardDiff,
  toDashboardSnapshot,
  asString,
  type MemoryIngestResult,
} from "./memory-dashboard-mapping";
import {
  disposeMemoryDashboard,
  openMemoryDashboard,
  type MemoryDashboardSnapshot,
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

/** [PROFILE-UX-PROGRESS] Progress-notification titles, one per memory operation. */
const MEM_OP_TITLE: Readonly<Record<string, string>> = {
  [LSP_MEM_CMD.snapshot]: "Basilisk: Taking memory snapshot",
  [LSP_MEM_CMD.diff]: "Basilisk: Comparing memory snapshots",
  [LSP_MEM_CMD.gcCollect]: "Basilisk: Forcing garbage collection",
  [LSP_MEM_CMD.references]: "Basilisk: Building the reference graph",
};

/** The progress title for starting memory tracking (shared by both entry points). */
const MEM_START_TITLE = "Basilisk: Starting memory tracking";
/** Reference-graph traversal bounds. */
const REF_GRAPH_MAX_DEPTH = 5;
const REF_GRAPH_MAX_NODES = 200;

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
      withUserProgress(MEM_START_TITLE, async (report) => handleMemoryStart(store, report)),
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

/**
 * The active memory-tracking session id, or undefined when not tracking.
 * E2e seam for [PROFILE-MEMORY-HOWTO]/[PROFILE-PROCESSES-LAUNCH-FILE]: lets
 * tests observe that tracking really started (e.g. the track-on-launch flow).
 */
export function activeMemorySession(): string | undefined {
  return activeMemorySessionId;
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
    placeHolder: tracking ? "Basilisk memory profiling" : "Start memory tracking (briefly pauses the program)",
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
 * Returns null (with an actionable message) when there is no session, the
 * debuggee cannot be paused, or evaluation fails. debugpy can only `evaluate`
 * against a stopped frame, so a running program is transparently paused for
 * the script and resumed afterwards (a user's own breakpoint pause is left
 * untouched) — IDE-grade snapshots never demand a manual pause.
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
  // The pause → evaluate → analyze round-trip takes a beat; show its stages
  // under one notification ([PROFILE-UX-PROGRESS]).
  return withUserProgress(
    MEM_OP_TITLE[command] ?? "Basilisk: Inspecting memory",
    async (report) => runMemoryScriptStages({ client, command, extraArgs, report }),
  );
}

/** Everything one staged memory round-trip needs. */
interface MemoryScriptRun {
  readonly client: NonNullable<Store["client"]["value"]>;
  readonly command: string;
  readonly extraArgs: Record<string, unknown>;
  readonly report: (message: string) => void;
}

/** The staged body of [`runMemoryScript`] — acquire, evaluate, ingest. */
async function runMemoryScriptStages(
  { client, command, extraArgs, report }: MemoryScriptRun,
): Promise<MemoryIngestResult | null> {
  report("Pausing the program…");
  const acquired = await acquireStoppedFrame();
  if (acquired === null) {
    void vscode.window.showWarningMessage(
      "Basilisk: Could not pause the program for memory inspection — pause at a breakpoint and retry.",
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

    report("Inspecting the debuggee…");
    const output = await evaluateInDebugSession(script, acquired.frameId);
    if (output === null) {
      void vscode.window.showWarningMessage("Basilisk: Could not run the memory script in the debuggee.");
      return null;
    }

    report("Analyzing…");
    return await client.sendRequest<MemoryIngestResult | null>("workspace/executeCommand", {
      command: LSP_MEM_CMD.ingest,
      arguments: [{ memorySessionId: activeMemorySessionId, output }],
    });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.warn(`[Memory] ${command} round-trip failed: ${msg}`);
    void vscode.window.showWarningMessage(`Basilisk: ${msg}`);
    return null;
  } finally {
    await acquired.release();
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
  // One notification spans the whole auto-start ([PROFILE-UX-PROGRESS]).
  await withUserProgress(MEM_START_TITLE, async (report) => {
    report("Waiting for the program to pause at entry…");
    const frameId = await waitForStoppedFrame();
    if (frameId === null) {
      void vscode.window.showWarningMessage(
        "Basilisk: The debuggee did not pause at entry — pause at a breakpoint, then start memory tracking.",
      );
      return;
    }
    await handleMemoryStart(store, report);
    if (activeMemorySessionId !== undefined) {
      Logger.info("Memory tracking started on launch — resuming the debuggee");
      report("Resuming the program…");
      await vscode.commands.executeCommand("workbench.action.debug.continue");
    }
  });
}

// ── Command handlers ──────────────────────────────────────────────────────

async function handleMemoryStart(
  store: Store,
  report: (message: string) => void,
): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    void vscode.window.showErrorMessage("Basilisk LSP not connected");
    return;
  }
  // tracemalloc must be injected into a paused debuggee — transparently pause
  // a running program (and resume it after), like any IDE memory profiler.
  report("Pausing the program…");
  const acquired = await acquireStoppedFrame();
  if (acquired === null) {
    void vscode.window.showWarningMessage(
      "Basilisk: Could not pause the program — pause at a breakpoint, then start memory tracking.",
    );
    return;
  }

  try {
    const result = await client.sendRequest<{ memorySessionId?: string; script?: string } | null>("workspace/executeCommand", {
      command: LSP_MEM_CMD.start,
      arguments: [{ tracebackDepth: TRACEBACK_DEPTH }],
    });
    if (result?.memorySessionId === undefined || result.script === undefined) { return; }

    report("Injecting tracemalloc…");
    const ack = await evaluateInDebugSession(result.script, acquired.frameId);
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
  } finally {
    await acquired.release();
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
    // + table, Self/Total size) — the same UI as Node.js heap profiles. Beside
    // the source, so the snapshotted file keeps its allocation decorations.
    const heapProfilePath = asString(result.heapProfilePath);
    if (heapProfilePath !== "") {
      await vscode.commands.executeCommand(
        "vscode.open",
        vscode.Uri.file(heapProfilePath),
        vscode.ViewColumn.Beside,
      );
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
    memoryStatusBarItem.tooltip = "Basilisk: click to start memory tracking (briefly pauses the program)";
    memoryStatusBarItem.backgroundColor = undefined;
  }
  memoryStatusBarItem.show();
}
