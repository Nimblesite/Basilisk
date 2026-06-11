// Implements [PROFILE-PROCESSES-LAUNCH]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-LAUNCH
/**
 * Launch actions for the Python Processes panel.
 *
 * Two surfaces, one language: per-row inline actions ("Profile CPU" 🔥 /
 * "Track Memory" 🗄️ — the #62 fix, hardened for #79) and the metric-explicit
 * title-bar launches for the current file ("Run & Profile CPU" / "Run & Track
 * Memory" — the #82 fix). The tree rendering lives in process-explorer.ts;
 * this module owns what happens when the user clicks.
 */

import * as vscode from "vscode";
import { startProfilingForPid } from "./profiler";
import { type Store } from "./store";
import { type ProcessInfo } from "./process-explorer";

// ── Per-row actions (#79) ────────────────────────────────────────────────

/** The slice of the panel's TreeView the launch actions read (test seam). */
export interface ProcessRowSource {
  readonly selection: readonly vscode.TreeItem[];
}

/** A tree row that targets a concrete process. */
type ProcessRowItem = vscode.TreeItem & { readonly process: ProcessInfo };

/** Structural guard: does this tree item carry a profilable process? */
function isProcessRow(item: vscode.TreeItem | undefined): item is ProcessRowItem {
  const pid = (item as { process?: { pid?: unknown } } | undefined)?.process?.pid;
  return typeof pid === "number";
}

/**
 * Resolve the row an inline action targets. VS Code sometimes invokes inline
 * tree-view commands with no argument (#79 — e.g. a click racing the panel's
 * auto-refresh), so fall back to the panel's current selection before warning.
 */
function resolveProcessRow(
  item: vscode.TreeItem | undefined,
  view: ProcessRowSource,
): ProcessRowItem | undefined {
  if (isProcessRow(item)) { return item; }
  return view.selection.find((entry): entry is ProcessRowItem => isProcessRow(entry));
}

/** Guard: narrow to a real target, warning when an action has no row. */
function ensureSelection(item: ProcessRowItem | undefined): item is ProcessRowItem {
  if (item === undefined) {
    vscode.window.showWarningMessage(
      "Basilisk: Select a Python process row, then use Profile CPU or Track Memory.",
    );
    return false;
  }
  return true;
}

/** Start CPU profiling the given process row — no input box (the #62 fix). */
async function profileProcess(store: Store, item: ProcessRowItem | undefined): Promise<void> {
  if (!ensureSelection(item)) { return; }
  const preset = vscode.workspace.getConfiguration("basilisk").get<string>("profiler.preset", "default");
  await startProfilingForPid(store, item.process.pid, preset);
}

/** Where a row's Track Memory action routes. */
export type MemoryTrackRoute = "start-tracking" | "offer-launch";

/** The slice of a debug session the memory route needs (test seam). */
type SessionRef = Pick<vscode.DebugSession, "id" | "type">;

/** Injectable environment for the row actions (test seam, like ProcessRowSource). */
export interface ProcessRowDeps {
  readonly runCommand: (command: string) => Thenable<unknown>;
  readonly activeSession: () => SessionRef | undefined;
}

const DEFAULT_ROW_DEPS: ProcessRowDeps = {
  runCommand: async (command) => vscode.commands.executeCommand(command),
  activeSession: () => vscode.debug.activeDebugSession,
};

/**
 * Memory tracking rides the DAP-`evaluate` courier ([PROFILE-MEMORY-HOWTO]),
 * so it can only ever target the live Basilisk debuggee — tracemalloc cannot
 * be injected into an arbitrary external PID. Anything else routes to the
 * "launch it under the debugger" offer instead of silently starting the wrong
 * (CPU) profiler.
 */
export function memoryTrackRoute(
  store: Store,
  pid: number,
  session: SessionRef | undefined,
): MemoryTrackRoute {
  const debuggeePid =
    session?.type === "basilisk-debug" ? store.getDebuggeeProcessId(session.id) : undefined;
  return debuggeePid === pid ? "start-tracking" : "offer-launch";
}

/** Track memory for the given row: real tracking for the debuggee, an honest offer otherwise. */
async function memoryTrackProcess(
  store: Store,
  item: ProcessRowItem | undefined,
  deps: ProcessRowDeps,
): Promise<void> {
  if (!ensureSelection(item)) { return; }
  const pid = item.process.pid;
  if (memoryTrackRoute(store, pid, deps.activeSession()) === "start-tracking") {
    await deps.runCommand("basilisk.memoryStart");
    return;
  }
  const launch = "Run & Track Memory (Current File)";
  void vscode.window
    .showWarningMessage(
      `Basilisk: Memory tracking needs the target running under the Basilisk debugger — PID ${pid} is not the active debuggee. Launch the current file with memory tracking instead?`,
      launch,
    )
    .then((choice) => {
      if (choice === launch) { void deps.runCommand("basilisk.trackMemoryCurrentFile"); }
    });
}

/**
 * The per-row launch handlers exactly as VS Code invokes them for the inline
 * buttons and context menu. Exported so tests can drive the real command
 * surface, including the no-argument invocation of issue #79.
 */
export function createProcessRowActions(
  store: Store,
  view: ProcessRowSource,
  deps: ProcessRowDeps = DEFAULT_ROW_DEPS,
): {
  readonly profileProcess: (item?: vscode.TreeItem) => Promise<void>;
  readonly memoryTrackProcess: (item?: vscode.TreeItem) => Promise<void>;
} {
  return {
    profileProcess: async (item) => profileProcess(store, resolveProcessRow(item, view)),
    memoryTrackProcess: async (item) =>
      memoryTrackProcess(store, resolveProcessRow(item, view), deps),
  };
}

// ── Current-file launches (#82) ──────────────────────────────────────────
// Implements [PROFILE-PROCESSES-LAUNCH-FILE].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-LAUNCH-FILE

/** Which metric a metric-explicit "Run & …" launch tracks (#82). */
export type LaunchMetric = "cpu" | "memory";

/**
 * Build the debug configuration for a metric-explicit run-and-profile launch.
 * Exported so tests can assert each metric produces the right launch shape.
 *
 * CPU sets `profileOnLaunch` (honoured by profiler.ts). Memory stops on entry
 * because tracemalloc is injected via DAP `evaluate`, which needs a paused
 * debuggee ([PROFILE-MEMORY-HOWTO]); memory-profiler.ts starts tracking at the
 * entry pause and then resumes the program.
 */
export function buildProfileLaunchConfig(
  metric: LaunchMetric,
  program: string,
): vscode.DebugConfiguration {
  if (metric === "cpu") {
    return {
      type: "basilisk-debug",
      request: "launch",
      name: "Run & Profile CPU (Current File)",
      program,
      profileOnLaunch: true,
      // macOS uses the cooperative in-process sampler, which is injected at
      // the entry pause ([PROFILE-COOPERATIVE]); other platforms attach
      // py-spy to the running debuggee and need no pause.
      ...(process.platform === "darwin" ? { stopOnEntry: true } : {}),
    };
  }
  return {
    type: "basilisk-debug",
    request: "launch",
    name: "Run & Track Memory (Current File)",
    program,
    stopOnEntry: true,
    memoryTrackOnLaunch: true,
  };
}

/** Run the active `.py` file under a debug session, tracking the given metric. */
async function profileCurrentFile(metric: LaunchMetric): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (editor?.document.languageId !== "python") {
    vscode.window.showWarningMessage("Basilisk: Open a Python file to run and profile.");
    return;
  }
  const file = editor.document.uri;
  const folder = vscode.workspace.getWorkspaceFolder(file);
  const started = await vscode.debug.startDebugging(folder, buildProfileLaunchConfig(metric, file.fsPath));
  if (!started) {
    vscode.window.showErrorMessage("Basilisk: Could not launch the current file for profiling.");
  }
}

// ── Registration ─────────────────────────────────────────────────────────

/** Register every launch command the Python Processes panel contributes. */
export function registerLaunchCommands(store: Store, view: ProcessRowSource): vscode.Disposable[] {
  const rowActions = createProcessRowActions(store, view);
  return [
    vscode.commands.registerCommand("basilisk.profileProcess", rowActions.profileProcess),
    vscode.commands.registerCommand("basilisk.memoryTrackProcess", rowActions.memoryTrackProcess),
    vscode.commands.registerCommand("basilisk.profileCurrentFileCpu", async () => profileCurrentFile("cpu")),
    vscode.commands.registerCommand("basilisk.trackMemoryCurrentFile", async () => profileCurrentFile("memory")),
  ];
}
