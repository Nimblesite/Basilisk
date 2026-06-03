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
import { POLL_INTERVAL_MS, STARTUP_TIMEOUT_MS } from "./timeouts";
import {
  applyProfileDecorations,
  disposeProfileDecorations,
  type ProfileResult,
} from "./profiler-decorations";
import { buildFlamegraphHtml } from "./profiler-flamegraph-html";

// ── Constants ─────────────────────────────────────────────────────────────

/** Status bar priority — slightly lower than main Basilisk item. */
const PROFILER_STATUS_BAR_PRIORITY = 99;

/** LSP command names (must match basilisk-common constants). */
const LSP_CMD = {
  start: "basilisk.profiler.start",
  stop: "basilisk.profiler.stop",
  snapshot: "basilisk.profiler.snapshot",
  list: "basilisk.profiler.list",
} as const;

/** LSP notification for profiling progress. */
const PROFILER_PROGRESS_NOTIFICATION = "basilisk/profiler/progress";

// ── Constants ─────────────────────────────────────────────────────────────

/** Default sample rate fallback (Hz). */
const DEFAULT_SAMPLE_RATE = 100;

// ── State ─────────────────────────────────────────────────────────────────

let profilerStatusBarItem: vscode.StatusBarItem | undefined;
let activeSessionId: string | undefined;
let activePid: number | undefined;
let lastResult: ProfileResult | undefined;
let flamegraphPanel: vscode.WebviewPanel | undefined;

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

  // Status bar item.
  profilerStatusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    PROFILER_STATUS_BAR_PRIORITY,
  );
  profilerStatusBarItem.command = "basilisk.profileStop";
  updateProfilerStatusBar("idle");
  disposables.push(profilerStatusBarItem);

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
      const profileOnLaunch = vscode.workspace
        .getConfiguration("basilisk")
        .get<boolean>("profiler.profileOnLaunch", false);
      if (profileOnLaunch && session.type === "basilisk-debug" && activeSessionId === undefined) {
        Logger.info(`Profile on Launch: auto-profiling debug session ${session.id}`);
        // The debuggee PID arrives asynchronously via the DAP `process` event,
        // so wait for it before attaching (avoids a "not ready yet" race).
        void waitForDebuggeePid(store, session.id).then((ready) => {
          if (ready && activeSessionId === undefined) {
            void handleProfileAttachToDebug(store);
          }
        });
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

// ── Debuggee PID readiness ─────────────────────────────────────────────────

/**
 * Resolve once the debuggee's PID for `sessionId` is known (captured from the
 * DAP `process` event into the store), or after a bounded wait. Returns whether
 * the PID became available. Used by the "Profile on Launch" auto-attach so it
 * doesn't fire before debugpy reports the process.
 */
async function waitForDebuggeePid(store: Store, sessionId: string): Promise<boolean> {
  if (store.getDebuggeeProcessId(sessionId) !== undefined) {
    return true;
  }
  return new Promise((resolve) => {
    const interval = setInterval(() => {
      if (store.getDebuggeeProcessId(sessionId) !== undefined) {
        clearInterval(interval);
        clearTimeout(timeout);
        resolve(true);
      }
    }, POLL_INTERVAL_MS);
    // Clear BOTH timers on whichever path resolves first so neither dangles.
    const timeout = setTimeout(() => {
      clearInterval(interval);
      resolve(store.getDebuggeeProcessId(sessionId) !== undefined);
    }, STARTUP_TIMEOUT_MS);
  });
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

  try {
    const result = await client.sendRequest<{ sessionId: string; pid: number; pythonVersion: string } | undefined>("workspace/executeCommand", {
      command: LSP_CMD.start,
      arguments: [args],
    });

    if (result !== undefined && result !== null) {
      activeSessionId = result.sessionId;
      activePid = result.pid;
      void vscode.commands.executeCommand("setContext", "basilisk.profiling", true);
      updateProfilerStatusBar("profiling");
      Logger.info(`Profiling started: PID ${result.pid}, Python ${result.pythonVersion}, session ${result.sessionId}`);
      vscode.window.showInformationMessage(
        `Basilisk: Profiling PID ${result.pid} (Python ${result.pythonVersion})`,
      );
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Profile start failed: ${msg}`);
    vscode.window.showErrorMessage(`Basilisk: ${msg}`);
  }
}

async function handleProfileStop(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true || activeSessionId === undefined) {
    vscode.window.showWarningMessage("Basilisk: No active profiling session.");
    return;
  }

  try {
    const result = await client.sendRequest<ProfileResult | undefined>("workspace/executeCommand", {
      command: LSP_CMD.stop,
      arguments: [{ sessionId: activeSessionId, format: "speedscope" }],
    });

    cleanupSession();

    if (result !== undefined && result !== null) {
      lastResult = result;
      applyProfileDecorations(result);
      openFlamegraphWebview(result);
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
      activeSessionId = result.sessionId;
      activePid = result.pid;
      void vscode.commands.executeCommand("setContext", "basilisk.profiling", true);
      updateProfilerStatusBar("profiling");
      Logger.info(`Profiling debug session: PID ${result.pid}, session ${result.sessionId}`);
      vscode.window.showInformationMessage(
        `Basilisk: Profiling debug session (PID ${result.pid})`,
      );
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Profile attach-to-debug failed: ${msg}`);
    vscode.window.showErrorMessage(`Basilisk: ${msg}`);
  }
}

// ── Status bar ────────────────────────────────────────────────────────────

function updateProfilerStatusBar(state: "idle" | "profiling"): void {
  if (profilerStatusBarItem === undefined) { return; }

  if (state === "idle") {
    profilerStatusBarItem.hide();
    return;
  }

  profilerStatusBarItem.text = "$(flame) Profiling...";
  profilerStatusBarItem.tooltip = `Profiling PID ${activePid ?? "?"} \u2014 click to stop`;
  profilerStatusBarItem.backgroundColor = new vscode.ThemeColor(
    "statusBarItem.warningBackground",
  );
  profilerStatusBarItem.show();
}

/** Seconds per minute — threshold for switching from "Ns" to "N.Mm" display. */
const SECONDS_PER_MINUTE = 60;
/** Threshold for switching from raw count to "N.NK" display. */
const KILO_THRESHOLD = 1000;

function updateProfilerProgress(sampleCount: number, duration: number, topFunction: string): void {
  if (profilerStatusBarItem === undefined) { return; }
  const durationStr = duration < SECONDS_PER_MINUTE
    ? `${duration.toFixed(0)}s`
    : `${(duration / SECONDS_PER_MINUTE).toFixed(1)}m`;
  const samplesStr = sampleCount >= KILO_THRESHOLD
    ? `${(sampleCount / KILO_THRESHOLD).toFixed(1)}K`
    : String(sampleCount);
  profilerStatusBarItem.text = `$(flame) ${samplesStr} samples (${durationStr})`;
  profilerStatusBarItem.tooltip =
    `PID ${activePid ?? "?"} \u2014 ${samplesStr} samples, ${durationStr}\n` +
    `Top: ${topFunction}\nClick to stop`;
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
  updateProfilerStatusBar("idle");
}

// ── Flamegraph webview ────────────────────────────────────────────────────

function openFlamegraphWebview(result: ProfileResult): void {
  if (flamegraphPanel !== undefined) {
    flamegraphPanel.reveal(vscode.ViewColumn.Beside);
  } else {
    flamegraphPanel = vscode.window.createWebviewPanel(
      "basilisk.flamegraph",
      "Basilisk Profiler",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [],
      },
    );
    flamegraphPanel.onDidDispose(() => {
      flamegraphPanel = undefined;
    });
  }

  flamegraphPanel.webview.html = buildFlamegraphHtml(result);

  // Handle messages from the webview.
  flamegraphPanel.webview.onDidReceiveMessage((msg: { type: string; file?: string; line?: number }) => {
    if (msg.type === "navigateToSource" && msg.file !== undefined && msg.line !== undefined) {
      const uri = vscode.Uri.file(msg.file);
      const position = new vscode.Position(msg.line - 1, 0);
      void vscode.window.showTextDocument(uri, {
        selection: new vscode.Range(position, position),
        viewColumn: vscode.ViewColumn.One,
      });
    }
  });
}


/** Dispose all profiler resources. */
export function disposeProfiler(): void {
  cleanupSession();
  disposeProfileDecorations();
  if (flamegraphPanel !== undefined) {
    flamegraphPanel.dispose();
    flamegraphPanel = undefined;
  }
  lastResult = undefined;
}
