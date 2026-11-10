// Implements [LSPTEST-UV-INTEGRATION-COVERAGE]. See docs/specs/LSP-TEST-INTEGRATION-SPEC.md#LSPTEST-UV-INTEGRATION-COVERAGE
/**
 * Coverage gutter decorations for Basilisk test coverage results.
 *
 * Listens for `basilisk/coverageResult` notifications and renders
 * covered (green) / uncovered (red) line backgrounds in the editor.
 */

import * as vscode from "vscode";
import { Logger } from "./logger";

/** Per-line coverage data from the LSP server. */
export interface LspLineCoverage {
  line: number;
  hits: number;
}

/** Per-file coverage data from the LSP server. */
export interface LspFileCoverage {
  file: string;
  lines: LspLineCoverage[];
  coveragePct: number;
}

/** Coverage result from the LSP server. */
export interface LspCoverageResult {
  files: LspFileCoverage[];
  totalPct: number;
}

/** Decoration types for coverage gutters (created lazily). */
let coveredDecorationType: vscode.TextEditorDecorationType | undefined;
let uncoveredDecorationType: vscode.TextEditorDecorationType | undefined;

/** Get or create the "covered" gutter decoration type. */
function getCoveredDecoration(): vscode.TextEditorDecorationType {
  coveredDecorationType ??= vscode.window.createTextEditorDecorationType({
    gutterIconPath: undefined,
    overviewRulerColor: new vscode.ThemeColor("testing.iconPassed"),
    overviewRulerLane: vscode.OverviewRulerLane.Left,
    isWholeLine: true,
    backgroundColor: new vscode.ThemeColor("diffEditor.insertedLineBackground"),
  });
  return coveredDecorationType;
}

/** Get or create the "uncovered" gutter decoration type. */
function getUncoveredDecoration(): vscode.TextEditorDecorationType {
  uncoveredDecorationType ??= vscode.window.createTextEditorDecorationType({
    gutterIconPath: undefined,
    overviewRulerColor: new vscode.ThemeColor("testing.iconFailed"),
    overviewRulerLane: vscode.OverviewRulerLane.Left,
    isWholeLine: true,
    backgroundColor: new vscode.ThemeColor("diffEditor.removedLineBackground"),
  });
  return uncoveredDecorationType;
}

/**
 * Apply coverage gutter decorations to all visible editors.
 *
 * For each file in the coverage result, finds matching open editors
 * and applies covered/uncovered line decorations.
 *
 * Implements [LSPTEST-UV-INTEGRATION-COVERAGE] (VS Code side) — renders the
 * `basilisk/coverageResult` payload parsed from the deterministic coverage XML.
 */
export function applyCoverageDecorations(coverage: LspCoverageResult): void {
  const enabled = vscode.workspace
    .getConfiguration("basilisk")
    .get<boolean>("testExplorer.coverageEnabled", false);

  if (!enabled) {
    Logger.info("Coverage decorations disabled — skipping");
    return;
  }

  // Clear previous decorations from all editors.
  clearCoverageDecorations();

  const covered = getCoveredDecoration();
  const uncovered = getUncoveredDecoration();

  for (const fileCov of coverage.files) {
    // Find editors showing this file.
    const editors = vscode.window.visibleTextEditors.filter((editor) =>
      editor.document.uri.fsPath.endsWith(fileCov.file)
    );

    if (editors.length === 0) { continue; }

    const coveredRanges: vscode.Range[] = [];
    const uncoveredRanges: vscode.Range[] = [];

    for (const line of fileCov.lines) {
      // coverage.xml uses 1-based lines, VS Code uses 0-based.
      const lineIdx = line.line - 1;
      if (lineIdx < 0) { continue; }
      const range = new vscode.Range(
        new vscode.Position(lineIdx, 0),
        new vscode.Position(lineIdx, 0)
      );
      if (line.hits > 0) {
        coveredRanges.push(range);
      } else {
        uncoveredRanges.push(range);
      }
    }

    for (const editor of editors) {
      editor.setDecorations(covered, coveredRanges);
      editor.setDecorations(uncovered, uncoveredRanges);
    }
  }

  Logger.info(`Coverage applied: ${coverage.totalPct.toFixed(1)}% total`);
}

/** Clear all coverage decorations from visible editors. */
export function clearCoverageDecorations(): void {
  if (coveredDecorationType !== undefined) {
    for (const editor of vscode.window.visibleTextEditors) {
      editor.setDecorations(coveredDecorationType, []);
    }
  }
  if (uncoveredDecorationType !== undefined) {
    for (const editor of vscode.window.visibleTextEditors) {
      editor.setDecorations(uncoveredDecorationType, []);
    }
  }
}
