/**
 * Memory profiler UI module for the Basilisk VS Code extension.
 *
 * Provides:
 * - Memory profiler command handlers (start/snapshot/stop/references)
 * - Reference graph webview (force-directed graph of object retention)
 * - Memory dashboard webview (summary cards, top allocations, leak badges)
 * - Memory status bar indicator
 *
 * All memory analysis logic lives in the LSP. This module handles
 * only the client-side UI and command routing.
 */

import * as vscode from "vscode";
import { Logger } from "./logger";
import type { Store } from "./store";
import {
  clearMemoryDecorations,
  disposeMemoryDecorations,
  type MemorySnapshotResult,
} from "./memory-decorations";

// ── Constants ─────────────────────────────────────────────────────────────

const LSP_MEM_CMD = {
  start: "basilisk.memory.start",
  snapshot: "basilisk.memory.snapshot",
  diff: "basilisk.memory.diff",
  references: "basilisk.memory.references",
  objectsByType: "basilisk.memory.objectsByType",
  gcCollect: "basilisk.memory.gcCollect",
} as const;

// ── State ─────────────────────────────────────────────────────────────────

let memoryStatusBarItem: vscode.StatusBarItem | undefined;
let activeMemorySessionId: string | undefined;
let refGraphPanel: vscode.WebviewPanel | undefined;
let memDashboardPanel: vscode.WebviewPanel | undefined;

// ── Registration ──────────────────────────────────────────────────────────

/** Register memory profiler commands and UI components. */
export function registerMemoryProfiler(
  context: vscode.ExtensionContext,
  store: Store,
): vscode.Disposable[] {
  /** Status bar priority — lower than main Basilisk item. */
  const MEMORY_STATUS_BAR_PRIORITY = 98;

  memoryStatusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    MEMORY_STATUS_BAR_PRIORITY,
  );
  memoryStatusBarItem.command = "basilisk.memoryStop";

  const disposables: vscode.Disposable[] = [
    memoryStatusBarItem,
    vscode.commands.registerCommand("basilisk.memoryStart", async () =>
      handleMemoryStart(store),
    ),
    vscode.commands.registerCommand("basilisk.memorySnapshot", async () =>
      handleMemorySnapshot(store),
    ),
    vscode.commands.registerCommand("basilisk.memoryStop", () => {
      handleMemoryStop(store);
    }),
    vscode.commands.registerCommand("basilisk.memoryReferences", async () =>
      handleMemoryReferences(store),
    ),
  ];

  return disposables;
}

/** Clean up memory profiler resources. */
export function disposeMemoryProfiler(): void {
  clearMemoryDecorations();
  disposeMemoryDecorations();
  if (refGraphPanel !== undefined) {
    refGraphPanel.dispose();
    refGraphPanel = undefined;
  }
  if (memDashboardPanel !== undefined) {
    memDashboardPanel.dispose();
    memDashboardPanel = undefined;
  }
}

// ── Command handlers ──────────────────────────────────────────────────────

async function handleMemoryStart(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    void vscode.window.showErrorMessage("Basilisk LSP not connected");
    return;
  }

  try {
    const TRACEBACK_DEPTH = 25;
    const result = await client.sendRequest<{ memorySessionId: string } | null>("workspace/executeCommand", {
      command: LSP_MEM_CMD.start,
      arguments: [{ tracebackDepth: TRACEBACK_DEPTH }],
    });

    if (result?.memorySessionId !== undefined && result.memorySessionId !== "") {
      activeMemorySessionId = result.memorySessionId;
      updateMemoryStatusBar("tracking");
      Logger.info(
        `Memory tracking started: session ${result.memorySessionId}`,
      );
    }
  } catch (err) {
    void vscode.window.showErrorMessage(
      `Memory tracking failed: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
}

async function handleMemorySnapshot(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true || activeMemorySessionId === undefined) {
    void vscode.window.showWarningMessage(
      "No active memory tracking session",
    );
    return;
  }

  try {
    const result = await client.sendRequest<MemorySnapshotResult | null>("workspace/executeCommand", {
      command: LSP_MEM_CMD.snapshot,
      arguments: [{ memorySessionId: activeMemorySessionId }],
    });

    if (result !== null) {
      Logger.info(`Memory snapshot taken: ${activeMemorySessionId}`);
    }
  } catch (err) {
    void vscode.window.showErrorMessage(
      `Memory snapshot failed: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
}

function handleMemoryStop(_store: Store): void {
  activeMemorySessionId = undefined;
  updateMemoryStatusBar("idle");
  clearMemoryDecorations();
  Logger.info("Memory tracking stopped");
}

async function handleMemoryReferences(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true) {
    void vscode.window.showErrorMessage("Basilisk LSP not connected");
    return;
  }

  const typeName = await vscode.window.showInputBox({
    prompt: "Object type to inspect (e.g. DataFrame, dict, MyClass)",
    placeHolder: "DataFrame",
  });

  if (typeName === undefined || typeName.trim() === "") {
    return;
  }

  try {
    const REF_GRAPH_MAX_DEPTH = 5;
    const REF_GRAPH_MAX_NODES = 200;
    const result = await client.sendRequest<ReferenceGraphResult | null>("workspace/executeCommand", {
      command: LSP_MEM_CMD.references,
      arguments: [
        {
          targetType: typeName.trim(),
          maxDepth: REF_GRAPH_MAX_DEPTH,
          maxNodes: REF_GRAPH_MAX_NODES,
        },
      ],
    });

    if (result !== null) {
      openRefGraphWebview(result);
    }
  } catch (err) {
    void vscode.window.showErrorMessage(
      `Reference graph failed: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
}

// ── Status bar ────────────────────────────────────────────────────────────

function updateMemoryStatusBar(state: "idle" | "tracking"): void {
  if (memoryStatusBarItem === undefined) { return; }

  if (state === "tracking") {
    memoryStatusBarItem.text = "$(eye) Memory Tracking";
    memoryStatusBarItem.tooltip =
      "Basilisk: Memory tracking active (click to stop)";
    memoryStatusBarItem.backgroundColor = new vscode.ThemeColor(
      "statusBarItem.warningBackground",
    );
    memoryStatusBarItem.show();
  } else {
    memoryStatusBarItem.hide();
  }
}

// ── Reference graph webview (Phase 5D) ────────────────────────────────────

interface ReferenceGraphResult {
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

function openRefGraphWebview(result: ReferenceGraphResult): void {
  if (refGraphPanel !== undefined) {
    refGraphPanel.reveal(vscode.ViewColumn.Beside);
  } else {
    refGraphPanel = vscode.window.createWebviewPanel(
      "basilisk.refGraph",
      `Retention Graph \u2014 ${result.targetType}`,
      vscode.ViewColumn.Beside,
      { enableScripts: true, retainContextWhenHidden: true },
    );
    refGraphPanel.onDidDispose(() => {
      refGraphPanel = undefined;
    });
  }

  refGraphPanel.webview.html = buildRefGraphHtml(result);

  refGraphPanel.webview.onDidReceiveMessage(
    (msg: { type: string; file?: string; line?: number }) => {
      if (
        msg.type === "navigateToSource" &&
        msg.file !== undefined &&
        msg.line !== undefined
      ) {
        const uri = vscode.Uri.file(msg.file);
        const position = new vscode.Position(msg.line - 1, 0);
        void vscode.window.showTextDocument(uri, {
          selection: new vscode.Range(position, position),
          viewColumn: vscode.ViewColumn.One,
        });
      }
    },
  );
}

function buildRefGraphCss(): string {
  return `
    :root {
      --mem-critical: #c084fc;
      --mem-hot: #a78bfa;
      --mem-leak: #f87171;
      --mem-freed: #34d399;
      --mem-info: #60a5fa;
      --bg: #0a0c12;
      --surface: #141820;
      --border: #1a1f2e;
      --text: #f0f2f7;
      --text-secondary: #8892a4;
    }
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { background: var(--bg); color: var(--text); font-family: 'Space Grotesk', sans-serif; padding: 16px; }
    h1 { font-size: 18px; font-weight: 600; margin-bottom: 12px; }
    h1 .accent { color: var(--mem-critical); }
    .retention-path {
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 12px 16px;
      margin-bottom: 16px;
      font-family: 'JetBrains Mono', monospace;
      font-size: 12px;
      line-height: 1.8;
    }
    .retention-path .label {
      font-size: 11px;
      color: var(--text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.05em;
      margin-bottom: 6px;
    }
    .retention-path .step { color: var(--mem-info); }
    .retention-path .target { color: var(--mem-critical); font-weight: 600; }
    canvas { display: block; border-radius: 8px; background: var(--surface); }
    .legend {
      display: flex; gap: 16px; margin-top: 12px; font-size: 11px;
      color: var(--text-secondary);
    }
    .legend-item { display: flex; align-items: center; gap: 4px; }
    .legend-dot { width: 8px; height: 8px; border-radius: 50%; }
    .no-data { text-align: center; padding: 60px; color: var(--text-secondary); }`;
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
    const vscode = acquireVsCodeApi();
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
      const W = canvas.width, H = canvas.height;
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
      ctx.strokeStyle = 'rgba(136, 146, 164, 0.3)';
      ctx.lineWidth = 1;
      for (const edge of edges) {
        const a = nodeMap.get(edge.from), b = nodeMap.get(edge.to);
        if (!a || !b) continue;
        const isCycleEdge = cycleNodeIds.has(edge.from) && cycleNodeIds.has(edge.to);
        ctx.strokeStyle = isCycleEdge ? '#f87171' : 'rgba(136, 146, 164, 0.3)';
        ctx.lineWidth = isCycleEdge ? 2 : 1;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.stroke();
        if (edge.label) {
          const mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2;
          ctx.fillStyle = '#8892a4';
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
        ctx.fillStyle = '#f0f2f7';
        ctx.font = '10px monospace';
        ctx.textAlign = 'center';
        ctx.fillText(n.type, n.x, n.y + n.radius + 14);
        ctx.fillStyle = '#8892a4';
        ctx.font = '9px monospace';
        ctx.fillText(formatBytes(n.size), n.x, n.y + n.radius + 26);
      }
    }
    function formatBytes(bytes) {
      if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + ' GB';
      if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + ' MB';
      if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB';
      return bytes + ' B';
    }`;
}

function buildRefGraphScript(nodesJson: string, edgesJson: string, cyclesJson: string): string {
  return buildRefGraphScriptInit(nodesJson, edgesJson, cyclesJson) +
    buildRefGraphScriptDraw();
}

function buildRefGraphHtml(result: ReferenceGraphResult): string {
  const nodesJson = JSON.stringify(result.graph?.nodes ?? []);
  const edgesJson = JSON.stringify(result.graph?.edges ?? []);
  const cyclesJson = JSON.stringify(result.graph?.cycles ?? []);
  const retentionPath = result.graph?.retentionPath ?? [];
  const escapedType = escapeHtml(result.targetType);

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Retention Graph \u2014 ${escapedType}</title>
  <style>${buildRefGraphCss()}</style>
</head>
<body>
  <h1><span class="accent">\u25C9</span> Retention Graph \u2014 <span class="accent">${escapedType}</span></h1>
  ${buildRetentionPathHtml(retentionPath)}
  <canvas id="graph" width="800" height="500"></canvas>
  <div class="legend">
    <div class="legend-item"><div class="legend-dot" style="background: var(--mem-critical)"></div> Target object</div>
    <div class="legend-item"><div class="legend-dot" style="background: var(--mem-info)"></div> Root retainer</div>
    <div class="legend-item"><div class="legend-dot" style="background: var(--text-secondary)"></div> Intermediate</div>
    <div class="legend-item"><div class="legend-dot" style="background: var(--mem-leak)"></div> Cycle member</div>
  </div>
  <script>${buildRefGraphScript(nodesJson, edgesJson, cyclesJson)}</script>
</body>
</html>`;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
