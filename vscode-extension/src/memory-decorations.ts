// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
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

/** Leak confidence level matching the LSP's `LeakConfidence` enum. */
export type LeakConfidence = "LOW" | "MEDIUM" | "HIGH" | "DEFINITE";

/** Per-line suspected leak data from the LSP diff diagnostics. */
export interface SuspectedLeak {
  file: string;
  line: number;
  sizeGrowth: number;
  countGrowth: number;
  currentSize: number;
  confidence: LeakConfidence;
  reason: string;
}

/** Memory diff result from the LSP. */
export interface MemoryDiffResult {
  totalGrowth: number;
  totalFreed: number;
  netGrowth: number;
  suspectedLeaks: SuspectedLeak[];
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

/** 1 GiB in bytes. */
const BYTES_PER_GIB = 1_073_741_824;
/** 1 MiB in bytes. */
const BYTES_PER_MIB = 1_048_576;
/** 1 KiB in bytes. */
const BYTES_PER_KIB = 1024;

/** Threshold for "critical" memory allocation color (100 MiB). */
const MEM_CRITICAL_THRESHOLD = 104_857_600;
/** Threshold for "hot" memory allocation color (10 MiB). */
const MEM_HOT_THRESHOLD = 10_485_760;
/** Threshold for "warm" memory allocation color (1 MiB). */
const MEM_WARM_THRESHOLD = 1_048_576;

function formatBytes(bytes: number): string {
  if (bytes >= BYTES_PER_GIB) { return `${(bytes / BYTES_PER_GIB).toFixed(1)} GB`; }
  if (bytes >= BYTES_PER_MIB) { return `${(bytes / BYTES_PER_MIB).toFixed(1)} MB`; }
  if (bytes >= BYTES_PER_KIB) { return `${(bytes / BYTES_PER_KIB).toFixed(1)} KB`; }
  return `${bytes} B`;
}

function memColor(size: number): string {
  if (size >= MEM_CRITICAL_THRESHOLD) { return "#c084fc"; } // >100 MB — critical purple
  if (size >= MEM_HOT_THRESHOLD) { return "#a78bfa"; }      // >10 MB — hot purple
  if (size >= MEM_WARM_THRESHOLD) { return "#8b5cf6"; }     // >1 MB — warm purple
  return "#7c3aed";                                          // <1 MB — base purple
}

function leakColor(confidence: LeakConfidence): string {
  switch (confidence) {
    case "DEFINITE": return "#ef4444"; // red-500 — definite leak
    case "HIGH":     return "#f87171"; // red-400 — high confidence
    case "MEDIUM":   return "#fb923c"; // orange-400 — medium confidence
    case "LOW":      return "#a78bfa"; // purple — low confidence (warmup)
  }
}

function confidenceBadge(confidence: LeakConfidence): string {
  switch (confidence) {
    case "DEFINITE": return "\u26d4 DEFINITE";
    case "HIGH":     return "\u26a0 HIGH";
    case "MEDIUM":   return "\u25cf MEDIUM";
    case "LOW":      return "\u25cb LOW";
  }
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

/** Apply leak indicator decorations with confidence badges to visible editors. */
export function applyLeakDecorations(result: MemoryDiffResult): void {
  const leaksByFile = new Map<string, SuspectedLeak[]>();
  for (const leak of result.suspectedLeaks) {
    const existing = leaksByFile.get(leak.file);
    if (existing !== undefined) {
      existing.push(leak);
    } else {
      leaksByFile.set(leak.file, [leak]);
    }
  }

  for (const editor of vscode.window.visibleTextEditors) {
    const filePath = editor.document.uri.fsPath;
    const leaks = leaksByFile.get(filePath);
    if (leaks === undefined) { continue; }

    const optionsByColor = new Map<string, vscode.DecorationOptions[]>();

    for (const leak of leaks) {
      const color = leakColor(leak.confidence);
      const badge = confidenceBadge(leak.confidence);
      const growth = formatBytes(Math.abs(leak.sizeGrowth));
      const text = ` ${badge} +${growth} leak (${leak.countGrowth} objects)`;

      const lineIdx = leak.line - 1;
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

  Logger.info(`Leak decorations applied: ${result.suspectedLeaks.length} suspected leaks`);
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
