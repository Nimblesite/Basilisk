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

/** Exit code returned by `basilisk check` on internal errors. */
const BASILISK_INTERNAL_ERROR_EXIT_CODE = 3;

/** Shape of a single diagnostic emitted by `basilisk check --output json`. */
interface BasiliskDiagnostic {
  code: string;
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
        vscode.window.showWarningMessage(
          `Basilisk: internal error checking ${path.basename(filePath)}: ${stderr}`
        );
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

function parseDiagnostics(json: string, doc: vscode.TextDocument): vscode.Diagnostic[] {
  let items: BasiliskDiagnostic[];
  try {
    items = JSON.parse(json) as BasiliskDiagnostic[];
  } catch {
    return [];
  }
  if (!Array.isArray(items)) {return [];}

  return items
    .filter((item) => item.path === doc.uri.fsPath)
    .map((item) => {
      const range = new vscode.Range(
        new vscode.Position(item.line - 1, item.col - 1),
        new vscode.Position(item.end_line - 1, item.end_col - 1)
      );
      const severity = item.severity === "error"
        ? vscode.DiagnosticSeverity.Error
        : vscode.DiagnosticSeverity.Warning;
      const diag = new vscode.Diagnostic(range, `${item.message} [${item.code}]`, severity);
      diag.source = "basilisk";
      diag.code = {
        value: item.code,
        target: vscode.Uri.parse(`https://www.basilisk-python.dev/errors/${item.code}`),
      };
      return diag;
    });
}
