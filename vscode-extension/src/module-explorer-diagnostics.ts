// Implements [EXTACT-MODULES-DIAGNOSTICS]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-MODULES-DIAGNOSTICS
/**
 * Diagnostic drill-down rows for the Module Explorer (GitHub #235).
 *
 * Expanding a module lists its actual diagnostics as the first children —
 * above its symbols — so the `🔴 n 🟠 n` tally on the row is n navigable rows,
 * not a dead number. Each row opens the file with the selection on the
 * diagnostic's range. Lives in its own module (module-explorer.ts is already
 * over the repo's file-size bound).
 */

import * as vscode from "vscode";

/**
 * The wire shape of one diagnostic on `ModuleNode.diagnostics`
 * ([EXTACT-DATA-MODEL]), mirroring the file's publish-diagnostics. `line` and
 * `character` are the zero-based start position.
 */
export interface DiagnosticNode {
  readonly severity: "error" | "warning";
  readonly code: string;
  readonly message: string;
  readonly line: number;
  readonly character: number;
}

/**
 * One navigable diagnostic row: message label, `code · Ln n` description
 * (1-based line), the editor's own diagnostic colours on the severity icon
 * ([EXTACT-MODULES-COUNT-STYLE]), and an open-at-range click action.
 */
export class DiagnosticTreeItem extends vscode.TreeItem {
  constructor(
    public readonly diagnostic: DiagnosticNode,
    modulePath: string,
  ) {
    super(diagnostic.message, vscode.TreeItemCollapsibleState.None);
    const isError = diagnostic.severity === "error";
    this.description = `${diagnostic.code} · Ln ${diagnostic.line + 1}`;
    this.iconPath = new vscode.ThemeIcon(
      isError ? "error" : "warning",
      new vscode.ThemeColor(isError ? "editorError.foreground" : "editorWarning.foreground"),
    );
    this.contextValue = "diagnostic";
    this.tooltip = `${diagnostic.code}: ${diagnostic.message}`;
    const range = new vscode.Range(
      diagnostic.line,
      diagnostic.character,
      diagnostic.line,
      diagnostic.character,
    );
    this.command = {
      command: "vscode.open",
      title: "Go to Diagnostic",
      arguments: [vscode.Uri.file(modulePath), { selection: range }],
    };
  }
}

/** Errors sort before warnings ([EXTACT-MODULES-DIAGNOSTICS] row order). */
function severityRank(diagnostic: DiagnosticNode): number {
  return diagnostic.severity === "error" ? 0 : 1;
}

/**
 * The diagnostic rows for one module, ordered errors-before-warnings then by
 * ascending line. The server already sorts ([EXTACT-MODULES-DIAGNOSTICS]);
 * sorting again here keeps the rendered order correct against any server.
 * `undefined` (a pre-#235 server binary) renders as no rows, same as clean.
 */
export function diagnosticItems(
  diagnostics: readonly DiagnosticNode[] | undefined,
  modulePath: string,
): DiagnosticTreeItem[] {
  return [...(diagnostics ?? [])]
    .sort((a, b) => {
      const bySeverity = severityRank(a) - severityRank(b);
      return bySeverity !== 0 ? bySeverity : a.line - b.line;
    })
    .map((diagnostic) => new DiagnosticTreeItem(diagnostic, modulePath));
}
