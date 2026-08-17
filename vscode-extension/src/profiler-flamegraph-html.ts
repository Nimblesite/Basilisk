// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * CPU profile results webview for the Basilisk profiler.
 *
 * Owns the results panel end to end: the interactive flame graph hero (the
 * inferno SVG the LSP exports on every stop, [PROFILE-FLAMEGRAPH]), summary
 * cards, hot function/line tables with click-to-source navigation, and the
 * result-landing flow that never dead-ends the user (#145): the panel is the
 * primary view on stop, with the raw V8 trace one click away.
 *
 * Panel lifecycle, CSP, and safe data embedding come from the shared webview
 * host ([PROFILE-WEBVIEW-HOST], profiler-webview.ts).
 */

import { readFileSync } from "node:fs";
import * as vscode from "vscode";
import { Logger } from "./logger";
import { serveProfileForBrowser } from "./profile-server";
import type { ProfileResult } from "./profiler-decorations";
import {
  PROFILER_CSS_VARS,
  PROFILER_CSS_RESET,
  PROFILER_CSS_CARDS,
  PROFILER_CSS_TABLE,
  PROFILER_CSS_HEADING,
  PROFILER_JS_UTILS,
} from "./profiler-styles";
import {
  buildWebviewDocument,
  embedJson,
  handleSourceNavigation,
  SingletonWebviewPanel,
  type WebviewMessage,
} from "./profiler-webview";

// Implements [PROFILE-NATIVE-FALLBACK]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-NATIVE-FALLBACK
// ── Result presentation (#145) ─────────────────────────────────────────────

/** Completion-toast actions that always reach a working view (#145). */
const OPEN_NATIVE_TRACE = "Open Trace in VS Code Viewer";
const REVEAL_TRACE = "Reveal Trace File";

/**
 * Largest flame graph SVG embedded inline as a data URI (bytes). Anything
 * bigger still opens externally via the panel button; it is just not inlined.
 */
const MAX_INLINE_SVG_BYTES = 4_194_304;

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
 * The self-contained results panel (summary cards, flame graph hero, navigable
 * hot-function/line tables) opens immediately as the primary view: it always
 * renders, while VS Code's built-in `.cpuprofile` viewer lands on a raw
 * self/total-time table that reads as a wall of numbers until the user finds
 * its flame icon. The native trace stays one deliberate click away — the
 * completion toast's "Open Trace in VS Code Viewer" action, the panel's own
 * button, or the "Basilisk: Show Profile Results" re-entry command — so the
 * user is never stranded and never dumped somewhere confusing.
 */
export function presentProfileResult(result: ProfileResult): void {
  if (profileHasNoUsableData(result)) {
    presentNoUsableData(result);
    return;
  }
  openFlamegraphWebview(result);
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
 * Open a generated profile artifact in VS Code's built-in viewer beside the
 * source, falling back when no file was produced OR when the built-in viewer's
 * open throws (unavailable in the host, or a file it rejects). Opening beside
 * keeps the profiled file (and its inline decorations) visible. The CPU
 * (`.cpuprofile`) and memory (`.heapprofile`) paths share it through
 * `openNativeTraceViewer` so the open-beside-else-fall-back primitive lives
 * once (dry-1).
 */
async function openNativeProfileViewerBeside(
  filePath: string,
  fallback: () => void,
): Promise<void> {
  if (filePath === "") {
    fallback();
    return;
  }
  try {
    await vscode.commands.executeCommand(
      "vscode.open",
      vscode.Uri.file(filePath),
      vscode.ViewColumn.Beside,
    );
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.warn(`Built-in profile viewer failed to open (${msg}); using the in-extension fallback`);
    fallback();
  }
}

/**
 * Open a V8 trace (`.cpuprofile` / `.heapprofile`) in VS Code's built-in viewer
 * on demand — the user asked for it explicitly (toast action, results-panel
 * button, or memory-dashboard button), so a viewer that cannot open
 * (unavailable in the host, or no trace produced) is said out loud and the
 * trace file is revealed instead; an explicit request never fails silently.
 */
export async function openNativeTraceViewer(cpuProfilePath: string, traceFile: string): Promise<void> {
  await openNativeProfileViewerBeside(cpuProfilePath, () => {
    void vscode.window.showWarningMessage(
      "Basilisk: VS Code's built-in trace viewer could not open the profile — revealing the trace file instead.",
    );
    if (traceFile !== "") {
      void vscode.commands.executeCommand("revealFileInOS", vscode.Uri.file(traceFile));
    }
  });
}

/** Completion toast that always offers a path to the raw trace (#145). */
async function offerProfileResultActions(result: ProfileResult): Promise<void> {
  const choice = await vscode.window.showInformationMessage(
    `Basilisk: Profile complete — ${result.totalSamples} samples in ${result.duration.toFixed(1)}s`,
    OPEN_NATIVE_TRACE,
    REVEAL_TRACE,
  );
  if (choice === OPEN_NATIVE_TRACE) {
    await openNativeTraceViewer(result.cpuProfilePath ?? "", traceFileFor(result));
  } else if (choice === REVEAL_TRACE) {
    const trace = traceFileFor(result);
    if (trace !== "") {
      await vscode.commands.executeCommand("revealFileInOS", vscode.Uri.file(trace));
    }
  }
}

// ── Webview panel ─────────────────────────────────────────────────────────

/** Route a message posted from the results webview. */
function handleFlamegraphMessage(msg: WebviewMessage): void {
  if (handleSourceNavigation(msg)) {
    return;
  }
  if (msg.type === "openSpeedscope" && msg.file !== undefined && msg.file !== "") {
    void openSpeedscopeImport(msg.file);
  } else if (msg.type === "openCpuProfile" && msg.file !== undefined && msg.file !== "") {
    // The panel's "Open Trace in VS Code Viewer" button — the on-demand path to
    // the built-in `.cpuprofile` table view ([PROFILE-NATIVE]).
    void openNativeTraceViewer(msg.file, msg.file);
  } else if (msg.type === "openFlamegraphSvg" && msg.file !== undefined && msg.file !== "") {
    // The inferno SVG carries its own zoom/search interactivity, which the
    // CSP-locked webview intentionally does not run — open it externally where
    // its script works ([PROFILE-FLAMEGRAPH]).
    void vscode.env.openExternal(vscode.Uri.file(msg.file));
  }
}

const flamegraphPanel = new SingletonWebviewPanel("basilisk.flamegraph", handleFlamegraphMessage);

/** Open (or reveal) the profile results panel beside the source. */
export function openFlamegraphWebview(result: ProfileResult): void {
  flamegraphPanel.show("Basilisk Profiler", buildFlamegraphHtml(result));
}

/**
 * "Open in Speedscope": speedscope.app is served over https and cannot read
 * `file://` URLs — but browsers treat loopback as a potentially-trustworthy
 * origin, so the profile is served from the extension's 127.0.0.1 server and
 * the deep link loads it automatically ([PROFILE-VIEWER-DELIVERY]). The value
 * is left unencoded on purpose: it contains only `:` and `/` (legal raw in a
 * fragment), and percent-encoding it invites `vscode.Uri` re-encoding mangling.
 * A browser without the loopback mixed-content exemption still shows
 * speedscope's error page, so a toast always offers the drag-and-drop path;
 * a failure to serve falls back to that path directly.
 *
 * Shared by the CPU results panel (speedscope JSON) and the memory dashboard
 * (V8 `.heapprofile` — a format speedscope also imports).
 */
export async function openSpeedscopeImport(file: string): Promise<void> {
  try {
    const localUrl = await serveProfileForBrowser(file);
    await vscode.env.openExternal(
      vscode.Uri.parse(`https://www.speedscope.app/#profileURL=${localUrl}`),
    );
    void offerSpeedscopeFallback(file);
  } catch (err: unknown) {
    Logger.warn(
      `Profile loopback serving failed (${err instanceof Error ? err.message : String(err)}); ` +
        "falling back to manual speedscope import",
    );
    await vscode.commands.executeCommand("revealFileInOS", vscode.Uri.file(file));
    await vscode.env.openExternal(vscode.Uri.parse("https://www.speedscope.app/"));
    vscode.window.showInformationMessage(
      `Basilisk: Drag the revealed file into speedscope.app to view it — ${file}`,
    );
  }
}

/**
 * The always-works escape hatch behind the speedscope deep link: fire-and-forget
 * toast (sticky with a button — must not block) whose action reveals the JSON
 * for manual drag-and-drop import.
 */
async function offerSpeedscopeFallback(file: string): Promise<void> {
  const choice = await vscode.window.showInformationMessage(
    "Basilisk: Opened in speedscope.app — if the profile doesn't load, drag the trace file in.",
    "Reveal Trace File",
  );
  if (choice === "Reveal Trace File") {
    await vscode.commands.executeCommand("revealFileInOS", vscode.Uri.file(file));
  }
}

/**
 * Test seam: is the self-contained results panel currently open? Lets the
 * run→profile→view e2e assert the user reaches a working flame chart rather
 * than dead-ending on the built-in viewer's error ([PROFILE-NATIVE]).
 */
export function flamegraphPanelOpen(): boolean {
  return flamegraphPanel.isOpen();
}

/** Close and forget the panel (extension teardown). */
export function disposeFlamegraphPanel(): void {
  flamegraphPanel.dispose();
}

// ── Flame graph hero ([PROFILE-FLAMEGRAPH]) ───────────────────────────────

/**
 * Inline the LSP-exported flame graph SVG as a `data:` URI so the CSP-locked
 * webview (`img-src data:`) can render it without running its embedded script.
 * Returns undefined when there is no SVG, it cannot be read, or it is too large
 * to inline — the panel then simply omits the hero (the tables still work).
 */
export function loadFlamegraphSvgDataUri(flamegraphPath: string | undefined): string | undefined {
  if (flamegraphPath === undefined || flamegraphPath === "") {
    return undefined;
  }
  try {
    const svg = readFileSync(flamegraphPath);
    if (svg.byteLength === 0 || svg.byteLength > MAX_INLINE_SVG_BYTES) {
      return undefined;
    }
    return `data:image/svg+xml;base64,${svg.toString("base64")}`;
  } catch (err: unknown) {
    Logger.warn(
      `Flame graph SVG could not be inlined (${err instanceof Error ? err.message : String(err)})`,
    );
    return undefined;
  }
}

/** The flame graph hero markup, or an empty string when no SVG is available. */
function flamegraphHeroHtml(svgDataUri: string | undefined): string {
  if (svgDataUri === undefined) {
    return "";
  }
  return `
  <h2>Flame Graph</h2>
  <div class="flame-hero">
    <img id="flame-svg" alt="CPU flame graph" src="${svgDataUri}">
    <button class="action-link" id="open-flame-svg" title="Open the interactive flame graph (zoom and search) in your default viewer">
      Open Interactive Flame Graph
    </button>
  </div>`;
}

// ── Document assembly ─────────────────────────────────────────────────────

/** Flamegraph-specific CSS on top of the shared profiler design system. */
function flamegraphCss(): string {
  return `${PROFILER_CSS_VARS}${PROFILER_CSS_RESET}
    body { padding: 16px; }
    h1 .accent { color: var(--prof-critical); }
    ${PROFILER_CSS_HEADING}${PROFILER_CSS_CARDS}${PROFILER_CSS_TABLE}
    .data-table .pct { font-weight: 500; min-width: 56px; text-align: right; }
    .bar.critical { background: var(--prof-critical); }
    .bar.hot { background: var(--prof-hot); }
    .bar.warm { background: var(--prof-warm); }
    .bar.cool { background: var(--prof-cool); }
    .file-link { color: var(--prof-text-secondary); font-size: 12px; }
    .file-link:hover { color: var(--prof-text); text-decoration: underline; }
    .flame-hero {
      background: var(--prof-surface); border: 1px solid var(--prof-border);
      border-radius: 8px; padding: 12px; margin-bottom: 8px;
    }
    .flame-hero img { display: block; width: 100%; height: auto; border-radius: 4px; }
    .action-link {
      display: inline-block; margin-top: 12px; padding: 8px 16px;
      background: var(--prof-bg); border: 1px solid var(--prof-border);
      border-radius: 6px; color: var(--prof-text);
      font-size: 13px; cursor: pointer; font-family: inherit;
    }
    .action-link:hover { border-color: var(--prof-critical); color: var(--prof-critical); }
    .action-link + .action-link { margin-left: 8px; }`;
}

/** Build the body HTML with the flame graph hero, summary cards and tables. */
function flamegraphBody(svgDataUri: string | undefined): string {
  return `
  <h1><span class="accent">BASILISK</span> PROFILER</h1>
  <div class="summary-cards">
    <div class="card"><div class="label">Samples</div><div class="value" id="total-samples">0</div></div>
    <div class="card"><div class="label">Duration</div><div class="value" id="duration">0s</div></div>
    <div class="card"><div class="label">Functions</div><div class="value" id="fn-count">0</div></div>
    <div class="card"><div class="label">Hot Lines</div><div class="value" id="line-count">0</div></div>
  </div>${flamegraphHeroHtml(svgDataUri)}
  <h2>Hot Functions</h2>
  <table class="data-table">
    <thead><tr><th>Function</th><th>Location</th><th>Total %</th><th>Self %</th><th></th></tr></thead>
    <tbody id="fn-body"></tbody>
  </table>
  <h2>Hot Lines</h2>
  <table class="data-table">
    <thead><tr><th>Location</th><th>%</th><th>Samples</th><th></th></tr></thead>
    <tbody id="line-body"></tbody>
  </table>
  <div id="trace-actions"></div>`;
}

/** Build the script initialization and helpers for the results webview. */
function flamegraphScriptInit(result: ProfileResult): string {
  return `
    const vscode = acquireVsCodeApi();
    const hotFunctions = ${embedJson(result.hotFunctions)};
    const hotLines = ${embedJson(result.hotLines)};
    const totalSamples = ${result.totalSamples};
    const duration = ${result.duration};
    const outputFile = ${embedJson(result.outputFile)};
    const flamegraphFile = ${embedJson(result.flamegraphPath ?? "")};
    const cpuProfileFile = ${embedJson(result.cpuProfilePath ?? "")};
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
    const openSvgButton = document.getElementById('open-flame-svg');
    if (openSvgButton) {
      openSvgButton.onclick = () => vscode.postMessage({ type: 'openFlamegraphSvg', file: flamegraphFile });
    }
    function heatClass(pct) {
      if (pct >= 20) return 'critical';
      if (pct >= 10) return 'hot';
      if (pct >= 5) return 'warm';
      return 'cool';
    }`;
}

/** Build the table rendering and speedscope link script. */
function flamegraphScriptRender(): string {
  return `
    const fnBody = document.getElementById('fn-body');
    for (const fn of hotFunctions) {
      const tr = document.createElement('tr');
      tr.onclick = () => vscode.postMessage({ type: 'navigateToSource', file: fn.file, line: fn.line });
      // fn.name/fn.file are CPython co_name/co_filename from the profiled (possibly
      // third-party) program — escape before innerHTML. This also renders synthetic
      // frames like <module>/<lambda> literally instead of as broken HTML elements.
      tr.innerHTML = [
        '<td>' + escapeHtml(fn.name) + '</td>',
        '<td class="file-link">' + escapeHtml(basename(fn.file)) + ':' + fn.line + '</td>',
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
        '<td class="file-link">' + escapeHtml(basename(line.file)) + ':' + line.line + '</td>',
        '<td class="pct">' + line.percentage.toFixed(1) + '%</td>',
        '<td>' + line.samples + '</td>',
        '<td class="bar-cell"><div class="bar ' + heatClass(line.percentage) + '" style="width:' + Math.max(4, line.percentage * 2) + 'px"></div></td>',
      ].join('');
      lineBody.appendChild(tr);
    }
    const section = document.getElementById('trace-actions');
    if (cpuProfileFile) {
      const nativeButton = document.createElement('button');
      nativeButton.className = 'action-link';
      nativeButton.textContent = 'Open Trace in VS Code Viewer';
      nativeButton.title = 'Open the raw .cpuprofile in the built-in trace viewer (self/total time table)';
      nativeButton.onclick = () => {
        vscode.postMessage({ type: 'openCpuProfile', file: cpuProfileFile });
      };
      section.appendChild(nativeButton);
    }
    if (outputFile) {
      const link = document.createElement('button');
      link.className = 'action-link';
      link.textContent = 'Open in Speedscope (external)';
      link.title = 'Open speedscope.app with the profile loaded automatically';
      link.onclick = () => {
        vscode.postMessage({ type: 'openSpeedscope', file: outputFile });
      };
      section.appendChild(link);
    }`;
}

/** Build the complete results-panel HTML for the profiler webview. */
export function buildFlamegraphHtml(result: ProfileResult): string {
  const svgDataUri = loadFlamegraphSvgDataUri(result.flamegraphPath);
  return buildWebviewDocument({
    title: "Basilisk Profiler",
    css: flamegraphCss(),
    body: flamegraphBody(svgDataUri),
    script: `${PROFILER_JS_UTILS}${flamegraphScriptInit(result)}${flamegraphScriptRender()}`,
  });
}
