// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Flamegraph webview for the Basilisk profiler.
 *
 * Owns the results panel end to end: the complete HTML document (CSS
 * styling, summary cards, function/line tables, count-up animations) and
 * the webview panel that hosts it, including click-to-source navigation.
 *
 * Extracted from profiler.ts to satisfy the 500 LOC file limit.
 */

import * as vscode from "vscode";
import { Logger } from "./logger";
import type { ProfileResult } from "./profiler-decorations";

// Implements [PROFILE-NATIVE-FALLBACK]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-NATIVE-FALLBACK
// ── Result presentation (#145) ─────────────────────────────────────────────

/** Completion-toast actions that always reach a working view (#145). */
const OPEN_FLAME_CHART = "Open Flame Chart";
const REVEAL_TRACE = "Reveal Trace File";

// Implements [PROFILE-SHORT-PROGRAM]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-SHORT-PROGRAM
/**
 * Whether a completed profile has nothing worth showing — no hot functions and
 * no hot lines (#145).
 *
 * Raw sample count is the wrong signal: a sub-tick program (e.g.
 * `debug_demo.py` ≈ 1 ms) finishes before its work can be sampled, yet the
 * session keeps sampling the idle/exiting interpreter — so the result can carry
 * dozens of samples that resolve to zero user-code attribution. A sampling
 * profiler at 100 Hz cannot fix this by raising the rate; deterministic
 * profiling can (Phase 2). When there is no attribution, say so honestly instead
 * of presenting an empty flame chart/heat map as a result.
 */
export function profileHasNoUsableData(
  result: Pick<ProfileResult, "hotFunctions" | "hotLines">,
): boolean {
  return result.hotFunctions.length === 0 && result.hotLines.length === 0;
}

/**
 * Land a completed CPU profile on a viewable result ([PROFILE-NATIVE-FALLBACK], #145).
 *
 * Opens VS Code's built-in `.cpuprofile` viewer when one was produced, but never
 * *relies* on it: that viewer can refuse to render (unavailable in the host, or
 * a profile it rejects) and `vscode.open` does not reject when it does. So the
 * completion notification always offers an "Open Flame Chart" action onto the
 * self-contained flamegraph webview (plus "Reveal Trace File") — the user is
 * never stranded on "the editor could not be opened".
 */
export async function presentProfileResult(result: ProfileResult): Promise<void> {
  if (profileHasNoUsableData(result)) {
    presentNoUsableData(result);
    return;
  }
  await openNativeViewerOrFallback(result);
  Logger.info(
    `Profiling stopped: ${result.totalSamples} samples, ${result.duration.toFixed(1)}s, ` +
    `output: ${result.outputFile}`,
  );
  // A failed/refused export is never silent — even when there is still data to
  // show, surface why an artifact is missing ([PROFILE-SPEEDSCOPE-VALIDATE]).
  if (result.exportError !== undefined && result.exportError !== "") {
    void vscode.window.showWarningMessage(
      `Basilisk: Some profile artifacts could not be exported — ${result.exportError}`,
    );
  }
  // Fire-and-forget: a notification carrying action buttons is sticky (VS Code
  // does not auto-dismiss it), so awaiting it would block the stop handler until
  // the user clicks. Let it float — its actions open their views when chosen —
  // but swallow any rejection so it never surfaces as an unhandled promise.
  void offerProfileResultActions(result).catch((err: unknown) => {
    Logger.warn(`Profile completion action failed: ${err instanceof Error ? err.message : String(err)}`);
  });
}

/**
 * A profile with no attribution (#145, [PROFILE-SHORT-PROGRAM]). Be honest —
 * don't present an empty flame chart/heat map as a real result, and don't
 * promise a "higher sample rate" that provably cannot help a sub-tick run.
 * Fire-and-forget (a notification is sticky if it carries a button, but even
 * plain ones must not block the stop handler).
 */
function presentNoUsableData(result: ProfileResult): void {
  const detail =
    result.exportError !== undefined && result.exportError !== "" ? ` (${result.exportError})` : "";
  if (result.totalSamples === 0) {
    void vscode.window.showWarningMessage(
      `Basilisk: Profiling captured no samples — the program finished before sampling began${detail}. ` +
        "Sampling needs a longer-running target; short scripts need precise (deterministic) profiling.",
    );
    return;
  }
  void vscode.window.showWarningMessage(
    `Basilisk: Captured ${result.totalSamples} sample${result.totalSamples === 1 ? "" : "s"}, but none ` +
      "landed in your code — the program ran its work too briefly to profile by sampling. " +
      "Short scripts need precise (deterministic) profiling.",
  );
}

/** The on-disk trace a "Reveal" action points at: the `.cpuprofile`, else the speedscope JSON. */
function traceFileFor(result: ProfileResult): string {
  const cpu = result.cpuProfilePath ?? "";
  return cpu !== "" ? cpu : result.outputFile;
}

/**
 * Open the native `.cpuprofile` viewer beside the source, falling back to the
 * self-contained flamegraph webview when no profile was produced or when the
 * built-in viewer's open throws. Opening beside keeps the profiled file (and
 * its inline heat map) visible.
 */
async function openNativeViewerOrFallback(result: ProfileResult): Promise<void> {
  const cpuProfilePath = result.cpuProfilePath ?? "";
  if (cpuProfilePath === "") {
    openFlamegraphWebview(result);
    return;
  }
  try {
    await vscode.commands.executeCommand(
      "vscode.open",
      vscode.Uri.file(cpuProfilePath),
      vscode.ViewColumn.Beside,
    );
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.warn(`Built-in .cpuprofile viewer failed to open (${msg}); using the flamegraph fallback`);
    openFlamegraphWebview(result);
  }
}

/** Completion toast that always offers a path to a working view (#145). */
async function offerProfileResultActions(result: ProfileResult): Promise<void> {
  const choice = await vscode.window.showInformationMessage(
    `Basilisk: Profile complete — ${result.totalSamples} samples in ${result.duration.toFixed(1)}s`,
    OPEN_FLAME_CHART,
    REVEAL_TRACE,
  );
  if (choice === OPEN_FLAME_CHART) {
    openFlamegraphWebview(result);
  } else if (choice === REVEAL_TRACE) {
    const trace = traceFileFor(result);
    if (trace !== "") {
      await vscode.commands.executeCommand("revealFileInOS", vscode.Uri.file(trace));
    }
  }
}

// ── Webview panel ─────────────────────────────────────────────────────────

let flamegraphPanel: vscode.WebviewPanel | undefined;

/** Open (or reveal) the flamegraph results panel beside the source. */
export function openFlamegraphWebview(result: ProfileResult): void {
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
    // Register the message handler ONCE — re-opening reveals the same panel, so
    // re-registering on every call would fire each action N times.
    flamegraphPanel.webview.onDidReceiveMessage(handleFlamegraphMessage);
  }

  flamegraphPanel.webview.html = buildFlamegraphHtml(result);
}

/** Route a message posted from the flamegraph webview. */
function handleFlamegraphMessage(msg: { type: string; file?: string; line?: number }): void {
  if (msg.type === "navigateToSource" && msg.file !== undefined && msg.line !== undefined) {
    const uri = vscode.Uri.file(msg.file);
    const position = new vscode.Position(msg.line - 1, 0);
    void vscode.window.showTextDocument(uri, {
      selection: new vscode.Range(position, position),
      viewColumn: vscode.ViewColumn.One,
    });
  } else if (msg.type === "openSpeedscope" && msg.file !== undefined && msg.file !== "") {
    void openSpeedscopeImport(msg.file);
  }
}

/**
 * "Open in Speedscope": speedscope.app is served over https and cannot read
 * `file://` URLs, so a `#profileURL=file://…` deep link always fails with
 * "Something went wrong" ([PROFILE-VIEWER-DELIVERY]). Instead reveal the
 * speedscope JSON and open the app so the user can drag-and-drop it in.
 */
async function openSpeedscopeImport(file: string): Promise<void> {
  await vscode.commands.executeCommand("revealFileInOS", vscode.Uri.file(file));
  await vscode.env.openExternal(vscode.Uri.parse("https://www.speedscope.app/"));
  vscode.window.showInformationMessage(
    `Basilisk: Drag the revealed file into speedscope.app to view it — ${file}`,
  );
}

/**
 * Test seam: is the self-contained flamegraph results panel currently open?
 * Lets the run→profile→view e2e assert the user reaches a working flame chart
 * rather than dead-ending on the built-in viewer's error ([PROFILE-NATIVE]).
 */
export function flamegraphPanelOpen(): boolean {
  return flamegraphPanel !== undefined;
}

/** Close and forget the panel (extension teardown). */
export function disposeFlamegraphPanel(): void {
  if (flamegraphPanel !== undefined) {
    flamegraphPanel.dispose();
    flamegraphPanel = undefined;
  }
}

/** Build the CSS for the flamegraph webview. */
function flamegraphCssVars(): string {
  return `
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
      font-size: 18px; font-weight: 600; margin-bottom: 16px;
      display: flex; align-items: center; gap: 8px;
    }
    h1 .accent { color: var(--prof-critical); }`;
}

function flamegraphCssComponents(): string {
  return `
    .fn-table { width: 100%; border-collapse: collapse; }
    .fn-table th {
      text-align: left; font-size: 11px;
      color: var(--prof-text-secondary);
      text-transform: uppercase; letter-spacing: 0.05em;
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
    .fn-table tr:hover td { background: var(--prof-surface); }
    .bar-cell { position: relative; width: 200px; }
    .bar { height: 20px; border-radius: 3px; transition: width 200ms ease; }
    .bar.critical { background: var(--prof-critical); }
    .bar.hot { background: var(--prof-hot); }
    .bar.warm { background: var(--prof-warm); }
    .bar.cool { background: var(--prof-cool); }
    .pct { font-weight: 500; min-width: 56px; text-align: right; }
    .file-link { color: var(--prof-text-secondary); font-size: 12px; }
    .file-link:hover { color: var(--prof-text); text-decoration: underline; }
    .speedscope-link {
      display: inline-block; margin-top: 16px; padding: 8px 16px;
      background: var(--prof-surface); border: 1px solid var(--prof-border);
      border-radius: 6px; color: var(--prof-text);
      text-decoration: none; font-size: 13px; cursor: pointer;
    }
    .speedscope-link:hover { border-color: var(--prof-critical); color: var(--prof-critical); }`;
}

function flamegraphCss(): string {
  return `${flamegraphCssVars()}
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
      font-size: 14px; font-weight: 600;
      color: var(--prof-text-secondary);
      text-transform: uppercase; letter-spacing: 0.05em;
      margin: 16px 0 8px;
    }${flamegraphCssComponents()}`;
}

/** Build the body HTML with summary cards and tables. */
function flamegraphBody(): string {
  return `
  <h1><span class="accent">BASILISK</span> PROFILER</h1>
  <div class="summary-cards">
    <div class="card"><div class="label">Samples</div><div class="value" id="total-samples">0</div></div>
    <div class="card"><div class="label">Duration</div><div class="value" id="duration">0s</div></div>
    <div class="card"><div class="label">Functions</div><div class="value" id="fn-count">0</div></div>
    <div class="card"><div class="label">Hot Lines</div><div class="value" id="line-count">0</div></div>
  </div>
  <h2>Hot Functions</h2>
  <table class="fn-table">
    <thead><tr><th>Function</th><th>Location</th><th>Total %</th><th>Self %</th><th></th></tr></thead>
    <tbody id="fn-body"></tbody>
  </table>
  <h2>Hot Lines</h2>
  <table class="fn-table">
    <thead><tr><th>Location</th><th>%</th><th>Samples</th><th></th></tr></thead>
    <tbody id="line-body"></tbody>
  </table>
  <div id="speedscope-section"></div>`;
}

/** Build the script initialization and helpers for the flamegraph webview. */
function flamegraphScriptInit(result: ProfileResult): string {
  const hotFunctionsJson = JSON.stringify(result.hotFunctions);
  const hotLinesJson = JSON.stringify(result.hotLines);

  return `
    const vscode = acquireVsCodeApi();
    const hotFunctions = ${hotFunctionsJson};
    const hotLines = ${hotLinesJson};
    const totalSamples = ${result.totalSamples};
    const duration = ${result.duration};
    const outputFile = ${JSON.stringify(result.outputFile)};
    function animateValue(el, target, suffix) {
      const start = 0;
      const stepTime = Math.max(10, Math.floor(400 / target));
      let current = start;
      const timer = setInterval(() => {
        current += Math.ceil(target / 40);
        if (current >= target) { current = target; clearInterval(timer); }
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
    function basename(filePath) { return filePath.split(/[\\/\\\\]/).pop() || filePath; }`;
}

/** Build the table rendering and speedscope link script. */
function flamegraphScriptRender(): string {
  return `
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
    if (outputFile) {
      const section = document.getElementById('speedscope-section');
      const link = document.createElement('div');
      link.className = 'speedscope-link';
      link.textContent = 'Open in Speedscope (external)';
      link.title = 'Reveal the trace and open speedscope.app to drag it in';
      link.onclick = () => {
        vscode.postMessage({ type: 'openSpeedscope', file: outputFile });
      };
      section.appendChild(link);
    }`;
}

/** Build the complete flamegraph HTML for the profiler webview panel. */
export function buildFlamegraphHtml(result: ProfileResult): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Basilisk Profiler</title>
  <style>${flamegraphCss()}</style>
</head>
<body>${flamegraphBody()}
  <script>${flamegraphScriptInit(result)}${flamegraphScriptRender()}</script>
</body>
</html>`;
}
