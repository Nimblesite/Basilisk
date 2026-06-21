// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Memory profiler command + lifecycle layer for the Basilisk VS Code extension.
 *
 * Owns the `basilisk.memory*` command registrations, the memory status-bar
 * indicator, the track-on-launch auto-start, and the at-exit finalisation
 * ([PROFILE-MEMORY-FINAL]). The editor-as-courier round-trip and result
 * presentation live in [`memory-capture.ts`](./memory-capture.ts); the
 * reference-graph type picker in [`memory-ref-picker.ts`](./memory-ref-picker.ts);
 * the automatic per-pause / interval capture in
 * [`memory-autopilot.ts`](./memory-autopilot.ts). All memory analysis logic lives
 * in the LSP — this module is client-side UI and command routing.
 */

import * as vscode from "vscode";
import { effect } from "@preact/signals-core";
import { Logger } from "./logger";
import type { Store } from "./store";
import { isProfilingUiEnabled } from "./profiling-ui";
import { acquireStoppedFrame, evaluateInDebugSession, waitForStoppedFrame } from "./dap-evaluate";
import { withUserProgress } from "./progress-ops";
import type { MemoryIngestResult } from "./memory-dashboard-mapping";
import { disposeMemoryDashboard } from "./memory-dashboard";
import { disposeRefGraph } from "./memory-ref-graph";
import {
  clearMemoryDecorations,
  disposeMemoryDecorations,
} from "./memory-decorations";
import {
  LSP_MEM_CMD,
  MEM_START_TITLE,
  TRACEBACK_DEPTH,
  hasCapturedSnapshot,
  presentDiff,
  presentSnapshot,
  readFinalSnapshot,
  resetCaptureState,
  runMemoryScript,
} from "./memory-capture";
import { pickReferenceType, walkReferences } from "./memory-ref-picker";

// ── State ─────────────────────────────────────────────────────────────────
//
// Memory-tracking session state lives in the store as a reactive signal
// ([PROFILE-PROCESSES-REACTIVE]); `boundStore` is the handle this module reads
// it through (and the e2e seam reads it through `activeMemorySession`).

let memoryStatusBarItem: vscode.StatusBarItem | undefined;
let boundStore: Store | undefined;
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
      // Finalise when the tracked session ends. If we never captured a concrete
      // session id — `activeDebugSession` was momentarily undefined at start
      // (memui-3) — fall back to ANY basilisk-debug session ending while tracking
      // is active: only one memory session exists at a time, so it is the tracked
      // one. This guarantees tracking always settles instead of hanging on
      // "Memory: tracking" forever (no stale state on screen).
      const trackedId = store.profiler.value.memoryDebugSessionId;
      const tracking = store.profiler.value.memory !== "idle";
      const isTrackedSession =
        trackedId !== undefined ? session.id === trackedId : session.type === "basilisk-debug";
      if (tracking && isTrackedSession) {
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
    // With the autopilot on ([PROFILE-MEMORY-AUTOPILOT]), the user just sets
    // breakpoints and presses Continue — each pause is captured automatically;
    // and even a breakpoint-free run captures a final snapshot at exit
    // ([PROFILE-MEMORY-FINAL]) — so this is never a dead end.
    void vscode.window.showInformationMessage(
      "Basilisk: Memory tracking started. Press Continue to auto-capture each pause, or let the program finish for an automatic final snapshot.",
    );
  }
}

/** The `basilisk.memoryStop` body: stop tracking and never report nothing. */
function runMemoryStopCommand(store: Store): void {
  // Stopping must never silently produce nothing: surface whether a snapshot was
  // captured, or say plainly that none was ([PROFILE-MEMORY-FINAL], #146).
  const hadSnapshot = hasCapturedSnapshot();
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
  resetCaptureState();
  pendingSnapshotCleanup.clear();
}

// ── Track-memory-on-launch ────────────────────────────────────────────────

/**
 * Auto-start flow for the "Run & Track Memory (Current File)" entry point
 * (#82). Memory tracking needs a paused debuggee ([PROFILE-MEMORY-HOWTO]), so
 * the launch config sets `stopOnEntry`; tracemalloc is injected at the entry
 * pause and the debuggee is resumed so the program runs with tracking on. With a
 * breakpoint set, the autopilot then captures each pass automatically
 * ([PROFILE-MEMORY-AUTOPILOT-PAUSE]); with none, a final snapshot opens at exit
 * ([PROFILE-MEMORY-FINAL]). Implements [PROFILE-PROCESSES-LAUNCH-FILE] (memory leg).
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
      // State the outcome up front so the toast never points nowhere: a run with
      // a breakpoint auto-captures each pass ([PROFILE-MEMORY-AUTOPILOT-PAUSE]);
      // one without finalises at exit ([PROFILE-MEMORY-FINAL], #146).
      const hasBreakpoints = vscode.debug.breakpoints.length > 0;
      void vscode.window.showInformationMessage(
        hasBreakpoints
          ? "Basilisk: Tracking memory — press Continue and each pause is captured automatically."
          : "Basilisk: Tracking memory — a final snapshot opens automatically when the program finishes.",
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
    presentSnapshot(result, { openNativeViewer: true });
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
    // No file / no marker: either the program made no tracked allocations, or it
    // exited too abruptly for even the SIGTERM/SIGINT hook to run (a native
    // crash, SIGKILL, or os._exit). Don't claim it "finished" — that is wrong
    // for a kill (ux-1).
    void vscode.window.showInformationMessage(
      "Basilisk: No final memory snapshot was captured — the program either made no tracked allocations or exited abruptly (a crash, kill, or os._exit). Set a breakpoint and take a snapshot to inspect allocations.",
    );
    return;
  }

  // The snapshot WAS captured; from here every failure to turn it into a view is
  // surfaced, never swallowed — the spec's "stopping never silently produces
  // nothing" guarantee covers the post-capture paths too (memui-2).
  const captured = "Basilisk: Captured a final memory snapshot at exit, but ";
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    void vscode.window.showWarningMessage(`${captured}the language server is not running, so it could not be analyzed.`);
    return;
  }
  try {
    const result = await client.sendRequest<MemoryIngestResult | null>("workspace/executeCommand", {
      command: LSP_MEM_CMD.ingest,
      arguments: [{ memorySessionId, output }],
    });
    if (result?.kind === "snapshot") {
      presentSnapshot(result, { openNativeViewer: true });
      void vscode.window.showInformationMessage(
        "Basilisk: Captured a final memory snapshot at exit — opened the allocation view.",
      );
      return;
    }
    void vscode.window.showWarningMessage(`${captured}it could not be analyzed into an allocation view.`);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.warn(`[Memory] final snapshot ingest failed: ${msg}`);
    void vscode.window.showWarningMessage(`${captured}analyzing it failed — ${msg}.`);
  }
}

async function handleMemoryDiff(store: Store): Promise<void> {
  const result = await runMemoryScript(store, LSP_MEM_CMD.diff);
  if (result?.kind === "diff") {
    const diff = presentDiff(result);
    void vscode.window.showInformationMessage(
      `Basilisk: Compared snapshots — ${diff.suspectedLeaks.length} suspected leak(s)`,
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
  resetCaptureState();
  clearMemoryDecorations();
  Logger.info("Memory tracking stopped");
}

/**
 * Show Reference Graph — pick a type from the data-driven Quick Pick (the active
 * file's classes + container builtins) and walk its retainers
 * ([PROFILE-MEMORY-REFGRAPH-PICKER]); no blank "type a name" box.
 */
async function handleMemoryReferences(store: Store): Promise<void> {
  if (store.profiler.value.memory !== "active") {
    void vscode.window.showWarningMessage("Basilisk: Start memory tracking first.");
    return;
  }
  const typeName = await pickReferenceType();
  if (typeName === undefined || typeName.trim() === "") { return; }
  await walkReferences(store, typeName.trim());
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
