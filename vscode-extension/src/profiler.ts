// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Profiler UI module for the Basilisk VS Code extension.
 *
 * Provides:
 * - Profiler status bar item (pulsing orange dot while profiling)
 * - Command handlers for start/stop/snapshot/attach-to-debug
 * - Flamegraph webview panel
 * - Progress notification during active profiling
 *
 * All profiling logic lives in the LSP server. This module handles
 * only the client-side UI and command routing.
 */

import * as vscode from "vscode";
import { Logger } from "./logger";
import type { Store } from "./store";
import { evaluateInDebugSession, waitForStoppedFrame } from "./dap-evaluate";
import { withUserProgress } from "./progress-ops";
import { POLL_INTERVAL_MS, STARTUP_TIMEOUT_MS } from "./timeouts";
import {
  applyProfileDecorations,
  disposeProfileDecorations,
  type ProfileResult,
} from "./profiler-decorations";
import { disposeFlamegraphPanel, openFlamegraphWebview } from "./profiler-flamegraph-html";
import { shouldProfileOnLaunch, waitForDebuggeePid } from "./profiler-launch";
import {
  createProfilerStatusBar,
  setProfilerStatus,
  updateProfilerProgress,
} from "./profiler-status";

// Re-exported so tests keep one import site for the profiler's public seams.
export { profilerStatusText } from "./profiler-status";
export { shouldProfileOnLaunch } from "./profiler-launch";

// ── Constants ─────────────────────────────────────────────────────────────

/** LSP command names (must match basilisk-common constants). */
const LSP_CMD = {
  start: "basilisk.profiler.start",
  stop: "basilisk.profiler.stop",
  snapshot: "basilisk.profiler.snapshot",
  list: "basilisk.profiler.list",
  cooperativeScript: "basilisk.profiler.cooperativeScript",
  cooperativeAttach: "basilisk.profiler.cooperativeAttach",
} as const;

/** Ack printed by the injected cooperative sampler ([PROFILE-COOPERATIVE]). */
const COOPERATIVE_ACK = "__BASILISK_CPU_ACK__";

/** LSP notification for profiling progress. */
const PROFILER_PROGRESS_NOTIFICATION = "basilisk/profiler/progress";

// ── Constants ─────────────────────────────────────────────────────────────

/** Default sample rate fallback (Hz). */
const DEFAULT_SAMPLE_RATE = 100;

// ── State ─────────────────────────────────────────────────────────────────

let activeSessionId: string | undefined;
let activePid: number | undefined;
let lastResult: ProfileResult | undefined;

// ── Registration ──────────────────────────────────────────────────────────

/**
 * Register profiler UI components. Called once during extension activation.
 * Returns disposables for cleanup.
 */
export function registerProfiler(
  context: vscode.ExtensionContext,
  store: Store,
): vscode.Disposable[] {
  const disposables: vscode.Disposable[] = [];

  // Status bar item ([PROFILE-UX-PROGRESS] lifecycle lives in profiler-status.ts).
  disposables.push(createProfilerStatusBar());

  // Client-side commands that proxy to LSP.
  disposables.push(
    vscode.commands.registerCommand("basilisk.profileStart", async () => handleProfileStart()),
    vscode.commands.registerCommand("basilisk.profileStop", async () => handleProfileStop(store)),
    vscode.commands.registerCommand("basilisk.profileSnapshot", async () => handleProfileSnapshot(store)),
    vscode.commands.registerCommand("basilisk.profileAttachToDebug", async () => handleProfileAttachToDebug(store)),
  );

  // Listen for profiler progress notifications from LSP.
  registerProgressListener(store);

  // Clear decorations when active editor changes (optional, re-applies on focus).
  disposables.push(
    vscode.window.onDidChangeVisibleTextEditors(() => {
      if (lastResult !== undefined) {
        applyProfileDecorations(lastResult);
      }
    }),
  );

  // "Profile on Launch" — automatically start profiling when a debug session starts.
  disposables.push(
    vscode.debug.onDidStartDebugSession((session) => {
      if (shouldProfileOnLaunch(session) && activeSessionId === undefined) {
        Logger.info(`Profile on Launch: auto-profiling debug session ${session.id}`);
        void startProfilerOnLaunch(store, session.id);
      }
    }),
  );

  // Auto-stop profiling when debug session ends.
  disposables.push(
    vscode.debug.onDidTerminateDebugSession(() => {
      if (activeSessionId !== undefined) {
        Logger.info("Debug session ended — auto-stopping profiler");
        void handleProfileStop(store);
      }
    }),
  );

  return disposables;
}

// ── Session adoption ──────────────────────────────────────────────────────

/** The shape every profiling start command resolves to. */
interface StartedSession {
  sessionId: string;
  pid: number;
  pythonVersion: string;
}

/**
 * Adopt a freshly started session into the shared UI state (status bar,
 * stop/snapshot routing, live progress) and announce it.
 */
function adoptSession(result: StartedSession, announcement: string): void {
  activeSessionId = result.sessionId;
  activePid = result.pid;
  void vscode.commands.executeCommand("setContext", "basilisk.profiling", true);
  setProfilerStatus("profiling", result.pid);
  Logger.info(
    `Profiling started: PID ${result.pid}, Python ${result.pythonVersion}, session ${result.sessionId}`,
  );
  vscode.window.showInformationMessage(announcement);
}

// ── Launch flows ([PROFILE-COOPERATIVE], [PROFILE-UX-PROGRESS]) ───────────

/** The single progress title every CPU-start flow shares. */
const CPU_START_TITLE = "Basilisk: Starting CPU profiler";

/**
 * Auto-start dispatcher for a freshly launched debug session: cooperative
 * sampler on macOS, py-spy attach elsewhere. The whole start runs under one
 * progress notification with stage messages, and the status bar shows a
 * spinner until the live sample counter takes over — a click is never
 * followed by silence ([PROFILE-UX-PROGRESS]).
 */
async function startProfilerOnLaunch(store: Store, debugSessionId: string): Promise<void> {
  setProfilerStatus("starting");
  try {
    await withUserProgress(CPU_START_TITLE, async (report) => {
      if (process.platform === "darwin") {
        // macOS gates task ports behind entitled debuggers — even root
        // py-spy gets EPERM — so use the cooperative in-process sampler
        // injected at the entry pause ([PROFILE-COOPERATIVE], OOTB).
        await startCooperativeProfileOnLaunch(store, report);
        return;
      }
      // The debuggee PID arrives asynchronously via the DAP `process` event,
      // so wait for it before attaching (avoids a "not ready yet" race).
      report("Waiting for the program to start…");
      const ready = await waitForDebuggeePid(store, debugSessionId);
      if (ready && activeSessionId === undefined) {
        report("Attaching the sampler…");
        await handleProfileAttachToDebug(store);
      }
    });
  } finally {
    // Any failure path above leaves no session — never strand the spinner.
    if (activeSessionId === undefined) { setProfilerStatus("idle"); }
  }
}

/**
 * OOTB CPU profiling for a debug-launched session: inject the in-process
 * sampler at the `stopOnEntry` pause via the courier, resume the program,
 * then adopt the streamed session. No task ports, no elevation prompt.
 */
async function startCooperativeProfileOnLaunch(
  store: Store,
  report: (message: string) => void,
): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true) { return; }

  report("Waiting for the program to pause at entry…");
  const frameId = await waitForStoppedFrame();
  if (frameId === null) {
    vscode.window.showWarningMessage(
      "Basilisk: The debuggee did not pause at entry, so the profiler could not be injected.",
    );
    return;
  }

  const sampleRate = vscode.workspace
    .getConfiguration("basilisk")
    .get<number>("profiler.sampleRate", DEFAULT_SAMPLE_RATE);

  try {
    const leg1 = await client.sendRequest<{ script: string; sampleFile: string } | undefined>(
      "workspace/executeCommand",
      { command: LSP_CMD.cooperativeScript, arguments: [{ sampleRate }] },
    );
    if (leg1?.script === undefined) { return; }

    report("Injecting the in-process sampler…");
    const ack = await evaluateInDebugSession(leg1.script, frameId);
    await vscode.commands.executeCommand("workbench.action.debug.continue");
    if (!ack?.includes(COOPERATIVE_ACK)) {
      vscode.window.showWarningMessage("Basilisk: Could not inject the in-process CPU sampler.");
      return;
    }

    report("Starting the sample stream…");
    const result = await client.sendRequest<StartedSession | undefined>(
      "workspace/executeCommand",
      {
        command: LSP_CMD.cooperativeAttach,
        arguments: [{ sampleFile: leg1.sampleFile, sampleRate }],
      },
    );
    if (result !== undefined && result !== null && activeSessionId === undefined) {
      adoptSession(result, `Basilisk: Profiling current file (PID ${result.pid}, in-process sampler)`);
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Cooperative profile start failed: ${msg}`);
    vscode.window.showErrorMessage(`Basilisk: ${msg}`);
  }
}

// ── Command handlers ──────────────────────────────────────────────────────

async function handleProfileStart(): Promise<void> {
  // [PROFILE-PROCESSES-LAUNCH] #62: the profiler no longer asks the user to
  // hand-type a PID (and there was never a real "auto-detect"). Reveal the
  // Python Processes panel so they can pick a process — or run & profile the
  // current file — and start profiling with one click.
  await vscode.commands.executeCommand("basilisk.pythonProcesses.focus");
  vscode.window.showInformationMessage(
    "Basilisk: Pick a Python process in the panel to profile, or run & profile the current file.",
  );
}

/**
 * Start profiling a specific PID — the panel's one-click path. Updates the
 * shared session state (status bar, active session) so Stop, Snapshot, and
 * live progress work exactly as they do for the debug-attach flow. The LSP
 * resolves `preset` server-side; `sampleRate`/`includeNative` apply when the
 * preset is `"default"`. Implements [PROFILE-PROCESSES-LAUNCH].
 */
export async function startProfilingForPid(store: Store, pid: number, preset: string): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    vscode.window.showWarningMessage("Basilisk: Language server not running.");
    return;
  }
  if (activeSessionId !== undefined) {
    vscode.window.showWarningMessage(
      `Basilisk: Already profiling PID ${activePid ?? "?"} (session ${activeSessionId}). Stop it first.`,
    );
    return;
  }

  const cfg = vscode.workspace.getConfiguration("basilisk");
  const args: Record<string, unknown> = {
    pid,
    preset,
    sampleRate: cfg.get<number>("profiler.sampleRate", DEFAULT_SAMPLE_RATE),
    includeNative: cfg.get<boolean>("profiler.includeNative", false),
  };

  setProfilerStatus("starting");
  try {
    await withUserProgress(CPU_START_TITLE, async (report) => {
      report(`Attaching to PID ${pid}…`);
      const result = await client.sendRequest<
        { sessionId: string; pid: number; pythonVersion: string } | undefined
      >("workspace/executeCommand", {
        command: LSP_CMD.start,
        arguments: [args],
      });

      if (result !== undefined && result !== null) {
        adoptSession(result, `Basilisk: Profiling PID ${result.pid} (Python ${result.pythonVersion})`);
      }
    });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Profile start failed: ${msg}`);
    vscode.window.showErrorMessage(`Basilisk: ${msg}`);
  } finally {
    if (activeSessionId === undefined) { setProfilerStatus("idle"); }
  }
}

async function handleProfileStop(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true || activeSessionId === undefined) {
    vscode.window.showWarningMessage("Basilisk: No active profiling session.");
    return;
  }

  try {
    // Collecting samples + writing artifacts takes a beat — show it
    // ([PROFILE-UX-PROGRESS]).
    const result = await withUserProgress(
      "Basilisk: Stopping profiler",
      async (report) => {
        report("Collecting results…");
        return client.sendRequest<ProfileResult | undefined>("workspace/executeCommand", {
          command: LSP_CMD.stop,
          arguments: [{ sessionId: activeSessionId, format: "speedscope" }],
        });
      },
    );

    cleanupSession();

    if (result !== undefined && result !== null) {
      lastResult = result;
      applyProfileDecorations(result);
      // Open the V8 .cpuprofile in VS Code's built-in profile viewer (flame
      // chart + bottom-up/left-heavy tables); fall back to the speedscope-style
      // webview only if the file wasn't produced. Open BESIDE the source so
      // the profiled file (and its inline heat map) stays visible — opening
      // in the active group hides the file and the visible-editors re-apply
      // would clear its decorations.
      if (result.cpuProfilePath !== undefined && result.cpuProfilePath !== "") {
        await vscode.commands.executeCommand(
          "vscode.open",
          vscode.Uri.file(result.cpuProfilePath),
          vscode.ViewColumn.Beside,
        );
      } else {
        openFlamegraphWebview(result);
      }
      Logger.info(
        `Profiling stopped: ${result.totalSamples} samples, ${result.duration.toFixed(1)}s, ` +
        `output: ${result.outputFile}`,
      );
      vscode.window.showInformationMessage(
        `Basilisk: Profile complete \u2014 ${result.totalSamples} samples in ${result.duration.toFixed(1)}s`,
      );
    }
  } catch (err: unknown) {
    cleanupSession();
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Profile stop failed: ${msg}`);
    vscode.window.showErrorMessage(`Basilisk: ${msg}`);
  }
}

async function handleProfileSnapshot(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true || activeSessionId === undefined) {
    vscode.window.showWarningMessage("Basilisk: No active profiling session.");
    return;
  }

  try {
    const result = await client.sendRequest<ProfileResult | undefined>("workspace/executeCommand", {
      command: LSP_CMD.snapshot,
      arguments: [{ sessionId: activeSessionId }],
    });

    if (result !== undefined && result !== null) {
      lastResult = result;
      applyProfileDecorations(result);
      Logger.info(`Profile snapshot: ${result.totalSamples} samples so far`);
      vscode.window.showInformationMessage(
        `Basilisk: Snapshot \u2014 ${result.totalSamples} samples (profiling continues)`,
      );
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Profile snapshot failed: ${msg}`);
    vscode.window.showErrorMessage(`Basilisk: ${msg}`);
  }
}

async function handleProfileAttachToDebug(store: Store): Promise<void> {
  const session = vscode.debug.activeDebugSession;
  if (session === undefined) {
    vscode.window.showWarningMessage("Basilisk: No active debug session to profile.");
    return;
  }

  const client = store.client.value;
  if (client?.isRunning() !== true) {
    vscode.window.showWarningMessage("Basilisk: Language server not running.");
    return;
  }

  if (activeSessionId !== undefined) {
    vscode.window.showWarningMessage(
      `Basilisk: Already profiling (session ${activeSessionId}).`,
    );
    return;
  }

  // Resolve the debuggee's PID captured from the DAP `process` event so we
  // profile the SAME process the debugger is attached to. The LSP profiler is
  // PID-based; the privilege layer handles elevation (macOS helper) transparently.
  const pid = store.getDebuggeeProcessId(session.id);
  if (pid === undefined) {
    vscode.window.showWarningMessage(
      "Basilisk: The debuggee process isn't ready yet — let it start running, then run “Profile Debug Session” again.",
    );
    return;
  }

  const cfg = vscode.workspace.getConfiguration("basilisk");
  const sampleRate = cfg.get<number>("profiler.sampleRate", DEFAULT_SAMPLE_RATE);
  const includeNative = cfg.get<boolean>("profiler.includeNative", false);

  try {
    const result = await client.sendRequest<{ sessionId: string; pid: number; pythonVersion: string } | undefined>("workspace/executeCommand", {
      command: LSP_CMD.start,
      arguments: [{ pid, sampleRate, includeNative }],
    });

    if (result !== undefined && result !== null) {
      adoptSession(result, `Basilisk: Profiling debug session (PID ${result.pid})`);
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Profile attach-to-debug failed: ${msg}`);
    vscode.window.showErrorMessage(`Basilisk: ${msg}`);
  }
}

// ── Progress listener ─────────────────────────────────────────────────────

function registerProgressListener(store: Store): void {
  // Check periodically if the client is available and register the handler.
  const interval = setInterval(() => {
    const client = store.client.value;
    if (client?.isRunning() === true) {
      clearInterval(interval);
      client.onNotification(PROFILER_PROGRESS_NOTIFICATION, (params: {
        sessionId: string;
        sampleCount: number;
        duration: number;
        topFunction: string;
      }) => {
        if (params.sessionId === activeSessionId) {
          updateProfilerProgress(params.sampleCount, params.duration, params.topFunction);
        }
      });
    }
  }, POLL_INTERVAL_MS);

  // Clean up interval if client never starts.
  setTimeout(() => { clearInterval(interval); }, STARTUP_TIMEOUT_MS);
}

// ── Session cleanup ───────────────────────────────────────────────────────

function cleanupSession(): void {
  activeSessionId = undefined;
  activePid = undefined;
  void vscode.commands.executeCommand("setContext", "basilisk.profiling", false);
  setProfilerStatus("idle");
}

/** Dispose all profiler resources. */
export function disposeProfiler(): void {
  cleanupSession();
  disposeProfileDecorations();
  disposeFlamegraphPanel();
  lastResult = undefined;
}
