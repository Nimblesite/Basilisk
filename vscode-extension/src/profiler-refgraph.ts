// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Reference graph webview: Canvas 2D force-directed graph for memory
 * retention chains. Node sizing by log(size), coloring by role, edge
 * labels, click-to-expand, zoom/pan, search, layout modes (force/tree/radial).
 */

import {
    PROFILER_CSS_VARS,
    PROFILER_CSS_RESET,
    PROFILER_JS_UTILS,
} from "./profiler-styles";

// ── Types ─────────────────────────────────────────────────────────────────

/** A node in the reference graph (one Python object). */
export interface RefGraphNode {
    id: string;
    type: string;
    size: number;
    repr: string;
    refcount: number;
    role: "target" | "root" | "container" | "cycle";
    creationFile?: string;
    creationLine?: number;
}

/** An edge in the reference graph (one reference relationship). */
export interface RefGraphEdge {
    from: string;
    to: string;
    label: string;
}

/** Data returned by the `basilisk/memory/references` LSP command. */
export interface RefGraphData {
    targetType: string;
    targetSize: number;
    nodes: RefGraphNode[];
    edges: RefGraphEdge[];
    cycles: string[][];
    retentionPath: string;
}

/** Layout mode for the graph renderer. */
export type LayoutMode = "force" | "tree" | "radial";

// ── HTML builder ──────────────────────────────────────────────────────────

/**
 * Build the full HTML for the reference graph webview panel.
 *
 * The graph is rendered entirely in client-side Canvas 2D JavaScript
 * embedded in the webview. The LSP data is injected as JSON constants.
 */
export function buildRefGraphHtml(data: RefGraphData): string {
    const nodesJson = JSON.stringify(data.nodes);
    const edgesJson = JSON.stringify(data.edges);
    const cyclesJson = JSON.stringify(data.cycles);

    return [
        refGraphHead(),
        refGraphBody(data),
        refGraphScriptPart1(nodesJson, edgesJson, cyclesJson),
        refGraphScriptPart2(),
        refGraphScriptPart2b(),
        refGraphScriptPart2c(),
        refGraphScriptPart3(),
        refGraphScriptPart3nodes(),
        refGraphScriptPart3a(),
        refGraphScriptPart3b(),
        refGraphScriptPart3c(),
        `</script>\n</body>\n</html>`,
    ].join("\n");
}

function refGraphCssLocal(): string {
    return `
    ${PROFILER_CSS_VARS}
    ${PROFILER_CSS_RESET}
    body { padding: 0; overflow: hidden; }
    .toolbar {
      display: flex;
      align-items: center;
      gap: 12px;
      padding: 8px 16px;
      background: var(--prof-surface);
      border-bottom: 1px solid var(--prof-border);
      font-size: 13px;
    }
    .toolbar .title {
      font-weight: 600;
      font-size: 14px;
      color: var(--prof-mem-critical);
    }
    .toolbar input {
      background: var(--prof-bg);
      border: 1px solid var(--prof-border);
      border-radius: 4px;
      color: var(--prof-text);
      padding: 4px 8px;
      font-size: 12px;
      width: 200px;
    }
    .toolbar select {
      background: var(--prof-bg);
      border: 1px solid var(--prof-border);
      border-radius: 4px;
      color: var(--prof-text);
      padding: 4px 8px;
      font-size: 12px;
    }
    .toolbar .retention-path {
      flex: 1;
      font-family: 'JetBrains Mono', monospace;
      font-size: 11px;
      color: var(--prof-text-secondary);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
`;
}

function refGraphCssCanvas(): string {
    return `
    canvas {
      display: block;
      width: 100%;
      cursor: grab;
    }
    canvas:active { cursor: grabbing; }
    .tooltip {
      position: absolute;
      display: none;
      background: var(--prof-surface);
      border: 1px solid var(--prof-border);
      border-radius: 6px;
      padding: 8px 12px;
      font-size: 12px;
      font-family: 'JetBrains Mono', monospace;
      color: var(--prof-text);
      pointer-events: none;
      max-width: 300px;
      z-index: 10;
    }
    .tooltip .type-label { color: var(--prof-mem-critical); font-weight: 600; }
    .tooltip .size-label { color: var(--prof-mem-hot); }
`;
}

function refGraphHead(): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Basilisk Reference Graph</title>
  <style>${refGraphCssLocal()}${refGraphCssCanvas()}</style>
</head>`;
}

function refGraphBody(data: RefGraphData): string {
    return `<body>
  <div class="toolbar">
    <span class="title">RETENTION GRAPH</span>
    <span>\u2014 ${escapeHtml(data.targetType)} (${formatBytesTs(data.targetSize)})</span>
    <input type="text" id="search" placeholder="Search type or repr..." />
    <select id="layout">
      <option value="force">Force-directed</option>
      <option value="tree">Tree</option>
      <option value="radial">Radial</option>
    </select>
    <span class="retention-path">${escapeHtml(data.retentionPath)}</span>
  </div>
  <canvas id="graph"></canvas>
  <div class="tooltip" id="tooltip"></div>`;
}

function refGraphScriptPart1(nodesJson: string, edgesJson: string, cyclesJson: string): string {
    return `<script>
    const vscode = acquireVsCodeApi();
    ${PROFILER_JS_UTILS}

    const rawNodes = ${nodesJson};
    const rawEdges = ${edgesJson};
    const cycles = ${cyclesJson};
    const cycleNodeIds = new Set(cycles.flat());

    // ── Physics constants ────────────────────────────────────────────
    const REPULSION = 800;
    const SPRING_K = 0.02;
    const SPRING_REST = 120;
    const DAMPING = 0.92;
    const DT = 0.5;
    const SETTLE_FRAMES = 300;

    // ── Node/edge state ──────────────────────────────────────────────
    const nodes = rawNodes.map((n, i) => ({
      ...n,
      x: 400 + Math.cos(i * 2.399) * 200,
      y: 300 + Math.sin(i * 2.399) * 200,
      vx: 0,
      vy: 0,
      radius: Math.max(12, Math.min(50, Math.log(Math.max(n.size, 1)) * 4)),
    }));
    const nodeMap = new Map(nodes.map(n => [n.id, n]));

    // ── Canvas setup ─────────────────────────────────────────────────
    const canvas = document.getElementById('graph');
    const ctx = canvas.getContext('2d');
    const tooltip = document.getElementById('tooltip');
    let width = window.innerWidth;
    let height = window.innerHeight - 44;
    let scale = 1;
    let offsetX = 0;
    let offsetY = 0;
    let dragging = false;
    let dragStartX = 0;
    let dragStartY = 0;
    let selectedNode = null;
    let frame = 0;
    let layoutMode = 'force';
    let searchTerm = '';

    function resize() {
      width = window.innerWidth;
      height = window.innerHeight - 44;
      canvas.width = width * devicePixelRatio;
      canvas.height = height * devicePixelRatio;
      canvas.style.height = height + 'px';
      ctx.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
    }
    resize();
    window.addEventListener('resize', resize);

`;
}

function refGraphScriptPart2(): string {
    return `
    // ── Layout modes ─────────────────────────────────────────────────
    function applyTreeLayout() {
      const levels = new Map();
      const visited = new Set();
      const roots = nodes.filter(n => n.role === 'root' || n.role === 'target');
      const queue = roots.map((r, i) => { levels.set(r.id, 0); visited.add(r.id); return r; });
      let idx = 0;
      while (idx < queue.length) {
        const node = queue[idx++];
        const level = levels.get(node.id) || 0;
        for (const edge of rawEdges) {
          const neighborId = edge.from === node.id ? edge.to : (edge.to === node.id ? edge.from : null);
          if (neighborId && !visited.has(neighborId)) {
            visited.add(neighborId);
            levels.set(neighborId, level + 1);
            const neighbor = nodeMap.get(neighborId);
            if (neighbor) queue.push(neighbor);
          }
        }
      }
      const maxLevel = Math.max(1, ...levels.values());
      const byLevel = new Map();
      for (const [id, level] of levels) { byLevel.set(level, (byLevel.get(level) || []).concat(id)); }
      for (const [level, ids] of byLevel) {
        const spacing = width / (ids.length + 1);
        ids.forEach((id, i) => {
          const node = nodeMap.get(id);
          if (node) { node.x = spacing * (i + 1); node.y = (level / maxLevel) * (height - 80) + 40; node.vx = 0; node.vy = 0; }
        });
      }
    }

    function applyRadialLayout() {
      const target = nodes.find(n => n.role === 'target') || nodes[0];
      if (!target) return;
      target.x = width / 2;
      target.y = height / 2;
      const others = nodes.filter(n => n !== target);
      others.forEach((n, i) => {
        const angle = (i / others.length) * Math.PI * 2;
        const ring = 1 + Math.floor(i / 8);
        n.x = target.x + Math.cos(angle) * ring * 100;
        n.y = target.y + Math.sin(angle) * ring * 100;
        n.vx = 0;
        n.vy = 0;
      });
    }

    function setLayout(mode) {
      layoutMode = mode;
      if (mode === 'tree') { applyTreeLayout(); }
      else if (mode === 'radial') { applyRadialLayout(); }
      frame = 0;
    }

    document.getElementById('layout').addEventListener('change', e => setLayout(e.target.value));
    document.getElementById('search').addEventListener('input', e => { searchTerm = e.target.value.toLowerCase(); });

`;
}

function refGraphScriptPart2b(): string {
    return `
    // ── Color scheme (Basilisk design system) ────────────────────────
    function nodeColor(node) {
      if (cycleNodeIds.has(node.id)) return '#f87171';
      switch (node.role) {
        case 'target': return '#c084fc';
        case 'root': return '#60a5fa';
        case 'container': return '#8892a4';
        default: return '#8892a4';
      }
    }

    function nodeGlow(node) {
      if (cycleNodeIds.has(node.id)) return 'rgba(248,113,113,0.4)';
      if (node.role === 'target') return 'rgba(192,132,252,0.3)';
      return null;
    }

    function matchesSearch(node) {
      if (!searchTerm) return true;
      return node.type.toLowerCase().includes(searchTerm) ||
             node.repr.toLowerCase().includes(searchTerm);
    }

    // ── Physics simulation (force-directed) ──────────────────────────
    function simulate() {
      if (layoutMode !== 'force') return;
      for (const node of nodes) {
        node.vx *= DAMPING;
        node.vy *= DAMPING;
      }
      // Repulsion.
      for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
          const dx = nodes[j].x - nodes[i].x;
          const dy = nodes[j].y - nodes[i].y;
          const dist = Math.max(1, Math.sqrt(dx * dx + dy * dy));
          const force = REPULSION / (dist * dist);
          const fx = (dx / dist) * force;
          const fy = (dy / dist) * force;
          nodes[i].vx -= fx;
          nodes[i].vy -= fy;
          nodes[j].vx += fx;
          nodes[j].vy += fy;
        }
      }
`;
}

function refGraphScriptPart2c(): string {
    return `
      // Spring attraction along edges.
      for (const edge of rawEdges) {
        const a = nodeMap.get(edge.from);
        const b = nodeMap.get(edge.to);
        if (!a || !b) continue;
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dist = Math.max(1, Math.sqrt(dx * dx + dy * dy));
        const force = SPRING_K * (dist - SPRING_REST);
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        a.vx += fx;
        a.vy += fy;
        b.vx -= fx;
        b.vy -= fy;
      }
      // Integrate.
      for (const node of nodes) {
        node.x += node.vx * DT;
        node.y += node.vy * DT;
      }
    }

`;
}

function refGraphScriptPart3(): string {
    return `
    // ── Rendering ────────────────────────────────────────────────────
    function draw() {
      ctx.clearRect(0, 0, width, height);
      ctx.save();
      ctx.translate(offsetX, offsetY);
      ctx.scale(scale, scale);

      // Draw edges.
      for (const edge of rawEdges) {
        const a = nodeMap.get(edge.from);
        const b = nodeMap.get(edge.to);
        if (!a || !b) continue;
        const isCycleEdge = cycleNodeIds.has(edge.from) && cycleNodeIds.has(edge.to);
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.strokeStyle = isCycleEdge ? '#f87171' : '#1a1f2e';
        ctx.lineWidth = isCycleEdge ? 3 : 1;
        ctx.stroke();

        // Edge label.
        if (edge.label) {
          const mx = (a.x + b.x) / 2;
          const my = (a.y + b.y) / 2;
          ctx.font = '10px JetBrains Mono, monospace';
          ctx.fillStyle = '#8892a4';
          ctx.textAlign = 'center';
          ctx.fillText(edge.label, mx, my - 4);
        }
      }

`;
}

function refGraphScriptPart3nodes(): string {
    return `
      // Draw nodes.
      for (const node of nodes) {
        const visible = matchesSearch(node);
        const alpha = visible ? 1 : 0.15;
        const color = nodeColor(node);
        const glow = nodeGlow(node);

        // Glow effect for targets and cycles.
        if (glow && visible) {
          const pulseScale = cycleNodeIds.has(node.id) ? 1 + Math.sin(frame * 0.08) * 0.15 : 1;
          ctx.beginPath();
          ctx.arc(node.x, node.y, node.radius * 1.6 * pulseScale, 0, Math.PI * 2);
          ctx.fillStyle = glow;
          ctx.fill();
        }

        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius, 0, Math.PI * 2);
        ctx.fillStyle = color;
        ctx.globalAlpha = alpha;
        ctx.fill();
        ctx.globalAlpha = 1;

        // Node label.
        ctx.font = '11px Space Grotesk, sans-serif';
        ctx.fillStyle = '#f0f2f7';
        ctx.textAlign = 'center';
        ctx.globalAlpha = alpha;
        ctx.fillText(node.type, node.x, node.y + node.radius + 14);
        if (node.size > 0) {
          ctx.font = '9px JetBrains Mono, monospace';
          ctx.fillStyle = '#8892a4';
          ctx.fillText(formatBytes(node.size), node.x, node.y + node.radius + 26);
        }
        ctx.globalAlpha = 1;
      }

      ctx.restore();
    }

`;
}

function refGraphScriptPart3a(): string {
    return `
    // ── Animation loop ───────────────────────────────────────────────
    function tick() {
      if (frame < SETTLE_FRAMES) { simulate(); }
      draw();
      frame++;
      requestAnimationFrame(tick);
    }
    tick();

`;
}

function refGraphScriptPart3b(): string {
    return `
    // ── Interaction: pan, zoom, click ────────────────────────────────
    canvas.addEventListener('wheel', e => {
      e.preventDefault();
      const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
      scale *= zoomFactor;
      scale = Math.max(0.1, Math.min(5, scale));
    });

    canvas.addEventListener('mousedown', e => {
      dragging = true;
      dragStartX = e.clientX - offsetX;
      dragStartY = e.clientY - offsetY;

      // Check if clicking a node.
      const mx = (e.clientX - offsetX) / scale;
      const my = (e.clientY - 44 - offsetY) / scale;
      selectedNode = null;
      for (const node of nodes) {
        const dx = mx - node.x;
        const dy = my - node.y;
        if (dx * dx + dy * dy < node.radius * node.radius) {
          selectedNode = node;
          vscode.postMessage({ type: 'expandNode', nodeId: node.id, nodeType: node.type });
          break;
        }
      }
    });

    canvas.addEventListener('mousemove', e => {
      if (dragging && !selectedNode) {
        offsetX = e.clientX - dragStartX;
        offsetY = e.clientY - dragStartY;
      }

`;
}

function refGraphScriptPart3c(): string {
    return `
      // Tooltip.
      const mx = (e.clientX - offsetX) / scale;
      const my = (e.clientY - 44 - offsetY) / scale;
      let hovered = null;
      for (const node of nodes) {
        const dx = mx - node.x;
        const dy = my - node.y;
        if (dx * dx + dy * dy < node.radius * node.radius) { hovered = node; break; }
      }
      if (hovered) {
        tooltip.style.display = 'block';
        tooltip.style.left = (e.clientX + 12) + 'px';
        tooltip.style.top = (e.clientY + 12) + 'px';
        tooltip.innerHTML =
          '<div class="type-label">' + escapeHtml(hovered.type) + '</div>' +
          '<div class="size-label">' + formatBytes(hovered.size) + '</div>' +
          '<div>' + escapeHtml(hovered.repr) + '</div>' +
          '<div>refcount: ' + hovered.refcount + '</div>' +
          (hovered.creationFile ? '<div>' + escapeHtml(hovered.creationFile) + ':' + hovered.creationLine + '</div>' : '');
      } else {
        tooltip.style.display = 'none';
      }
    });

    canvas.addEventListener('mouseup', () => { dragging = false; selectedNode = null; });

    // Handle messages from the extension (e.g., expand results).
    window.addEventListener('message', e => {
      const msg = e.data;
      if (msg.type === 'navigateToSource' && msg.file && msg.line) {
        vscode.postMessage(msg);
      }
    });`;
}

// ── Helper functions (server-side, used during HTML generation) ────────────

/** Escape HTML entities to prevent XSS in webview content. */
function escapeHtml(str: string): string {
    return str
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}

/** 1 GiB in bytes. */
const BYTES_PER_GIB = 1_073_741_824;
/** 1 MiB in bytes. */
const BYTES_PER_MIB = 1_048_576;
/** 1 KiB in bytes. */
const BYTES_PER_KIB = 1024;

/** Format byte count for display in toolbar. */
function formatBytesTs(bytes: number): string {
    if (bytes >= BYTES_PER_GIB) { return `${(bytes / BYTES_PER_GIB).toFixed(1)} GB`; }
    if (bytes >= BYTES_PER_MIB) { return `${(bytes / BYTES_PER_MIB).toFixed(1)} MB`; }
    if (bytes >= BYTES_PER_KIB) { return `${(bytes / BYTES_PER_KIB).toFixed(1)} KB`; }
    return `${bytes} B`;
}
