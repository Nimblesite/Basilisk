// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Flamegraph HTML builder for the Basilisk profiler webview.
 *
 * Generates the complete HTML document for the profiler results panel,
 * including CSS styling, summary cards, function/line tables, and
 * interactive JavaScript for count-up animations and navigation.
 *
 * Extracted from profiler.ts to satisfy the 500 LOC file limit.
 */

import type { ProfileResult } from "./profiler-decorations";

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
      link.title = outputFile;
      link.onclick = () => {
        vscode.postMessage({ type: 'openExternal', url: 'https://www.speedscope.app/#profileURL=file://' + encodeURIComponent(outputFile) });
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
