/**
 * Basilisk VS Code Extension
 *
 * Runs `basilisk check --output json` on every Python file save/open and
 * pushes the resulting diagnostics into VSCode's Problems panel.
 *
 * NOTE: This extension uses the subprocess approach (no LSP).
 * LSP integration is deferred to a future phase — see docs/lsp-plan.md.
 */

import * as vscode from "vscode";
import { execFile } from "child_process";
import * as path from "path";

/** Shape of a single diagnostic emitted by `basilisk check --output json`. */
interface BasiliskDiagnostic {
  code: string;
  severity: "error" | "warning";
  message: string;
  path: string;
  /** 1-based line number. */
  line: number;
  /** 1-based column number. */
  col: number;
  /** 1-based end line number. */
  end_line: number;
  /** 1-based end column number (exclusive). */
  end_col: number;
}

const COLLECTION_NAME = "basilisk";

export function activate(context: vscode.ExtensionContext): void {
  const collection =
    vscode.languages.createDiagnosticCollection(COLLECTION_NAME);
  context.subscriptions.push(collection);

  // Check on open.
  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => {
      if (doc.languageId === "python") {
        checkDocument(doc, collection);
      }
    })
  );

  // Check on save.
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "python") {
        checkDocument(doc, collection);
      }
    })
  );

  // Clear diagnostics when a file is closed.
  context.subscriptions.push(
    vscode.workspace.onDidCloseTextDocument((doc) => {
      collection.delete(doc.uri);
    })
  );

  // Check all already-open Python documents on activation.
  for (const doc of vscode.workspace.textDocuments) {
    if (doc.languageId === "python") {
      checkDocument(doc, collection);
    }
  }
}

export function deactivate(): void {
  // Nothing to tear down; the DiagnosticCollection is disposed via subscriptions.
}

function getConfig(): { executablePath: string; enabled: boolean } {
  const cfg = vscode.workspace.getConfiguration("basilisk");
  return {
    executablePath: cfg.get<string>("executablePath") ?? "basilisk",
    enabled: cfg.get<boolean>("enabled") ?? true,
  };
}

function checkDocument(
  doc: vscode.TextDocument,
  collection: vscode.DiagnosticCollection
): void {
  const { executablePath, enabled } = getConfig();

  if (!enabled) {
    collection.delete(doc.uri);
    return;
  }

  // Only check files on disk — unsaved buffers have no path the binary can read.
  if (doc.isUntitled || doc.uri.scheme !== "file") {
    return;
  }

  const filePath = doc.uri.fsPath;

  execFile(
    executablePath,
    ["check", "--output", "json", filePath],
    { cwd: workspaceRoot() },
    (error, stdout, stderr) => {
      // Exit code 1 means diagnostics found — that's normal, not a crash.
      // Exit code 3 means internal error.
      if (error && error.code === 3) {
        vscode.window.showWarningMessage(
          `Basilisk: internal error checking ${path.basename(filePath)}: ${stderr}`
        );
        return;
      }

      // Any other non-zero exit (e.g. binary not found) should also surface.
      if (error && typeof error.code === "number" && error.code !== 1) {
        vscode.window.showWarningMessage(
          `Basilisk: failed to run '${executablePath}'. ` +
            `Is it installed and on PATH? (${error.message})`
        );
        collection.delete(doc.uri);
        return;
      }

      const diagnostics = parseDiagnostics(stdout, doc);
      collection.set(doc.uri, diagnostics);
    }
  );
}

function parseDiagnostics(
  json: string,
  doc: vscode.TextDocument
): vscode.Diagnostic[] {
  let items: BasiliskDiagnostic[];

  try {
    items = JSON.parse(json) as BasiliskDiagnostic[];
  } catch {
    // Malformed JSON — swallow silently (binary may print warnings before JSON).
    return [];
  }

  if (!Array.isArray(items)) {
    return [];
  }

  return items
    .filter((item) => item.path === doc.uri.fsPath)
    .map((item) => {
      // Convert 1-based Basilisk coordinates to 0-based VSCode positions.
      const start = new vscode.Position(item.line - 1, item.col - 1);
      const end = new vscode.Position(item.end_line - 1, item.end_col - 1);
      const range = new vscode.Range(start, end);

      const severity =
        item.severity === "error"
          ? vscode.DiagnosticSeverity.Error
          : vscode.DiagnosticSeverity.Warning;

      const diag = new vscode.Diagnostic(
        range,
        `${item.message} [${item.code}]`,
        severity
      );
      diag.source = "basilisk";
      diag.code = {
        value: item.code,
        target: vscode.Uri.parse(
          `https://basilisk-lang.org/errors/${item.code}`
        ),
      };

      return diag;
    });
}

function workspaceRoot(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}
