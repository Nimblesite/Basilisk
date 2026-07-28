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
import * as path from "path";
import * as fs from "fs";
import { Logger } from "./logger";

/**
 * Key a file path so the profiler's paths and the editor's agree.
 *
 * The heat map matches profile rows to open editors by path. Those two paths
 * come from different producers and are only textually identical on POSIX:
 * the profiler reports the interpreter's own filename, while `Uri.fsPath` hands
 * back what VS Code resolved. On Windows those disagree in two ways — the drive
 * letter's case, and 8.3 short components (see `expandedWindowsPath`) — so a raw
 * string compare NEVER matches there and the heat map silently paints nothing:
 * the profile is correct, the editor just stays blank. Windows paths are
 * case-insensitive, so folding case there is sound; POSIX paths are
 * case-SENSITIVE, so it must not fold there.
 * Implements [PROFILE-VIS-HEATMAP] on Windows ([VSIX-CI-PLATFORM-COVERAGE]).
 */
function pathKey(file: string): string {
  return process.platform === "win32" ? expandedWindowsPath(file).toLowerCase() : path.resolve(file);
}

/**
 * A Windows path with any 8.3 short component expanded to its real name.
 *
 * Case-folding alone is not enough. The two producers reach the same file by
 * different routes: `os.tmpdir()` yields the short form Windows keeps for
 * legacy callers (`C:\Users\RUNNER~1\…`), while a path the debug adapter or the
 * editor resolved carries the long one (`C:\Users\runneradmin\…`). Those differ
 * in more than case, so `path.resolve` leaves them unequal and the heat map
 * matches nothing.
 *
 * `realpathSync.native` is what collapses the two spellings — it asks the
 * filesystem for the name it actually records. It therefore touches disk and
 * throws for a path that no longer exists, so a file that has since been
 * deleted falls back to the resolved-but-unexpanded form rather than taking the
 * whole decoration pass down with it. Callers memoise (see `pathKeyer`) so this
 * costs one lookup per distinct file, not one per hot line.
 */
function expandedWindowsPath(file: string): string {
  try {
    return fs.realpathSync.native(file);
  } catch {
    return path.resolve(file);
  }
}

/**
 * A `pathKey` memoised for the length of one decoration pass.
 *
 * A profile carries many hot lines across few files, and the expansion above
 * hits the filesystem — so the same handful of paths would otherwise be looked
 * up hundreds of times. The cache is per-pass rather than module-level so a
 * file that moves between runs is never matched against a stale name.
 */
function pathKeyer(): (file: string) => string {
  const cache = new Map<string, string>();
  return (file: string): string => {
    const hit = cache.get(file);
    if (hit !== undefined) { return hit; }
    const key = pathKey(file);
    cache.set(file, key);
    return key;
  };
}

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
  /**
   * Path to the self-contained interactive flame graph SVG the LSP always
   * attempts to export ([PROFILE-FLAMEGRAPH]). Embedded as the hero of the
   * results webview and openable externally for full zoom/search.
   */
  flamegraphPath?: string;
  /**
   * Why an export was refused or failed ([PROFILE-SPEEDSCOPE-VALIDATE]) — e.g.
   * a zero-sample run. Surfaced to the user; never silent (#145).
   */
  exportError?: string;
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

// ── Applied-decoration ledger ─────────────────────────────────────────────
// Implements [PROFILE-VIS-HEATMAP] observability: `setDecorations` is
// write-only in the VS Code API, so the heat map records what it applied and
// exposes it for e2e assertions (per the VSIX internal-state testing rule).

/** One heat decoration as applied to an open editor. */
export interface AppliedDecoration {
  readonly file: string;
  /** 1-based line the decoration landed on. */
  readonly line: number;
  /** Palette color used (e.g. `#e8500a` for critical). */
  readonly color: string;
  /** The after-line text shown to the user (bar + % + samples). */
  readonly contentText: string;
}

let appliedDecorations: AppliedDecoration[] = [];

/** The heat-map decorations currently applied across visible editors. */
export function appliedProfileDecorations(): readonly AppliedDecoration[] {
  return appliedDecorations;
}

/** Record everything painted for one file, for the applied ledger. */
export function recordApplied(
  ledger: AppliedDecoration[],
  file: string,
  paintedByColor: ReadonlyMap<string, readonly vscode.DecorationOptions[]>,
): void {
  for (const [color, options] of paintedByColor) {
    for (const option of options) {
      ledger.push({
        file,
        line: option.range.start.line + 1,
        color,
        contentText: option.renderOptions?.after?.contentText ?? "",
      });
    }
  }
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
/** Group items by normalised file path (see `pathKey`) into a map. */
function groupByFile<T extends { file: string }>(
  items: T[],
  keyOf: (file: string) => string,
): Map<string, T[]> {
  const grouped = new Map<string, T[]>();
  for (const item of items) {
    const key = keyOf(item.file);
    const existing = grouped.get(key);
    if (existing !== undefined) {
      existing.push(item);
    } else {
      grouped.set(key, [item]);
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

  const keyOf = pathKeyer();
  const linesByFile = groupByFile(result.hotLines, keyOf);
  const funcsByFile = groupByFile(result.hotFunctions, keyOf);

  const editors = vscode.window.visibleTextEditors;
  for (const editor of editors) {
    const filePath = editor.document.uri.fsPath;
    const key = keyOf(filePath);
    const optionsByColor = new Map<string, vscode.DecorationOptions[]>();

    const lines = linesByFile.get(key);
    if (lines !== undefined) {
      buildLineDecorations(editor, lines, optionsByColor);
    }

    const funcs = funcsByFile.get(key);
    if (funcs !== undefined) {
      buildFuncDecorations(editor, funcs, { lines, optionsByColor });
    }

    for (const [color, options] of optionsByColor) {
      editor.setDecorations(getDecorationTypeForColor(color), options);
    }
    recordApplied(appliedDecorations, filePath, optionsByColor);
  }

  // Count what was PAINTED, not what the profile held: a profile full of hot
  // lines that matches no open editor used to log exactly like a successful
  // pass, which is how the heat map stayed silently blank on Windows.
  Logger.info(
    `Profile decorations applied: ${appliedDecorations.length} across ${editors.length} visible editor(s)`
    + ` from ${result.hotLines.length} hot lines, ${result.hotFunctions.length} hot functions`,
  );
}

/** Clear all profiling decorations from visible editors. */
export function clearProfileDecorations(): void {
  for (const decorationType of decorationTypes.values()) {
    for (const editor of vscode.window.visibleTextEditors) {
      editor.setDecorations(decorationType, []);
    }
  }
  appliedDecorations = [];
}

/** Dispose all decoration types (call on extension deactivate). */
export function disposeProfileDecorations(): void {
  for (const decorationType of decorationTypes.values()) {
    decorationType.dispose();
  }
  decorationTypes.clear();
  appliedDecorations = [];
}
