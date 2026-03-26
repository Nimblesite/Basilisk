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
import * as path from "path";
import { Logger } from "./logger";
import type { Store } from "./store";
import {
  applyProfileDecorations,
  clearProfileDecorations,
  disposeProfileDecorations,
  type ProfileResult,
} from "./profiler-decorations";

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
    vscode.commands.registerCommand("basilisk.profileStart", () => handleProfileStart(store)),
    vscode.commands.registerCommand("basilisk.profileStop", () => handleProfileStop(store)),
    vscode.commands.registerCommand("basilisk.profileSnapshot", () => handleProfileSnapshot(store)),
    vscode.commands.registerCommand("basilisk.profileAttachToDebug", () => handleProfileAttachToDebug(store)),
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

  return disposables;
}

// ── Command handlers ──────────────────────────────────────────────────────

async function handleProfileStart(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    vscode.window.showWarningMessage("Basilisk: Language server not running.");
    return;
  }

  if (activeSessionId !== undefined) {
    vscode.window.showWarningMessage(
      `Basilisk: Already profiling PID ${activePid ?? "?"} (session ${activeSessionId}).`,
    );
    return;
  }

  // Prompt for PID or auto-detect.
  const pidInput = await vscode.window.showInputBox({
    prompt: "Python process PID (leave empty to auto-detect)",
    placeHolder: "e.g. 12345",
    validateInput: (value) => {
      if (value === "") { return null; }
      const num = Number(value);
      if (!Number.isInteger(num) || num <= 0) {
        return "Enter a valid positive integer PID";
      }
      return null;
    },
  });

  if (pidInput === undefined) { return; } // Cancelled.

  const cfg = vscode.workspace.getConfiguration("basilisk");
  const sampleRate = cfg.get<number>("profiler.sampleRate", 100);
  const includeNative = cfg.get<boolean>("profiler.includeNative", false);

  const args: Record<string, unknown> = { sampleRate, includeNative };
  if (pidInput !== "") {
    args.pid = Number(pidInput);
  }

  try {
    const result = await client.sendRequest("workspace/executeCommand", {
      command: LSP_CMD.start,
      arguments: [args],
    }) as { sessionId: string; pid: number; pythonVersion: string } | undefined;

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
    const result = await client.sendRequest("workspace/executeCommand", {
      command: LSP_CMD.stop,
      arguments: [{ sessionId: activeSessionId, format: "speedscope" }],
    }) as ProfileResult | undefined;

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
    const result = await client.sendRequest("workspace/executeCommand", {
      command: LSP_CMD.snapshot,
      arguments: [{ sessionId: activeSessionId }],
    }) as ProfileResult | undefined;

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

  const cfg = vscode.workspace.getConfiguration("basilisk");
  const sampleRate = cfg.get<number>("profiler.sampleRate", 100);
  const includeNative = cfg.get<boolean>("profiler.includeNative", false);

  try {
    const result = await client.sendRequest("workspace/executeCommand", {
      command: LSP_CMD.start,
      arguments: [{ debugSession: session.id, sampleRate, includeNative }],
    }) as { sessionId: string; pid: number; pythonVersion: string } | undefined;

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

function updateProfilerProgress(sampleCount: number, duration: number, topFunction: string): void {
  if (profilerStatusBarItem === undefined) { return; }
  const durationStr = duration < 60
    ? `${duration.toFixed(0)}s`
    : `${(duration / 60).toFixed(1)}m`;
  const samplesStr = sampleCount >= 1000
    ? `${(sampleCount / 1000).toFixed(1)}K`
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
  }, 500);

  // Clean up interval after 30s if client never starts.
  setTimeout(() => { clearInterval(interval); }, 30_000);
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

function buildFlamegraphHtml(result: ProfileResult): string {
  const hotFunctionsJson = JSON.stringify(result.hotFunctions);
  const hotLinesJson = JSON.stringify(result.hotLines);

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Basilisk Profiler</title>
  <style>
    :root {
      --prof-critical: #e8500a;
      --prof-hot: #f97316;
      --prof-warm: #fbbf24;
      --prof-cool: #4a5468;
      --prof-idle: #1a1f2e;
      --prof-bg: #0a0c12;
      --prof-surface: #141820;
      --prof-border: #1a1f2e;
      --prof-text: #f0f2f7;
      --prof-text-secondary: #8892a4;
    }
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      background: var(--prof-bg);
      color: var(--prof-text);
      font-family: 'Space Grotesk', -apple-system, sans-serif;
      padding: 16px;
    }
    h1 {
      font-size: 18px;
      font-weight: 600;
      margin-bottom: 16px;
      display: flex;
      align-items: center;
      gap: 8px;
    }
    h1 .accent { color: var(--prof-critical); }
    .summary-cards {
      display: flex;
      gap: 12px;
      margin-bottom: 20px;
      flex-wrap: wrap;
    }
    .card {
      background: var(--prof-surface);
      border: 1px solid var(--prof-border);
      border-radius: 8px;
      padding: 12px 16px;
      min-width: 120px;
    }
    .card .label {
      font-size: 11px;
      color: var(--prof-text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }
    .card .value {
      font-size: 24px;
      font-weight: 600;
      font-family: 'JetBrains Mono', monospace;
      margin-top: 4px;
    }
    h2 {
      font-size: 14px;
      font-weight: 600;
      color: var(--prof-text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.05em;
      margin: 16px 0 8px;
    }
    .fn-table {
      width: 100%;
      border-collapse: collapse;
    }
    .fn-table th {
      text-align: left;
      font-size: 11px;
      color: var(--prof-text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.05em;
      padding: 6px 8px;
      border-bottom: 1px solid var(--prof-border);
    }
    .fn-table td {
      padding: 8px;
      font-family: 'JetBrains Mono', monospace;
      font-size: 13px;
      border-bottom: 1px solid var(--prof-border);
      cursor: pointer;
    }
    .fn-table tr:hover td {
      background: var(--prof-surface);
    }
    .bar-cell {
      position: relative;
      width: 200px;
    }
    .bar {
      height: 20px;
      border-radius: 3px;
      transition: width 200ms ease;
    }
    .bar.critical { background: var(--prof-critical); }
    .bar.hot { background: var(--prof-hot); }
    .bar.warm { background: var(--prof-warm); }
    .bar.cool { background: var(--prof-cool); }
    .pct { font-weight: 500; min-width: 56px; text-align: right; }
    .file-link {
      color: var(--prof-text-secondary);
      font-size: 12px;
    }
    .file-link:hover { color: var(--prof-text); text-decoration: underline; }
    .speedscope-link {
      display: inline-block;
      margin-top: 16px;
      padding: 8px 16px;
      background: var(--prof-surface);
      border: 1px solid var(--prof-border);
      border-radius: 6px;
      color: var(--prof-text);
      text-decoration: none;
      font-size: 13px;
      cursor: pointer;
    }
    .speedscope-link:hover {
      border-color: var(--prof-critical);
      color: var(--prof-critical);
    }
  </style>
</head>
<body>
  <h1><span class="accent">BASILISK</span> PROFILER</h1>

  <div class="summary-cards">
    <div class="card">
      <div class="label">Samples</div>
      <div class="value" id="total-samples">0</div>
    </div>
    <div class="card">
      <div class="label">Duration</div>
      <div class="value" id="duration">0s</div>
    </div>
    <div class="card">
      <div class="label">Functions</div>
      <div class="value" id="fn-count">0</div>
    </div>
    <div class="card">
      <div class="label">Hot Lines</div>
      <div class="value" id="line-count">0</div>
    </div>
  </div>

  <h2>Hot Functions</h2>
  <table class="fn-table">
    <thead>
      <tr>
        <th>Function</th>
        <th>Location</th>
        <th>Total %</th>
        <th>Self %</th>
        <th></th>
      </tr>
    </thead>
    <tbody id="fn-body"></tbody>
  </table>

  <h2>Hot Lines</h2>
  <table class="fn-table">
    <thead>
      <tr>
        <th>Location</th>
        <th>%</th>
        <th>Samples</th>
        <th></th>
      </tr>
    </thead>
    <tbody id="line-body"></tbody>
  </table>

  <div id="speedscope-section"></div>

  <script>
    const vscode = acquireVsCodeApi();
    const hotFunctions = ${hotFunctionsJson};
    const hotLines = ${hotLinesJson};
    const totalSamples = ${result.totalSamples};
    const duration = ${result.duration};
    const outputFile = ${JSON.stringify(result.outputFile)};

    // Animate count-up for summary cards.
    function animateValue(el, target, suffix) {
      const start = 0;
      const stepTime = Math.max(10, Math.floor(400 / target));
      let current = start;
      const timer = setInterval(() => {
        current += Math.ceil(target / 40);
        if (current >= target) {
          current = target;
          clearInterval(timer);
        }
        el.textContent = (current >= 1000 ? (current / 1000).toFixed(1) + 'K' : String(current)) + (suffix || '');
      }, stepTime);
    }

    animateValue(document.getElementById('total-samples'), totalSamples, '');
    document.getElementById('duration').textContent = duration < 60 ? duration.toFixed(1) + 's' : (duration / 60).toFixed(1) + 'm';
    document.getElementById('fn-count').textContent = String(hotFunctions.length);
    document.getElementById('line-count').textContent = String(hotLines.length);

    function heatClass(pct) {
      if (pct >= 20) return 'critical';
      if (pct >= 10) return 'hot';
      if (pct >= 5) return 'warm';
      return 'cool';
    }

    function basename(filePath) {
      return filePath.split(/[\\/\\\\]/).pop() || filePath;
    }

    // Render hot functions.
    const fnBody = document.getElementById('fn-body');
    for (const fn of hotFunctions) {
      const tr = document.createElement('tr');
      tr.onclick = () => vscode.postMessage({ type: 'navigateToSource', file: fn.file, line: fn.line });
      tr.innerHTML = [
        '<td>' + fn.name + '</td>',
        '<td class="file-link">' + basename(fn.file) + ':' + fn.line + '</td>',
        '<td class="pct">' + fn.percentage.toFixed(1) + '%</td>',
        '<td class="pct">' + fn.selfPercentage.toFixed(1) + '%</td>',
        '<td class="bar-cell"><div class="bar ' + heatClass(fn.percentage) + '" style="width:' + Math.max(4, fn.percentage * 2) + 'px"></div></td>',
      ].join('');
      fnBody.appendChild(tr);
    }

    // Render hot lines.
    const lineBody = document.getElementById('line-body');
    for (const line of hotLines) {
      const tr = document.createElement('tr');
      tr.onclick = () => vscode.postMessage({ type: 'navigateToSource', file: line.file, line: line.line });
      tr.innerHTML = [
        '<td class="file-link">' + basename(line.file) + ':' + line.line + '</td>',
        '<td class="pct">' + line.percentage.toFixed(1) + '%</td>',
        '<td>' + line.samples + '</td>',
        '<td class="bar-cell"><div class="bar ' + heatClass(line.percentage) + '" style="width:' + Math.max(4, line.percentage * 2) + 'px"></div></td>',
      ].join('');
      lineBody.appendChild(tr);
    }

    // Speedscope link.
    if (outputFile) {
      const section = document.getElementById('speedscope-section');
      const link = document.createElement('div');
      link.className = 'speedscope-link';
      link.textContent = 'Open in Speedscope (external)';
      link.title = outputFile;
      link.onclick = () => {
        vscode.postMessage({ type: 'openExternal', url: 'https://www.speedscope.app/#profileURL=file://' + encodeURIComponent(outputFile) });
      };
      section.appendChild(link);
    }
  </script>
</body>
</html>`;
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
