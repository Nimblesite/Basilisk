// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-VIS-REFGRAPH
/**
 * Reference-graph webview for memory profiling.
 *
 * Renders the force-directed object-retention graph (`gc.get_referrers()` walk
 * parsed by the LSP) in a Canvas 2D webview. Extracted from `memory-profiler.ts`
 * so that module stays focused on command routing and the courier round-trip.
 *
 * Panel lifecycle, CSP, and safe data embedding come from the shared webview
 * host ([PROFILE-WEBVIEW-HOST], profiler-webview.ts) — node `repr`/type strings
 * come from the profiled program and must never escape the inline script.
 */

import {
  PROFILER_CSS_VARS,
  PROFILER_CSS_RESET,
  PROFILER_JS_UTILS,
} from "./profiler-styles";
import {
  buildWebviewDocument,
  embedJson,
  handleSourceNavigation,
  SingletonWebviewPanel,
} from "./profiler-webview";

/** Reference-graph result returned by `basilisk.memory.ingest` (kind `refs`). */
export interface ReferenceGraphResult {
  targetType: string;
  maxDepth: number;
  maxNodes: number;
  script: string;
  graph?: {
    nodes: RefGraphNode[];
    edges: RefGraphEdge[];
    cycles: number[][];
    retentionPath?: string[];
  };
}

interface RefGraphNode {
  id: number;
  type: string;
  size: number;
  repr: string;
  depth: number;
  isTarget: boolean;
}

interface RefGraphEdge {
  from: number;
  to: number;
  label: string;
}

const refGraphPanel = new SingletonWebviewPanel("basilisk.refGraph", (msg) => {
  handleSourceNavigation(msg);
});

/** Open (or reveal) the retention-graph webview for a parsed reference graph. */
export function openRefGraphWebview(result: ReferenceGraphResult): void {
  refGraphPanel.show(`Retention Graph — ${result.targetType}`, buildRefGraphHtml(result));
}

/** Dispose the reference-graph webview, if open. */
export function disposeRefGraph(): void {
  refGraphPanel.dispose();
}

function buildRefGraphCss(): string {
  return `${PROFILER_CSS_VARS}${PROFILER_CSS_RESET}
    body { padding: 16px; }
    h1 { font-size: 18px; font-weight: 600; margin-bottom: 12px; }
    h1 .accent { color: var(--prof-mem-critical); }
    .retention-path {
      background: var(--prof-surface);
      border: 1px solid var(--prof-border);
      border-radius: 8px;
      padding: 12px 16px;
      margin-bottom: 16px;
      font-family: 'JetBrains Mono', monospace;
      font-size: 12px;
      line-height: 1.8;
    }
    .retention-path .label {
      font-size: 11px;
      color: var(--prof-text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.05em;
      margin-bottom: 6px;
    }
    .retention-path .step { color: var(--prof-info); }
    .retention-path .target { color: var(--prof-mem-critical); font-weight: 600; }
    canvas { display: block; width: 100%; border-radius: 8px; background: var(--prof-surface); }
    .legend {
      display: flex; gap: 16px; margin-top: 12px; font-size: 11px;
      color: var(--prof-text-secondary);
    }
    .legend-item { display: flex; align-items: center; gap: 4px; }
    .legend-dot { width: 8px; height: 8px; border-radius: 50%; }
    .no-data { text-align: center; padding: 60px; color: var(--prof-text-secondary); }`;
}

function buildRetentionPathHtml(retentionPath: string[]): string {
  if (retentionPath.length === 0) { return ""; }
  const steps = retentionPath
    .map((step, i) => `<div class="${i === retentionPath.length - 1 ? "target" : "step"}">${escapeHtml(step)}</div>`)
    .join("\n    ");
  return `<div class="retention-path">
    <div class="label">Retention Path</div>
    ${steps}
  </div>`;
}

function buildRefGraphScriptInit(nodesJson: string, edgesJson: string, cyclesJson: string): string {
  return `
    const nodes = ${nodesJson};
    const edges = ${edgesJson};
    const cycles = ${cyclesJson};

    if (nodes.length === 0) {
      document.getElementById('graph').style.display = 'none';
      const noData = document.createElement('div');
      noData.className = 'no-data';
      noData.textContent = 'No reference graph data available. Run the memory references command with an active debug session.';
      document.body.appendChild(noData);
    } else {
      const canvas = document.getElementById('graph');
      const ctx = canvas.getContext('2d');
      // Fill the panel width so the graph is not letterboxed into 800px.
      const W = canvas.width = Math.max(800, canvas.parentElement ? canvas.clientWidth : 800);
      const H = canvas.height;
      const cycleNodeIds = new Set(cycles.flat());
      const nodeMap = new Map();
      nodes.forEach((n, i) => {
        nodeMap.set(n.id, {
          ...n,
          x: W / 2 + (Math.random() - 0.5) * W * 0.6,
          y: H / 2 + (Math.random() - 0.5) * H * 0.6,
          vx: 0, vy: 0,
          radius: Math.max(8, Math.min(30, Math.log2(Math.max(n.size, 1)) * 2)),
        });
      });
      for (let iter = 0; iter < 60; iter++) {
        const alpha = 0.3 * (1 - iter / 60);
        const nodeList = Array.from(nodeMap.values());
        for (let i = 0; i < nodeList.length; i++) {
          for (let j = i + 1; j < nodeList.length; j++) {
            const a = nodeList[i], b = nodeList[j];
            let dx = b.x - a.x, dy = b.y - a.y;
            const dist = Math.max(1, Math.sqrt(dx * dx + dy * dy));
            const force = 2000 / (dist * dist);
            dx = (dx / dist) * force * alpha;
            dy = (dy / dist) * force * alpha;
            a.x -= dx; a.y -= dy;
            b.x += dx; b.y += dy;
          }
        }
        for (const edge of edges) {
          const a = nodeMap.get(edge.from), b = nodeMap.get(edge.to);
          if (!a || !b) continue;
          let dx = b.x - a.x, dy = b.y - a.y;
          const dist = Math.max(1, Math.sqrt(dx * dx + dy * dy));
          const force = (dist - 80) * 0.01 * alpha;
          dx = (dx / dist) * force;
          dy = (dy / dist) * force;
          a.x += dx; a.y += dy;
          b.x -= dx; b.y -= dy;
        }
        for (const n of nodeList) {
          n.x = Math.max(40, Math.min(W - 40, n.x));
          n.y = Math.max(40, Math.min(H - 40, n.y));
        }
      }`;
}

function buildRefGraphScriptDraw(): string {
  return `
      const edgeColor = 'rgba(136, 146, 164, 0.3)';
      ctx.strokeStyle = edgeColor;
      ctx.lineWidth = 1;
      for (const edge of edges) {
        const a = nodeMap.get(edge.from), b = nodeMap.get(edge.to);
        if (!a || !b) continue;
        const isCycleEdge = cycleNodeIds.has(edge.from) && cycleNodeIds.has(edge.to);
        ctx.strokeStyle = isCycleEdge ? '#f87171' : edgeColor;
        ctx.lineWidth = isCycleEdge ? 2 : 1;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.stroke();
        if (edge.label) {
          const mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2;
          ctx.fillStyle = cssVar('--prof-text-secondary');
          ctx.font = '9px monospace';
          ctx.fillText(edge.label, mx + 4, my - 4);
        }
      }
      for (const n of nodeMap.values()) {
        const isCycle = cycleNodeIds.has(n.id);
        const color = n.isTarget ? '#c084fc'
          : isCycle ? '#f87171'
          : n.depth <= 1 ? '#60a5fa'
          : '#8892a4';
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
        ctx.fillStyle = color + '33';
        ctx.fill();
        ctx.strokeStyle = color;
        ctx.lineWidth = n.isTarget ? 3 : 1.5;
        ctx.stroke();
        ctx.fillStyle = cssVar('--prof-text');
        ctx.font = '10px monospace';
        ctx.textAlign = 'center';
        ctx.fillText(n.type, n.x, n.y + n.radius + 14);
        ctx.fillStyle = cssVar('--prof-text-secondary');
        ctx.font = '9px monospace';
        ctx.fillText(formatBytes(n.size), n.x, n.y + n.radius + 26);
      }
    }`;
}

function buildRefGraphScript(nodesJson: string, edgesJson: string, cyclesJson: string): string {
  return PROFILER_JS_UTILS +
    buildRefGraphScriptInit(nodesJson, edgesJson, cyclesJson) +
    buildRefGraphScriptDraw();
}

/** Build the complete retention-graph HTML (exported as an e2e seam). */
export function buildRefGraphHtml(result: ReferenceGraphResult): string {
  // Node types/reprs and edge labels are profiled-program data — embed them so
  // they can never close the inline <script> ([PROFILE-WEBVIEW-HOST]).
  const nodesJson = embedJson(result.graph?.nodes ?? []);
  const edgesJson = embedJson(result.graph?.edges ?? []);
  const cyclesJson = embedJson(result.graph?.cycles ?? []);
  const retentionPath = result.graph?.retentionPath ?? [];
  const escapedType = escapeHtml(result.targetType);

  return buildWebviewDocument({
    title: `Retention Graph — ${escapedType}`,
    css: buildRefGraphCss(),
    body: `
  <h1><span class="accent">◉</span> Retention Graph — <span class="accent">${escapedType}</span></h1>
  ${buildRetentionPathHtml(retentionPath)}
  <canvas id="graph" width="800" height="500"></canvas>
  <div class="legend">
    <div class="legend-item"><div class="legend-dot" style="background: var(--prof-mem-critical)"></div> Target object</div>
    <div class="legend-item"><div class="legend-dot" style="background: var(--prof-info)"></div> Root retainer</div>
    <div class="legend-item"><div class="legend-dot" style="background: var(--prof-text-secondary)"></div> Intermediate</div>
    <div class="legend-item"><div class="legend-dot" style="background: var(--prof-mem-leak)"></div> Cycle member</div>
  </div>`,
    script: buildRefGraphScript(nodesJson, edgesJson, cyclesJson),
  });
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
