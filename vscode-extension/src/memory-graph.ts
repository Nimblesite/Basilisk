/**
 * Reference graph visualization webview for Basilisk memory profiling.
 *
 * Renders an interactive force-directed graph using Canvas 2D showing
 * object retention paths. Answers "What is holding on to this object?"
 *
 * The HTML template is in memory-graph-html.ts. This module handles
 * panel lifecycle and message routing.
 *
 * Brand palette (Memory):
 *   Critical: #c084fc  — purple (high retention)
 *   Hot:      #a78bfa  — lighter purple
 *   Leak:     #f87171  — red (cycles/leaks)
 *   Freed:    #34d399  — green (freed objects)
 */

import * as vscode from "vscode";
import { Logger } from "./logger";
import { buildGraphHtml } from "./memory-graph-html";

// ── Types ─────────────────────────────────────────────────────────────────

/** A single node in the reference graph from the LSP response. */
export interface GraphNode {
  id: number;
  type: string;
  size: number;
  repr: string;
  depth: number;
  isTarget: boolean;
}

/** A directed edge between two graph nodes. */
export interface GraphEdge {
  from: number;
  to: number;
  label: string;
}

/** Full reference graph data returned by the LSP. */
export interface ReferenceGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  cycles: number[][];
  retentionPath?: string[];
}

// ── State ─────────────────────────────────────────────────────────────────

let graphPanel: vscode.WebviewPanel | undefined;

// ── Public API ────────────────────────────────────────────────────────────

/** Open (or reveal) the reference graph webview with the given data. */
export function openReferenceGraphWebview(graph: ReferenceGraph): void {
  if (graphPanel !== undefined) {
    graphPanel.reveal(vscode.ViewColumn.Beside);
  } else {
    graphPanel = vscode.window.createWebviewPanel(
      "basilisk.referenceGraph",
      "Basilisk Reference Graph",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [],
      },
    );
    graphPanel.onDidDispose(() => {
      graphPanel = undefined;
    });
  }

  graphPanel.webview.html = buildGraphHtml(graph);

  graphPanel.webview.onDidReceiveMessage((msg: {
    type: string;
    file?: string;
    line?: number;
    nodeId?: number;
  }) => {
    if (msg.type === "navigateToSource" && msg.file !== undefined && msg.line !== undefined) {
      const uri = vscode.Uri.file(msg.file);
      const position = new vscode.Position(msg.line - 1, 0);
      void vscode.window.showTextDocument(uri, {
        selection: new vscode.Range(position, position),
        viewColumn: vscode.ViewColumn.One,
      });
    }
    if (msg.type === "expandNode" && msg.nodeId !== undefined) {
      Logger.info(`Reference graph: expand requested for node ${msg.nodeId}`);
    }
  });

  Logger.info(
    `Reference graph opened: ${graph.nodes.length} nodes, ${graph.edges.length} edges, ` +
    `${graph.cycles.length} cycles`,
  );
}

/** Dispose the reference graph panel if open. */
export function disposeReferenceGraph(): void {
  if (graphPanel !== undefined) {
    graphPanel.dispose();
    graphPanel = undefined;
  }
}
