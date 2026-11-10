// Implements [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * Subprocess mode: run `basilisk check --output json` on open/save and publish
 * the parsed diagnostics. The fallback when the LSP is disabled
 * (`basilisk.useLsp: false`). Extracted from `extension.ts` to keep activation
 * focused on the LSP/debug/profiler wiring.
 */

import * as vscode from "vscode";
import { execFile } from "child_process";
import * as path from "path";
import { numberField, stringField } from "./unknown-shape";

/** Exit code returned by `basilisk check` on internal errors. */
const BASILISK_INTERNAL_ERROR_EXIT_CODE = 3;

/**
 * Shape of a single diagnostic emitted by `basilisk check --output json`.
 *
 * Consumes [CHKARCH-CLI-OUTPUT-FAILURES]. See
 * docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI-OUTPUT-FAILURES
 */
interface BasiliskDiagnostic {
  /**
   * The rule code, absent for a file the CLI could not analyse at all.
   *
   * A parse failure is reported with a `null` code because no rule produced
   * it. Requiring a code here dropped those entries on the floor, so a file
   * with a syntax error showed a clean editor — the same blind spot the CLI's
   * JSON output had.
   */
  code?: string;
  severity: "error" | "warning";
  message: string;
  path: string;
  line: number;
  col: number;
  end_line: number;
  end_col: number;
}

/** First workspace folder path, if any. */
function workspaceRoot(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

/** Start subprocess mode: check Python documents on open/save via the CLI. */
export function startSubprocessMode(
  context: vscode.ExtensionContext,
  executablePath: string
): void {
  const collection = vscode.languages.createDiagnosticCollection("basilisk");
  context.subscriptions.push(collection);

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => {
      if (doc.languageId === "python") {checkDocument(doc, collection, executablePath);}
    })
  );
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "python") {checkDocument(doc, collection, executablePath);}
    })
  );
  context.subscriptions.push(
    vscode.workspace.onDidCloseTextDocument((doc) => { collection.delete(doc.uri); })
  );

  for (const doc of vscode.workspace.textDocuments) {
    if (doc.languageId === "python") {checkDocument(doc, collection, executablePath);}
  }
}

function checkDocument(
  doc: vscode.TextDocument,
  collection: vscode.DiagnosticCollection,
  executablePath: string
): void {
  const enabled = vscode.workspace.getConfiguration("basilisk").get<boolean>("enabled") ?? true;
  if (!enabled) {
    collection.delete(doc.uri);
    return;
  }
  if (doc.isUntitled || doc.uri.scheme !== "file") {return;}

  const filePath = doc.uri.fsPath;
  execFile(
    executablePath,
    ["check", "--output", "json", filePath],
    { cwd: workspaceRoot() },
    (error, stdout, stderr) => {
      if (error?.code === BASILISK_INTERNAL_ERROR_EXIT_CODE) {
        // Exit 3 means at least one file could not be analysed at all, and the
        // report on stdout now names them. Returning here published nothing,
        // so a file with a syntax error kept whatever squiggles it had before
        // the edit and explained itself only through a toast.
        const reported = parseDiagnostics(stdout, doc);
        collection.set(doc.uri, reported);
        if (reported.length === 0) {
          vscode.window.showWarningMessage(
            `Basilisk: internal error checking ${path.basename(filePath)}: ${stderr}`
          );
        }
        return;
      }
      if (error && typeof error.code === "number" && error.code !== 1) {
        vscode.window.showWarningMessage(
          `Basilisk: failed to run '${executablePath}'. Is it on PATH? (${error.message})`
        );
        collection.delete(doc.uri);
        return;
      }
      collection.set(doc.uri, parseDiagnostics(stdout, doc));
    }
  );
}

/**
 * Map the CLI's JSON report onto the editor diagnostics for one document.
 *
 * Exported as the parse boundary between a separate process's output and the
 * editor: it is where an unrecognised payload has to degrade to "nothing to
 * show" rather than throw inside the `execFile` callback, so it is tested
 * directly against the shapes the CLI actually emits.
 */
export function parseDiagnostics(json: string, doc: vscode.TextDocument): vscode.Diagnostic[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) {return [];}

  return parsed
    .map(narrowDiagnostic)
    .filter((item): item is BasiliskDiagnostic => item !== undefined)
    .filter((item) => item.path === doc.uri.fsPath)
    .map(toVscodeDiagnostic);
}

/**
 * Narrow one element of the CLI's JSON array to a diagnostic.
 *
 * Returns `undefined` — dropping the entry — when any required field is absent
 * or the wrong type. The CLI is a separate process on a version the extension
 * does not control, so a shape change must degrade to "no diagnostic here",
 * never to a `TypeError` inside the `execFile` callback.
 */
function narrowDiagnostic(value: unknown): BasiliskDiagnostic | undefined {
  const code = stringField(value, "code");
  const message = stringField(value, "message");
  const filePath = stringField(value, "path");
  const line = numberField(value, "line");
  const col = numberField(value, "col");
  const endLine = numberField(value, "end_line");
  const endCol = numberField(value, "end_col");
  if (
    message === undefined || filePath === undefined ||
    line === undefined || col === undefined ||
    endLine === undefined || endCol === undefined
  ) {
    return undefined;
  }
  return {
    code,
    // Anything that is not explicitly "error" is reported as a warning, matching
    // the previous behaviour of the two-way severity mapping below.
    severity: stringField(value, "severity") === "error" ? "error" : "warning",
    message,
    path: filePath,
    line,
    col,
    end_line: endLine,
    end_col: endCol,
  };
}

/** Render a narrowed CLI diagnostic as an editor diagnostic. */
function toVscodeDiagnostic(item: BasiliskDiagnostic): vscode.Diagnostic {
  const range = new vscode.Range(
    new vscode.Position(item.line - 1, item.col - 1),
    new vscode.Position(item.end_line - 1, item.end_col - 1)
  );
  const severity = item.severity === "error"
    ? vscode.DiagnosticSeverity.Error
    : vscode.DiagnosticSeverity.Warning;
  const { code } = item;
  const label = code === undefined ? item.message : `${item.message} [${code}]`;
  const diag = new vscode.Diagnostic(range, label, severity);
  diag.source = "basilisk";
  if (code !== undefined) {
    diag.code = {
      value: code,
      target: vscode.Uri.parse(`https://www.basilisk-python.dev/errors/${code}`),
    };
  }
  return diag;
}
