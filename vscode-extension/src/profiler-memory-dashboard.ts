// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Memory dashboard webview for Basilisk memory profiling.
 *
 * Provides a comprehensive memory analysis view with:
 * - Summary cards (current/peak memory, GC objects, snapshot count)
 * - Memory timeline chart (stacked area, snapshot data points)
 * - Top allocations table (sortable by size/count)
 * - Leak confidence badges (Definite/High/Medium/Low)
 * - Dual heat map toggle (CPU orange + memory purple)
 *
 * Uses the Basilisk profiler design system from profiler-styles.ts.
 */

import {
    PROFILER_CSS_VARS,
    PROFILER_CSS_RESET,
    PROFILER_CSS_CARDS,
    PROFILER_CSS_TABLE,
    PROFILER_CSS_HEADING,
    PROFILER_JS_UTILS,
} from "./profiler-styles";
import type { MemoryAllocation, SuspectedLeak, LeakConfidence } from "./memory-decorations";

// ── Types ─────────────────────────────────────────────────────────────────

/** A single memory timeline data point (one snapshot). */
export interface MemoryTimelinePoint {
    timestamp: number;
    currentMemory: number;
    peakMemory: number;
    gcObjects: number;
    byType: Record<string, number>;
}

/** Full data for the memory dashboard webview. */
export interface MemoryDashboardData {
    currentMemory: number;
    peakMemory: number;
    gcObjects: number;
    snapshotCount: number;
    timeline: MemoryTimelinePoint[];
    topAllocations: MemoryAllocation[];
    suspectedLeaks: SuspectedLeak[];
    heatMapMode: "cpu" | "memory" | "dual";
}

// ── HTML builder ──────────────────────────────────────────────────────────

/**
 * Build the full HTML for the memory dashboard webview panel.
 *
 * Renders summary cards, a Canvas 2D timeline chart, an allocations table,
 * and a leak summary with confidence badges.
 */
export function buildMemoryDashboardHtml(data: MemoryDashboardData): string {
    const timelineJson = JSON.stringify(data.timeline);
    const allocsJson = JSON.stringify(data.topAllocations);
    const leaksJson = JSON.stringify(data.suspectedLeaks);

    return [
        dashboardHead(),
        dashboardBody(data),
        dashboardScriptPart1({ timelineJson, allocsJson, leaksJson, data }),
        dashboardScriptPart1a(),
        dashboardScriptPart1b(),
        dashboardScriptPart2(),
        `</script>\n</body>\n</html>`,
    ].join("\n");
}

function dashboardCssComponents(): string {
    return `
    .badge-definite { background: #ef4444; color: #fff; }
    .badge-high { background: #f87171; color: #fff; }
    .badge-medium { background: #fb923c; color: #fff; }
    .badge-low { background: #a78bfa; color: #fff; }
    .heatmap-toggle { display: flex; gap: 8px; margin: 16px 0; }
    .heatmap-btn {
      padding: 6px 14px; background: var(--prof-surface);
      border: 1px solid var(--prof-border); border-radius: 6px;
      color: var(--prof-text); font-size: 12px; cursor: pointer;
    }
    .heatmap-btn.active { border-color: var(--prof-mem-critical); color: var(--prof-mem-critical); }
    .heatmap-btn:hover { border-color: var(--prof-mem-hot); }
    .leak-row { cursor: pointer; }
    .leak-row:hover td { background: var(--prof-surface); }
    .growth-positive { color: var(--prof-mem-leak); }
    .growth-negative { color: var(--prof-success); }`;
}

function dashboardHead(): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Basilisk Memory Dashboard</title>
  <style>
    ${PROFILER_CSS_VARS}
    ${PROFILER_CSS_RESET}
    ${PROFILER_CSS_CARDS}
    ${PROFILER_CSS_TABLE}
    ${PROFILER_CSS_HEADING}
    body { padding: 16px; }
    h1 .accent { color: var(--prof-mem-critical); }
    .card .value.mem { color: var(--prof-mem-critical); }
    .card .value.peak { color: var(--prof-mem-hot); }
    .card .value.gc { color: var(--prof-info); }
    .card .value.snaps { color: var(--prof-success); }
    .timeline-container {
      background: var(--prof-surface);
      border: 1px solid var(--prof-border);
      border-radius: 8px;
      padding: 12px;
      margin-bottom: 20px;
    }
    canvas { width: 100%; height: 200px; display: block; }
    .badge {
      display: inline-block;
      padding: 2px 8px;
      border-radius: 4px;
      font-size: 11px;
      font-weight: 600;
      font-family: 'JetBrains Mono', monospace;
    }
    ${dashboardCssComponents()}
  </style>
</head>`;
}

function dashboardBody(data: MemoryDashboardData): string {
    return `<body>
  <h1><span class="accent">BASILISK</span> MEMORY DASHBOARD</h1>
  <div class="summary-cards">
    <div class="card"><div class="label">Current Memory</div><div class="value mem" id="current-mem">0</div></div>
    <div class="card"><div class="label">Peak Memory</div><div class="value peak" id="peak-mem">0</div></div>
    <div class="card"><div class="label">GC Objects</div><div class="value gc" id="gc-objects">0</div></div>
    <div class="card"><div class="label">Snapshots</div><div class="value snaps" id="snap-count">0</div></div>
  </div>
  <h2>Memory Timeline</h2>
  <div class="timeline-container"><canvas id="timeline" height="200"></canvas></div>
  <h2>Top Allocations</h2>
  <table class="data-table">
    <thead><tr><th>Location</th><th>Size</th><th>Objects</th><th></th></tr></thead>
    <tbody id="alloc-body"></tbody>
  </table>
  <h2>Suspected Leaks</h2>
  <table class="data-table">
    <thead><tr><th>Location</th><th>Confidence</th><th>Growth</th><th>Reason</th></tr></thead>
    <tbody id="leak-body"></tbody>
  </table>
  <h2>Heat Map Mode</h2>
  <div class="heatmap-toggle">
    <button class="heatmap-btn${data.heatMapMode === "cpu" ? " active" : ""}" data-mode="cpu">CPU (Orange)</button>
    <button class="heatmap-btn${data.heatMapMode === "memory" ? " active" : ""}" data-mode="memory">Memory (Purple)</button>
    <button class="heatmap-btn${data.heatMapMode === "dual" ? " active" : ""}" data-mode="dual">Dual (CPU + Memory)</button>
  </div>`;
}

interface DashboardScriptData {
    timelineJson: string;
    allocsJson: string;
    leaksJson: string;
    data: MemoryDashboardData;
}

function dashboardScriptPart1(scriptData: DashboardScriptData): string {
    const { data } = scriptData;
    return `<script>
    const vscode = acquireVsCodeApi();
    ${PROFILER_JS_UTILS}

    const currentMemory = ${data.currentMemory};
    const peakMemory = ${data.peakMemory};
    const gcObjects = ${data.gcObjects};
    const snapshotCount = ${data.snapshotCount};
    const timeline = ${scriptData.timelineJson};
    const allocations = ${scriptData.allocsJson};
    const leaks = ${scriptData.leaksJson};

    // ── Summary cards ──────────────────────────────────────────────
    document.getElementById('current-mem').textContent = formatBytes(currentMemory);
    document.getElementById('peak-mem').textContent = formatBytes(peakMemory);
    document.getElementById('gc-objects').textContent =
      gcObjects >= 1000 ? (gcObjects / 1000).toFixed(1) + 'K' : String(gcObjects);
    document.getElementById('snap-count').textContent = String(snapshotCount);

    // ── Timeline chart (Canvas 2D) ─────────────────────────────────
    const canvas = document.getElementById('timeline');
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;

    function drawTimeline() {
      const rect = canvas.getBoundingClientRect();
      canvas.width = rect.width * dpr;
      canvas.height = 200 * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const w = rect.width;
      const h = 200;
      const pad = { top: 10, right: 10, bottom: 30, left: 60 };
      const plotW = w - pad.left - pad.right;
      const plotH = h - pad.top - pad.bottom;

      if (timeline.length === 0) {
        ctx.fillStyle = '#8892a4';
        ctx.font = '13px Space Grotesk, sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText('No snapshot data yet', w / 2, h / 2);
        return;
      }

      const maxMem = Math.max(...timeline.map(p => p.peakMemory), 1);
      const minT = timeline[0].timestamp;
      const maxT = timeline[timeline.length - 1].timestamp || minT + 1;
      const rangeT = maxT - minT || 1;

      function xOf(t) { return pad.left + ((t - minT) / rangeT) * plotW; }
      function yOf(v) { return pad.top + plotH - (v / maxMem) * plotH; }

`;
}

function dashboardScriptPart1a(): string {
    return `
      // Axes.
      ctx.strokeStyle = '#1a1f2e';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(pad.left, pad.top);
      ctx.lineTo(pad.left, pad.top + plotH);
      ctx.lineTo(pad.left + plotW, pad.top + plotH);
      ctx.stroke();

      // Y-axis labels.
      ctx.fillStyle = '#8892a4';
      ctx.font = '10px JetBrains Mono, monospace';
      ctx.textAlign = 'right';
      for (let i = 0; i <= 4; i++) {
        const val = (maxMem / 4) * i;
        const y = yOf(val);
        ctx.fillText(formatBytes(val), pad.left - 6, y + 4);
        ctx.strokeStyle = '#1a1f2e';
        ctx.beginPath();
        ctx.moveTo(pad.left, y);
        ctx.lineTo(pad.left + plotW, y);
        ctx.stroke();
      }

`;
}

function dashboardScriptPart1b(): string {
    return `
      // Memory area fill.
      ctx.beginPath();
      ctx.moveTo(xOf(timeline[0].timestamp), yOf(0));
      for (const pt of timeline) {
        ctx.lineTo(xOf(pt.timestamp), yOf(pt.currentMemory));
      }
      ctx.lineTo(xOf(timeline[timeline.length - 1].timestamp), yOf(0));
      ctx.closePath();
      ctx.fillStyle = 'rgba(192, 132, 252, 0.15)';
      ctx.fill();

      // Memory line.
      ctx.beginPath();
      for (let i = 0; i < timeline.length; i++) {
        const pt = timeline[i];
        const method = i === 0 ? 'moveTo' : 'lineTo';
        ctx[method](xOf(pt.timestamp), yOf(pt.currentMemory));
      }
      ctx.strokeStyle = '#c084fc';
      ctx.lineWidth = 2;
      ctx.stroke();

      // Peak line.
      ctx.beginPath();
      for (let i = 0; i < timeline.length; i++) {
        const pt = timeline[i];
        const method = i === 0 ? 'moveTo' : 'lineTo';
        ctx[method](xOf(pt.timestamp), yOf(pt.peakMemory));
      }
      ctx.strokeStyle = '#a78bfa';
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.stroke();
      ctx.setLineDash([]);

      // Snapshot dots.
      for (const pt of timeline) {
        ctx.beginPath();
        ctx.arc(xOf(pt.timestamp), yOf(pt.currentMemory), 4, 0, Math.PI * 2);
        ctx.fillStyle = '#c084fc';
        ctx.fill();
      }
    }
    drawTimeline();
    window.addEventListener('resize', drawTimeline);

`;
}

function dashboardScriptPart2(): string {
    return `
    // ── Allocations table ──────────────────────────────────────────
    const allocBody = document.getElementById('alloc-body');
    const sorted = [...allocations].sort((a, b) => b.size - a.size);
    for (const alloc of sorted) {
      const tr = document.createElement('tr');
      tr.className = 'leak-row';
      tr.onclick = () => vscode.postMessage({
        type: 'navigateToSource', file: alloc.file, line: alloc.line,
      });
      const pct = currentMemory > 0 ? ((alloc.size / currentMemory) * 100).toFixed(1) : '0.0';
      tr.innerHTML = [
        '<td>' + escapeHtml(basename(alloc.file)) + ':' + alloc.line + '</td>',
        '<td>' + formatBytes(alloc.size) + '</td>',
        '<td>' + alloc.count + '</td>',
        '<td class="bar-cell"><div class="bar" style="width:' + Math.max(4, parseFloat(pct) * 1.5) + 'px;background:var(--prof-mem-critical)"></div></td>',
      ].join('');
      allocBody.appendChild(tr);
    }

    // ── Leaks table ────────────────────────────────────────────────
    function badgeClass(conf) {
      return 'badge badge-' + conf.toLowerCase();
    }
    const leakBody = document.getElementById('leak-body');
    for (const leak of leaks) {
      const tr = document.createElement('tr');
      tr.className = 'leak-row';
      tr.onclick = () => vscode.postMessage({
        type: 'navigateToSource', file: leak.file, line: leak.line,
      });
      const growthClass = leak.sizeGrowth > 0 ? 'growth-positive' : 'growth-negative';
      tr.innerHTML = [
        '<td>' + escapeHtml(basename(leak.file)) + ':' + leak.line + '</td>',
        '<td><span class="' + badgeClass(leak.confidence) + '">' + leak.confidence + '</span></td>',
        '<td class="' + growthClass + '">+' + formatBytes(Math.abs(leak.sizeGrowth)) + '</td>',
        '<td>' + escapeHtml(leak.reason) + '</td>',
      ].join('');
      leakBody.appendChild(tr);
    }

    // ── Heat map toggle ────────────────────────────────────────────
    for (const btn of document.querySelectorAll('.heatmap-btn')) {
      btn.addEventListener('click', () => {
        for (const b of document.querySelectorAll('.heatmap-btn')) b.classList.remove('active');
        btn.classList.add('active');
        vscode.postMessage({ type: 'setHeatMapMode', mode: btn.dataset.mode });
      });
    }`;
}

/**
 * Determine the CSS badge class for a leak confidence level.
 * Exported for use in other modules that need badge styling.
 */
export function leakBadgeClass(confidence: LeakConfidence): string {
    switch (confidence) {
        case "DEFINITE": return "badge-definite";
        case "HIGH":     return "badge-high";
        case "MEDIUM":   return "badge-medium";
        case "LOW":      return "badge-low";
    }
}
