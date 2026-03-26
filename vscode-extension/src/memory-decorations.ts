/**
 * Inline memory allocation decorations for Basilisk memory profiling.
 *
 * After a memory snapshot, hot allocation lines get purple-palette
 * decorations showing allocation size and count. Uses the Basilisk
 * memory profiler brand palette:
 *
 *   Critical: #c084fc  — purple (high allocation)
 *   Hot:      #a78bfa  — lighter purple
 *   Leak:     #f87171  — red (suspected leak)
 *   Freed:    #34d399  — green (memory freed)
 */

import * as vscode from "vscode";
import { Logger } from "./logger";

// ── Types ─────────────────────────────────────────────────────────────────

/** Per-line allocation data from the LSP server. */
export interface MemoryAllocation {
  file: string;
  line: number;
  size: number;
  count: number;
}

/** Memory snapshot result from the LSP. */
export interface MemorySnapshotResult {
  memorySessionId: string;
  snapshotId: string;
  currentMemory: number;
  peakMemory: number;
  topAllocations: MemoryAllocation[];
}

// ── Decoration types ──────────────────────────────────────────────────────

const memDecorationTypes = new Map<string, vscode.TextEditorDecorationType>();

function getMemDecorationTypeForColor(color: string): vscode.TextEditorDecorationType {
  let existing = memDecorationTypes.get(color);
  if (existing !== undefined) { return existing; }

  existing = vscode.window.createTextEditorDecorationType({
    overviewRulerColor: color,
    overviewRulerLane: vscode.OverviewRulerLane.Right,
    isWholeLine: false,
    after: {
      margin: "0 0 0 2em",
      fontStyle: "normal",
      fontWeight: "500",
    },
  });
  memDecorationTypes.set(color, existing);
  return existing;
}

// ── Formatting ────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) { return `${(bytes / 1_073_741_824).toFixed(1)} GB`; }
  if (bytes >= 1_048_576) { return `${(bytes / 1_048_576).toFixed(1)} MB`; }
  if (bytes >= 1024) { return `${(bytes / 1024).toFixed(1)} KB`; }
  return `${bytes} B`;
}

function memColor(size: number): string {
  if (size >= 104_857_600) { return "#c084fc"; } // >100 MB — critical purple
  if (size >= 10_485_760) { return "#a78bfa"; }  // >10 MB — hot purple
  if (size >= 1_048_576) { return "#8b5cf6"; }   // >1 MB — warm purple
  return "#7c3aed";                                // <1 MB — base purple
}

// ── Public API ────────────────────────────────────────────────────────────

/** Apply memory allocation decorations to visible editors. */
export function applyMemoryDecorations(result: MemorySnapshotResult): void {
  clearMemoryDecorations();

  const allocsByFile = new Map<string, MemoryAllocation[]>();
  for (const alloc of result.topAllocations) {
    const existing = allocsByFile.get(alloc.file);
    if (existing !== undefined) {
      existing.push(alloc);
    } else {
      allocsByFile.set(alloc.file, [alloc]);
    }
  }

  for (const editor of vscode.window.visibleTextEditors) {
    const filePath = editor.document.uri.fsPath;
    const allocs = allocsByFile.get(filePath);
    if (allocs === undefined) { continue; }

    const optionsByColor = new Map<string, vscode.DecorationOptions[]>();

    for (const alloc of allocs) {
      const color = memColor(alloc.size);
      const text = ` \u2588\u2588\u2588\u2588 ${formatBytes(alloc.size)} allocated (${alloc.count} objects)`;
      const lineIdx = alloc.line - 1;
      if (lineIdx < 0 || lineIdx >= editor.document.lineCount) { continue; }

      const range = new vscode.Range(
        new vscode.Position(lineIdx, 0),
        new vscode.Position(lineIdx, editor.document.lineAt(lineIdx).text.length),
      );

      const option: vscode.DecorationOptions = {
        range,
        renderOptions: { after: { contentText: text, color } },
      };

      const existing = optionsByColor.get(color);
      if (existing !== undefined) {
        existing.push(option);
      } else {
        optionsByColor.set(color, [option]);
      }
    }

    for (const [color, options] of optionsByColor) {
      editor.setDecorations(getMemDecorationTypeForColor(color), options);
    }
  }

  Logger.info(`Memory decorations applied: ${result.topAllocations.length} allocations`);
}

/** Clear all memory decorations from visible editors. */
export function clearMemoryDecorations(): void {
  for (const decorationType of memDecorationTypes.values()) {
    for (const editor of vscode.window.visibleTextEditors) {
      editor.setDecorations(decorationType, []);
    }
  }
}

/** Dispose all memory decoration types. */
export function disposeMemoryDecorations(): void {
  for (const decorationType of memDecorationTypes.values()) {
    decorationType.dispose();
  }
  memDecorationTypes.clear();
}
