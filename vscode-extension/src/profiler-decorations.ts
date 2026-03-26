/**
 * Inline heat map decorations for Basilisk CPU profiling results.
 *
 * After profiling completes, hot lines get colored decorations in the
 * editor gutter and after-line text showing CPU percentage and sample
 * count. Uses the Basilisk profiler brand palette:
 *
 *   Critical (>20%): #e8500a  — Basilisk orange
 *   Hot (10-20%):    #f97316  — lighter orange
 *   Warm (5-10%):    #fbbf24  — amber
 *   Cool (1-5%):     #4a5468  — muted slate
 */

import * as vscode from "vscode";
import { Logger } from "./logger";

// ── Types ─────────────────────────────────────────────────────────────────

/** Per-line profiling data from the LSP server. */
export interface ProfileHotLine {
  file: string;
  line: number;
  samples: number;
  percentage: number;
}

/** Per-function profiling data from the LSP server. */
export interface ProfileHotFunction {
  name: string;
  file: string;
  line: number;
  samples: number;
  percentage: number;
  selfPercentage: number;
}

/** Profile result returned by `basilisk/profiler/stop` or `/snapshot`. */
export interface ProfileResult {
  sessionId: string;
  duration: number;
  totalSamples: number;
  outputFile: string;
  hotFunctions: ProfileHotFunction[];
  hotLines: ProfileHotLine[];
}

// ── Heat levels ───────────────────────────────────────────────────────────

interface HeatLevel {
  minPct: number;
  color: string;
  barChar: string;
  barMaxLen: number;
}

const HEAT_LEVELS: HeatLevel[] = [
  { minPct: 20, color: "#e8500a", barChar: "\u2588", barMaxLen: 10 },
  { minPct: 10, color: "#f97316", barChar: "\u2588", barMaxLen: 8 },
  { minPct: 5, color: "#fbbf24", barChar: "\u2588", barMaxLen: 6 },
  { minPct: 1, color: "#4a5468", barChar: "\u2588", barMaxLen: 4 },
];

function heatLevelFor(pct: number): HeatLevel | undefined {
  return HEAT_LEVELS.find((h) => pct >= h.minPct);
}

// ── Decoration types (created lazily, one per heat level) ─────────────────

const decorationTypes = new Map<string, vscode.TextEditorDecorationType>();

function getDecorationTypeForColor(color: string): vscode.TextEditorDecorationType {
  let existing = decorationTypes.get(color);
  if (existing !== undefined) { return existing; }

  existing = vscode.window.createTextEditorDecorationType({
    overviewRulerColor: color,
    overviewRulerLane: vscode.OverviewRulerLane.Right,
    gutterIconSize: "contain",
    isWholeLine: false,
    after: {
      margin: "0 0 0 2em",
      fontStyle: "normal",
      fontWeight: "500",
    },
  });
  decorationTypes.set(color, existing);
  return existing;
}

// ── Public API ────────────────────────────────────────────────────────────

/**
 * Apply inline heat map decorations to all visible editors based on
 * profiling results.
 */
export function applyProfileDecorations(result: ProfileResult): void {
  const showHeatMap = vscode.workspace
    .getConfiguration("basilisk")
    .get<boolean>("profiler.showInlineHeatMap", true);

  if (!showHeatMap) {
    Logger.info("Profiler heat map disabled — skipping decorations");
    return;
  }

  clearProfileDecorations();

  // Group hot lines by file path.
  const linesByFile = new Map<string, ProfileHotLine[]>();
  for (const line of result.hotLines) {
    const existing = linesByFile.get(line.file);
    if (existing !== undefined) {
      existing.push(line);
    } else {
      linesByFile.set(line.file, [line]);
    }
  }

  // Also group hot functions for function-level decorations at definition lines.
  const funcsByFile = new Map<string, ProfileHotFunction[]>();
  for (const func of result.hotFunctions) {
    const existing = funcsByFile.get(func.file);
    if (existing !== undefined) {
      existing.push(func);
    } else {
      funcsByFile.set(func.file, [func]);
    }
  }

  // Apply decorations to each visible editor.
  for (const editor of vscode.window.visibleTextEditors) {
    const filePath = editor.document.uri.fsPath;

    // Build decoration options per heat level color.
    const optionsByColor = new Map<string, vscode.DecorationOptions[]>();

    // Line-level heat decorations.
    const lines = linesByFile.get(filePath);
    if (lines !== undefined) {
      for (const line of lines) {
        const level = heatLevelFor(line.percentage);
        if (level === undefined) { continue; }

        const barLen = Math.ceil((line.percentage / level.minPct) * (level.barMaxLen / 2));
        const bar = level.barChar.repeat(Math.min(barLen, level.barMaxLen));
        const text = ` ${bar} ${line.percentage.toFixed(1)}% (${line.samples} samples)`;

        const lineIdx = line.line - 1;
        if (lineIdx < 0 || lineIdx >= editor.document.lineCount) { continue; }

        const range = new vscode.Range(
          new vscode.Position(lineIdx, 0),
          new vscode.Position(lineIdx, editor.document.lineAt(lineIdx).text.length)
        );

        const option: vscode.DecorationOptions = {
          range,
          renderOptions: {
            after: {
              contentText: text,
              color: level.color,
            },
          },
        };

        const existing = optionsByColor.get(level.color);
        if (existing !== undefined) {
          existing.push(option);
        } else {
          optionsByColor.set(level.color, [option]);
        }
      }
    }

    // Function-level heat decorations (at the def line).
    const funcs = funcsByFile.get(filePath);
    if (funcs !== undefined) {
      for (const func of funcs) {
        const level = heatLevelFor(func.percentage);
        if (level === undefined) { continue; }

        // Skip if a line decoration already exists at this line.
        if (lines?.some((l) => l.line === func.line) === true) { continue; }

        const text = ` ${func.name} \u2014 ${func.percentage.toFixed(1)}% CPU (${func.selfPercentage.toFixed(1)}% self)`;

        const lineIdx = func.line - 1;
        if (lineIdx < 0 || lineIdx >= editor.document.lineCount) { continue; }

        const range = new vscode.Range(
          new vscode.Position(lineIdx, 0),
          new vscode.Position(lineIdx, editor.document.lineAt(lineIdx).text.length)
        );

        const option: vscode.DecorationOptions = {
          range,
          renderOptions: {
            after: {
              contentText: text,
              color: level.color,
            },
          },
        };

        const existing = optionsByColor.get(level.color);
        if (existing !== undefined) {
          existing.push(option);
        } else {
          optionsByColor.set(level.color, [option]);
        }
      }
    }

    // Apply all collected decorations.
    for (const [color, options] of optionsByColor) {
      const decorationType = getDecorationTypeForColor(color);
      editor.setDecorations(decorationType, options);
    }
  }

  const totalLines = result.hotLines.length;
  const totalFuncs = result.hotFunctions.length;
  Logger.info(`Profile decorations applied: ${totalLines} hot lines, ${totalFuncs} hot functions`);
}

/** Clear all profiling decorations from visible editors. */
export function clearProfileDecorations(): void {
  for (const decorationType of decorationTypes.values()) {
    for (const editor of vscode.window.visibleTextEditors) {
      editor.setDecorations(decorationType, []);
    }
  }
}

/** Dispose all decoration types (call on extension deactivate). */
export function disposeProfileDecorations(): void {
  for (const decorationType of decorationTypes.values()) {
    decorationType.dispose();
  }
  decorationTypes.clear();
}
