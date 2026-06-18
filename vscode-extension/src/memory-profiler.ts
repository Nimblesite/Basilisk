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
import * as fs from "fs";
import { effect } from "@preact/signals-core";
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

/** [PROFILE-MEMORY-FINAL] How long to wait for the at-exit snapshot file after
 *  the program exits (covers the terminate-event/final-flush race), and the poll
 *  cadence while waiting. */
const FINAL_SNAPSHOT_WAIT_MS = 3000;
const FINAL_SNAPSHOT_POLL_MS = 100;

// ── State ─────────────────────────────────────────────────────────────────
//
// Memory-tracking session state lives in the store as a reactive signal
// ([PROFILE-PROCESSES-REACTIVE]); `boundStore` is the handle this module reads
// it through (and the e2e seam reads it through `activeMemorySession`).

let memoryStatusBarItem: vscode.StatusBarItem | undefined;
let boundStore: Store | undefined;
/** Most recent snapshot, so a later "Compare" can show it alongside the diff. */
let lastDashboardSnapshot: MemoryDashboardSnapshot | undefined;
/**
 * Final-snapshot files awaiting cleanup, keyed by debug-session id. When the
 * user stops tracking *mid-run*, the debuggee's `atexit` hook is still armed and
 * will write its file when the program eventually exits; we delete it on that
 * session's termination so a manual stop never orphans a temp file
 * ([PROFILE-MEMORY-FINAL]).
 */
const pendingSnapshotCleanup = new Map<string, string>();
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

  boundStore = store;
  memoryStatusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    MEMORY_STATUS_BAR_PRIORITY,
  );
  // Click the status-bar item to open the memory action menu (no palette needed).
  memoryStatusBarItem.command = "basilisk.memoryMenu";

  // The memory indicator follows the store's tracking state reactively
  // ([PROFILE-PROCESSES-REACTIVE]); the debug-session listeners below cover the
  // "debugging but not yet tracking" state, which is not a store signal.
  const disposeMemoryEffect = effect(() => {
    void store.profiler.value.memory;
    refreshMemoryStatusBar(store);
  });

  const disposables: vscode.Disposable[] = [
    memoryStatusBarItem,
    { dispose: disposeMemoryEffect },
    ...memoryCommandDisposables(store),
  ];

  // [PROFILE-UI-GATE] The memory indicator is the one profiling surface no `when`
  // clause can reach, so it shares the single switch in code: shown under test,
  // hidden for shipped users (see refreshMemoryStatusBar).
  memoryUiEnabled = isProfilingUiEnabled(context);
  refreshMemoryStatusBar(store);
  return disposables;
}

/** The memory command registrations and debug-session listeners. */
function memoryCommandDisposables(store: Store): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand("basilisk.memoryMenu", async () => handleMemoryMenu(store)),
    vscode.commands.registerCommand("basilisk.memoryStart", async () =>
      withUserProgress(MEM_START_TITLE, async (report) => runMemoryStartCommand(store, report)),
    ),
    vscode.commands.registerCommand("basilisk.memorySnapshot", async () => handleMemorySnapshot(store)),
    vscode.commands.registerCommand("basilisk.memoryDiff", async () => handleMemoryDiff(store)),
    vscode.commands.registerCommand("basilisk.memoryGcCollect", async () => handleMemoryGcCollect(store)),
    vscode.commands.registerCommand("basilisk.memoryStop", () => { runMemoryStopCommand(store); }),
    vscode.commands.registerCommand("basilisk.memoryReferences", async () => handleMemoryReferences(store)),
    // Show/hide the memory status-bar entry as Basilisk debug sessions come and go.
    vscode.debug.onDidChangeActiveDebugSession(() => { refreshMemoryStatusBar(store); }),
    vscode.debug.onDidStartDebugSession((session) => {
      refreshMemoryStatusBar(store);
      // "Run & Track Memory (Current File)" (#82): the launch stopped on
      // entry; inject tracemalloc there, then resume the program.
      if (session.type === "basilisk-debug" && session.configuration.memoryTrackOnLaunch === true) {
        void startMemoryTrackingOnLaunch(store);
      }
    }),
    // Only the *tracked* session's termination finalises the run into a visible
    // result from the at-exit snapshot ([PROFILE-MEMORY-FINAL], #146) — an
    // unrelated debug session ending must never tear down live tracking. A
    // manually-stopped session deletes its now-orphaned at-exit file here.
    vscode.debug.onDidTerminateDebugSession((session) => {
      if (session.id === store.profiler.value.memoryDebugSessionId && store.profiler.value.memory !== "idle") {
        pendingSnapshotCleanup.delete(session.id);
        void finalizeMemorySessionOnEnd(store);
        return;
      }
      const orphan = pendingSnapshotCleanup.get(session.id);
      if (orphan !== undefined) {
        pendingSnapshotCleanup.delete(session.id);
        void readFinalSnapshot(orphan); // reads + unlinks; the payload is discarded
      }
      refreshMemoryStatusBar(store);
    }),
  ];
}

/** The `basilisk.memoryStart` body: start tracking, then narrate what comes next. */
async function runMemoryStartCommand(store: Store, report: (message: string) => void): Promise<void> {
  if (await handleMemoryStart(store, report)) {
    // Snapshots auto-pause a running program ([PROFILE-MEMORY-HOWTO]); and even
    // if the user just lets it run, a final snapshot is captured at exit
    // ([PROFILE-MEMORY-FINAL]) — so this is never a dead end.
    void vscode.window.showInformationMessage(
      "Basilisk: Memory tracking started. Take a snapshot to inspect allocations, or let the program finish for an automatic final snapshot.",
    );
  }
}

/** The `basilisk.memoryStop` body: stop tracking and never report nothing. */
function runMemoryStopCommand(store: Store): void {
  // Stopping must never silently produce nothing: surface whether a snapshot was
  // captured, or say plainly that none was ([PROFILE-MEMORY-FINAL], #146).
  const hadSnapshot = lastDashboardSnapshot !== undefined;
  // Stopping mid-run leaves the debuggee's `atexit` hook armed; schedule the
  // file it will write for cleanup when that session terminates, so a manual
  // stop never orphans a temp file.
  const debugSessionId = store.profiler.value.memoryDebugSessionId;
  const finalSnapshotFile = store.profiler.value.memoryFinalSnapshotFile;
  if (debugSessionId !== undefined && finalSnapshotFile !== undefined) {
    pendingSnapshotCleanup.set(debugSessionId, finalSnapshotFile);
  }
  handleMemoryStop(store);
  void vscode.window.showInformationMessage(
    hadSnapshot
      ? "Basilisk: Memory tracking stopped."
      : "Basilisk: Memory tracking stopped — no snapshot was taken. Take a snapshot while paused, or let the program finish, to inspect allocations.",
  );
}

/** Whether memory tracking is currently live, per the store. */
function isTracking(): boolean {
  return boundStore?.profiler.value.memory === "active";
}

/**
 * The active memory-tracking session id, or undefined when not tracking.
 * E2e seam for [PROFILE-MEMORY-HOWTO]/[PROFILE-PROCESSES-LAUNCH-FILE]: lets
 * tests observe that tracking really started (e.g. the track-on-launch flow).
 */
export function activeMemorySession(): string | undefined {
  return isTracking() ? boundStore?.profiler.value.memorySessionId : undefined;
}

/** Quick-pick menu of memory actions — the clickable alternative to the palette. */
async function handleMemoryMenu(store: Store): Promise<void> {
  const tracking = store.profiler.value.memory === "active";
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
  pendingSnapshotCleanup.clear();
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
  const memorySessionId = store.profiler.value.memorySessionId;
  if (store.profiler.value.memory !== "active" || memorySessionId === undefined) {
    void vscode.window.showWarningMessage("Basilisk: Start memory tracking first.");
    return null;
  }
  // The pause → evaluate → analyze round-trip takes a beat; show its stages
  // under one notification ([PROFILE-UX-PROGRESS]).
  return withUserProgress(
    MEM_OP_TITLE[command] ?? "Basilisk: Inspecting memory",
    async (report) => runMemoryScriptStages({ client, command, extraArgs, report, memorySessionId }),
  );
}

/** Everything one staged memory round-trip needs. */
interface MemoryScriptRun {
  readonly client: NonNullable<Store["client"]["value"]>;
  readonly command: string;
  readonly extraArgs: Record<string, unknown>;
  readonly report: (message: string) => void;
  readonly memorySessionId: string;
}

/** The staged body of [`runMemoryScript`] — acquire, evaluate, ingest. */
async function runMemoryScriptStages(
  { client, command, extraArgs, report, memorySessionId }: MemoryScriptRun,
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
      arguments: [{ memorySessionId, ...extraArgs }],
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
      arguments: [{ memorySessionId, output }],
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
    if (await handleMemoryStart(store, report)) {
      Logger.info("Memory tracking started on launch — resuming the debuggee");
      // State the outcome up front: this run has no breakpoint, so it runs to
      // completion and a final snapshot opens automatically at exit
      // ([PROFILE-MEMORY-FINAL], #146) — not a toast that points nowhere.
      void vscode.window.showInformationMessage(
        "Basilisk: Tracking memory — a final snapshot opens automatically when the program finishes.",
      );
      report("Resuming the program…");
      await vscode.commands.executeCommand("workbench.action.debug.continue");
    }
  });
}

// ── Command handlers ──────────────────────────────────────────────────────

/**
 * Inject tracemalloc into the paused debuggee and adopt the session. Returns
 * whether tracking actually started, so each caller can show its own
 * context-appropriate message (the menu vs. the run-and-track-on-launch flow)
 * instead of a one-size-fits-all toast. The leg-1 response carries the at-exit
 * `finalSnapshotFile` the session is finalised from on session end
 * ([PROFILE-MEMORY-FINAL]).
 */
async function handleMemoryStart(
  store: Store,
  report: (message: string) => void,
): Promise<boolean> {
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    void vscode.window.showErrorMessage("Basilisk LSP not connected");
    return false;
  }
  if (store.profiler.value.memory === "active") {
    void vscode.window.showWarningMessage("Basilisk: Memory tracking is already active.");
    return false;
  }
  // tracemalloc must be injected into a paused debuggee — transparently pause
  // a running program (and resume it after), like any IDE memory profiler.
  // From here on the panel reflects a tracking start in flight; every exit path
  // must settle it back to active or idle ([PROFILE-PROCESSES-REACTIVE]).
  store.memoryTrackingStarting();
  report("Pausing the program…");
  const acquired = await acquireStoppedFrame();
  if (acquired === null) {
    store.memoryTrackingStopped();
    void vscode.window.showWarningMessage(
      "Basilisk: Could not pause the program — pause at a breakpoint, then start memory tracking.",
    );
    return false;
  }

  try {
    const result = await client.sendRequest<
      { memorySessionId?: string; script?: string; finalSnapshotFile?: string } | null
    >("workspace/executeCommand", {
      command: LSP_MEM_CMD.start,
      arguments: [{ tracebackDepth: TRACEBACK_DEPTH }],
    });
    if (result?.memorySessionId === undefined || result.script === undefined) {
      store.memoryTrackingStopped();
      return false;
    }

    report("Injecting tracemalloc…");
    const ack = await evaluateInDebugSession(result.script, acquired.frameId);
    if (ack === null) {
      store.memoryTrackingStopped();
      void vscode.window.showWarningMessage("Basilisk: Could not start tracemalloc in the debuggee.");
      return false;
    }

    // Remember which debug session this tracks, so only its termination
    // finalises the run ([PROFILE-MEMORY-FINAL]); at this point (the entry pause
    // or a transparent pause) the Basilisk debuggee is the active session.
    store.memoryTrackingActive(
      result.memorySessionId,
      result.finalSnapshotFile,
      vscode.debug.activeDebugSession?.id,
    );
    Logger.info(`Memory tracking started: session ${result.memorySessionId}`);
    return true;
  } catch (err) {
    store.memoryTrackingStopped();
    void vscode.window.showErrorMessage(
      `Memory tracking failed: ${err instanceof Error ? err.message : String(err)}`,
    );
    return false;
  } finally {
    await acquired.release();
  }
}

async function handleMemorySnapshot(store: Store): Promise<void> {
  const result = await runMemoryScript(store, LSP_MEM_CMD.snapshot);
  if (result?.kind === "snapshot") {
    await presentMemorySnapshot(result);
  }
}

/**
 * Land the user on a viewable snapshot result: paint the purple allocation
 * track, open the V8 `.heapprofile` in VS Code's built-in profile viewer (flame
 * chart + table, beside the source so its decorations stay visible), and retain
 * it for a later "Compare". Falls back to the Basilisk dashboard when no
 * `.heapprofile` was produced. Shared by the interactive snapshot
 * ([`handleMemorySnapshot`]) and the at-exit finalisation
 * ([`finalizeMemorySessionOnEnd`], [PROFILE-MEMORY-FINAL]).
 */
async function presentMemorySnapshot(result: MemoryIngestResult): Promise<void> {
  applyMemoryDecorations(result as unknown as MemorySnapshotResult);
  lastDashboardSnapshot = toDashboardSnapshot(result);
  Logger.info(`Memory snapshot: ${lastDashboardSnapshot.currentMemory} bytes current`);
  const heapProfilePath = asString(result.heapProfilePath);
  if (heapProfilePath !== "") {
    await vscode.commands.executeCommand(
      "vscode.open",
      vscode.Uri.file(heapProfilePath),
      vscode.ViewColumn.Beside,
    );
  } else {
    openMemoryDashboard(lastDashboardSnapshot);
  }
}

/**
 * Finalise a memory-tracking session when its debug session ends — the fix for
 * the "Run & Track Memory" dead-end ([PROFILE-MEMORY-FINAL], #146). A run with
 * no breakpoint completes with no paused frame to snapshot from, so the start
 * script registered an `atexit` hook that wrote a final snapshot to a file as
 * the program exited. Tear down the (now-stale) tracking state first so the
 * panel settles immediately, then read that file, ingest it through the normal
 * `basilisk.memory.ingest` path, and present the snapshot exactly like a manual
 * one. If nothing was captured (a crash, `os._exit`, or no allocations), say so
 * explicitly — the session end never silently produces nothing.
 */
async function finalizeMemorySessionOnEnd(store: Store): Promise<void> {
  const memorySessionId = store.profiler.value.memorySessionId;
  const finalSnapshotFile = store.profiler.value.memoryFinalSnapshotFile;
  handleMemoryStop(store);

  if (memorySessionId === undefined || finalSnapshotFile === undefined) { return; }
  const output = await readFinalSnapshot(finalSnapshotFile);
  if (output === null) {
    void vscode.window.showInformationMessage(
      "Basilisk: The program finished before a final memory snapshot could be captured — set a breakpoint and take a snapshot to inspect allocations.",
    );
    return;
  }

  const client = store.client.value;
  if (client?.isRunning() !== true) { return; }
  try {
    const result = await client.sendRequest<MemoryIngestResult | null>("workspace/executeCommand", {
      command: LSP_MEM_CMD.ingest,
      arguments: [{ memorySessionId, output }],
    });
    if (result?.kind === "snapshot") {
      await presentMemorySnapshot(result);
      void vscode.window.showInformationMessage(
        "Basilisk: Captured a final memory snapshot at exit — opened the allocation view.",
      );
    }
  } catch (err: unknown) {
    Logger.warn(`[Memory] final snapshot ingest failed: ${err instanceof Error ? err.message : String(err)}`);
  }
}

/**
 * Read the debuggee's at-exit snapshot payload, deleting the file. The `atexit`
 * write completes (and closes the file) before the process exits, so by the time
 * the debug session terminates the file is whole; a short poll only covers the
 * brief window where the terminate event races the final flush. Returns null
 * when no usable payload was written ([PROFILE-MEMORY-FINAL]).
 */
async function readFinalSnapshot(path: string): Promise<string | null> {
  const deadline = Date.now() + FINAL_SNAPSHOT_WAIT_MS;
  for (;;) {
    try {
      const contents = await fs.promises.readFile(path, "utf8");
      await fs.promises.unlink(path).catch(() => undefined);
      return contents.includes("__BASILISK_MEM__") ? contents : null;
    } catch {
      if (Date.now() >= deadline) { return null; }
      await new Promise<void>((resolve) => setTimeout(resolve, FINAL_SNAPSHOT_POLL_MS));
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

function handleMemoryStop(store: Store): void {
  store.memoryTrackingStopped();
  lastDashboardSnapshot = undefined;
  clearMemoryDecorations();
  Logger.info("Memory tracking stopped");
}

async function handleMemoryReferences(store: Store): Promise<void> {
  if (store.profiler.value.memory !== "active") {
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
function refreshMemoryStatusBar(store: Store): void {
  if (memoryStatusBarItem === undefined) { return; }
  // [PROFILE-UI-GATE] Same switch as the declarative surfaces, applied in code.
  if (!memoryUiEnabled) { memoryStatusBarItem.hide(); return; }

  const debugging = vscode.debug.activeDebugSession?.type === "basilisk-debug";
  const tracking = store.profiler.value.memory === "active";
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
