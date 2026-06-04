// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
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
  /** Path to the V8 `.cpuprofile` for VS Code's built-in profile viewer. */
  cpuProfilePath?: string;
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

/** Minimum percentage for "critical" heat classification. */
const HEAT_CRITICAL_PCT = 20;
/** Minimum percentage for "hot" heat classification. */
const HEAT_HOT_PCT = 10;
/** Minimum percentage for "warm" heat classification. */
const HEAT_WARM_PCT = 5;

/** Max bar length for critical heat level. */
const BAR_LEN_CRITICAL = 10;
/** Max bar length for hot heat level. */
const BAR_LEN_HOT = 8;
/** Max bar length for warm heat level. */
const BAR_LEN_WARM = 6;
/** Max bar length for cool heat level. */
const BAR_LEN_COOL = 4;

const HEAT_LEVELS: HeatLevel[] = [
  { minPct: HEAT_CRITICAL_PCT, color: "#e8500a", barChar: "\u2588", barMaxLen: BAR_LEN_CRITICAL },
  { minPct: HEAT_HOT_PCT, color: "#f97316", barChar: "\u2588", barMaxLen: BAR_LEN_HOT },
  { minPct: HEAT_WARM_PCT, color: "#fbbf24", barChar: "\u2588", barMaxLen: BAR_LEN_WARM },
  { minPct: 1, color: "#4a5468", barChar: "\u2588", barMaxLen: BAR_LEN_COOL },
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
/** Group items by file path into a map. */
function groupByFile<T extends { file: string }>(items: T[]): Map<string, T[]> {
  const grouped = new Map<string, T[]>();
  for (const item of items) {
    const existing = grouped.get(item.file);
    if (existing !== undefined) {
      existing.push(item);
    } else {
      grouped.set(item.file, [item]);
    }
  }
  return grouped;
}

/** Add a decoration option to the color-keyed options map. */
function addDecorationOption(
  optionsByColor: Map<string, vscode.DecorationOptions[]>,
  color: string,
  option: vscode.DecorationOptions,
): void {
  const existing = optionsByColor.get(color);
  if (existing !== undefined) {
    existing.push(option);
  } else {
    optionsByColor.set(color, [option]);
  }
}

/** Build line-level heat decoration options for a single editor. */
function buildLineDecorations(
  editor: vscode.TextEditor,
  lines: ProfileHotLine[],
  optionsByColor: Map<string, vscode.DecorationOptions[]>,
): void {
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
      new vscode.Position(lineIdx, editor.document.lineAt(lineIdx).text.length),
    );

    addDecorationOption(optionsByColor, level.color, {
      range,
      renderOptions: { after: { contentText: text, color: level.color } },
    });
  }
}

/** Build function-level heat decoration options for a single editor. */
function buildFuncDecorations(
  editor: vscode.TextEditor,
  funcs: ProfileHotFunction[],
  ctx: { lines: ProfileHotLine[] | undefined; optionsByColor: Map<string, vscode.DecorationOptions[]> },
): void {
  const { lines, optionsByColor } = ctx;
  for (const func of funcs) {
    const level = heatLevelFor(func.percentage);
    if (level === undefined) { continue; }
    if (lines?.some((l) => l.line === func.line) === true) { continue; }

    const text = ` ${func.name} \u2014 ${func.percentage.toFixed(1)}% CPU (${func.selfPercentage.toFixed(1)}% self)`;
    const lineIdx = func.line - 1;
    if (lineIdx < 0 || lineIdx >= editor.document.lineCount) { continue; }

    const range = new vscode.Range(
      new vscode.Position(lineIdx, 0),
      new vscode.Position(lineIdx, editor.document.lineAt(lineIdx).text.length),
    );

    addDecorationOption(optionsByColor, level.color, {
      range,
      renderOptions: { after: { contentText: text, color: level.color } },
    });
  }
}

export function applyProfileDecorations(result: ProfileResult): void {
  const showHeatMap = vscode.workspace
    .getConfiguration("basilisk")
    .get<boolean>("profiler.showInlineHeatMap", true);

  if (!showHeatMap) {
    Logger.info("Profiler heat map disabled — skipping decorations");
    return;
  }

  clearProfileDecorations();

  const linesByFile = groupByFile(result.hotLines);
  const funcsByFile = groupByFile(result.hotFunctions);

  for (const editor of vscode.window.visibleTextEditors) {
    const filePath = editor.document.uri.fsPath;
    const optionsByColor = new Map<string, vscode.DecorationOptions[]>();

    const lines = linesByFile.get(filePath);
    if (lines !== undefined) {
      buildLineDecorations(editor, lines, optionsByColor);
    }

    const funcs = funcsByFile.get(filePath);
    if (funcs !== undefined) {
      buildFuncDecorations(editor, funcs, { lines, optionsByColor });
    }

    for (const [color, options] of optionsByColor) {
      editor.setDecorations(getDecorationTypeForColor(color), options);
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
