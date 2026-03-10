/**
 * Basilisk VS Code Extension
 *
 * Supports both subprocess mode (basilisk check --output json) and
 * LSP mode (basilisk lsp) based on configuration.
 */

import * as vscode from "vscode";
import { execFile } from "child_process";
import * as path from "path";
import * as fs from "fs";
import * as os from "os";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  CloseAction,
  ErrorAction,
  State,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;
let statusBarItem: vscode.StatusBarItem | undefined;
let outputChannel: vscode.OutputChannel | undefined;

/** Registered command IDs so we can avoid double-registering on re-activation. */
const registeredCommands = new Set<string>();

/**
 * Safely register a VS Code command, avoiding "command already exists" errors
 * that occur when the extension re-activates after an LSP crash/restart cycle.
 * Disposes the previous registration if one exists.
 */
function safeRegisterCommand(
  context: vscode.ExtensionContext,
  commandId: string,
  handler: (...args: unknown[]) => unknown
): void {
  // Dispose any previous subscription for this command.
  if (registeredCommands.has(commandId)) {
    return;
  }
  const disposable = vscode.commands.registerCommand(commandId, handler);
  context.subscriptions.push(disposable);
  registeredCommands.add(commandId);
}

/**
 * Resolve the basilisk executable path. On macOS, VS Code often does not
 * inherit the user's shell PATH, so a bare "basilisk" won't be found even
 * when ~/.cargo/bin/basilisk exists. This function checks common locations.
 */
function resolveExecutablePath(configured: string): string {
  // If the user provided an absolute path, use it directly.
  if (path.isAbsolute(configured)) {
    return configured;
  }

  // If it's a relative path with separators (e.g. "./basilisk"), resolve against workspace.
  if (configured.includes(path.sep) || configured.includes("/")) {
    const wsRoot = workspaceRoot();
    return wsRoot ? path.resolve(wsRoot, configured) : configured;
  }

  // Bare command name (e.g. "basilisk") — check well-known locations that
  // VS Code on macOS may not have in its PATH.
  const candidates = [
    path.join(os.homedir(), ".cargo", "bin", configured),
    `/usr/local/bin/${configured}`,
    `/opt/homebrew/bin/${configured}`,
  ];

  for (const candidate of candidates) {
    try {
      fs.accessSync(candidate, fs.constants.X_OK);
      return candidate;
    } catch {
      // Not found or not executable — try next.
    }
  }

  // Fall back to the bare name and let the OS resolve it via PATH.
  return configured;
}

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
  outputChannel = vscode.window.createOutputChannel("Basilisk");
  context.subscriptions.push(outputChannel);

  // Status bar item — shows server state and diagnostic count.
  statusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    100
  );
  statusBarItem.command = "basilisk.showOutput";
  context.subscriptions.push(statusBarItem);

  const cfg = vscode.workspace.getConfiguration("basilisk");
  const configuredPath = cfg.get<string>("executablePath") ?? "basilisk";
  const executablePath = resolveExecutablePath(configuredPath);
  const useLsp = cfg.get<boolean>("useLsp") ?? true;

  outputChannel.appendLine(`Basilisk executable: ${executablePath}`);

  // Register commands safely — avoids "command already exists" errors when
  // the extension re-activates after an LSP crash/restart cycle.
  safeRegisterCommand(context, "basilisk.restartServer", async () => {
    if (!client) {
      vscode.window.showWarningMessage("Basilisk: No language server to restart.");
      return;
    }
    try {
      outputChannel?.appendLine("Restarting Basilisk language server...");
      await client.stop();
      await client.start();
      outputChannel?.appendLine("Basilisk language server restarted.");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      outputChannel?.appendLine(`Restart failed: ${msg}`);
      vscode.window.showErrorMessage(`Basilisk: Failed to restart server: ${msg}`);
    }
  });

  safeRegisterCommand(context, "basilisk.showOutput", () => {
    outputChannel?.show();
  });

  if (useLsp) {
    // In LSP mode, the LanguageClient automatically registers
    // basilisk.organizeImports via the server's executeCommandProvider.
    // Do NOT register it manually — that would conflict.
    startLspClient(context, executablePath);
  } else {
    // In subprocess mode, register organizeImports client-side.
    safeRegisterCommand(context, "basilisk.organizeImports", () => {
      organizeImports();
    });
    startSubprocessMode(context, executablePath);
    updateStatusBar("ready");
  }

  // Update status bar when diagnostics change.
  context.subscriptions.push(
    vscode.languages.onDidChangeDiagnostics(() => updateStatusBarDiagnostics())
  );

  // Update status bar when active editor changes.
  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(() => updateStatusBarDiagnostics())
  );
}

export function deactivate(): Promise<void> | undefined {
  return client?.stop();
}

function updateStatusBar(state: "starting" | "ready" | "error" | "stopped"): void {
  if (!statusBarItem) return;
  switch (state) {
    case "starting":
      statusBarItem.text = "$(sync~spin) Basilisk";
      statusBarItem.tooltip = "Basilisk language server starting...";
      statusBarItem.backgroundColor = undefined;
      break;
    case "ready":
      statusBarItem.text = "$(check) Basilisk";
      statusBarItem.tooltip = "Basilisk language server running";
      statusBarItem.backgroundColor = undefined;
      break;
    case "error":
      statusBarItem.text = "$(error) Basilisk";
      statusBarItem.tooltip = "Basilisk language server error";
      statusBarItem.backgroundColor = new vscode.ThemeColor(
        "statusBarItem.errorBackground"
      );
      break;
    case "stopped":
      statusBarItem.text = "$(circle-slash) Basilisk";
      statusBarItem.tooltip = "Basilisk language server stopped";
      statusBarItem.backgroundColor = undefined;
      break;
  }
  statusBarItem.show();
}

function updateStatusBarDiagnostics(): void {
  if (!statusBarItem) return;
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "python") {
    return;
  }
  const diagnostics = vscode.languages.getDiagnostics(editor.document.uri);
  const basiliskDiags = diagnostics.filter((d) => d.source === "basilisk");
  const errorCount = basiliskDiags.filter(
    (d) => d.severity === vscode.DiagnosticSeverity.Error
  ).length;
  const warnCount = basiliskDiags.filter(
    (d) => d.severity === vscode.DiagnosticSeverity.Warning
  ).length;

  if (errorCount > 0) {
    statusBarItem.text = `$(error) Basilisk (${errorCount})`;
    statusBarItem.tooltip = `Basilisk: ${errorCount} error(s), ${warnCount} warning(s)`;
    statusBarItem.backgroundColor = new vscode.ThemeColor(
      "statusBarItem.errorBackground"
    );
  } else if (warnCount > 0) {
    statusBarItem.text = `$(warning) Basilisk (${warnCount})`;
    statusBarItem.tooltip = `Basilisk: ${warnCount} warning(s)`;
    statusBarItem.backgroundColor = new vscode.ThemeColor(
      "statusBarItem.warningBackground"
    );
  } else {
    statusBarItem.text = "$(check) Basilisk";
    statusBarItem.tooltip = "Basilisk: No issues";
    statusBarItem.backgroundColor = undefined;
  }
}

function startLspClient(
  context: vscode.ExtensionContext,
  executablePath: string
): void {
  const serverOptions: ServerOptions = {
    command: executablePath,
    args: ["lsp"],
  };

  const traceChannel = vscode.window.createOutputChannel("Basilisk LSP Trace");
  context.subscriptions.push(traceChannel);

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "python" }],
    synchronize: {
      configurationSection: "basilisk",
    },
    traceOutputChannel: traceChannel,
    outputChannel: outputChannel,
    errorHandler: {
      error: (error, _message, count) => {
        outputChannel?.appendLine(`LSP error: ${error.message ?? error}`);
        if (count !== undefined && count < 3) {
          return { action: ErrorAction.Continue };
        }
        updateStatusBar("error");
        return { action: ErrorAction.Shutdown };
      },
      closed: () => {
        outputChannel?.appendLine("LSP connection closed. Restarting...");
        return { action: CloseAction.Restart };
      },
    },
    middleware: {
      workspace: {
        // Intercept configuration requests from the server and inject
        // Basilisk-specific settings so the server can read them.
        configuration: async (params, token, next) => {
          const results = await next(params, token);
          if (!Array.isArray(results)) {
            return results;
          }
          const cfg = vscode.workspace.getConfiguration("basilisk");
          return results.map((item, idx) => {
            const section = params.items[idx]?.section;
            if (section === "basilisk" || section?.startsWith("basilisk.")) {
              return {
                ...(typeof item === "object" && item !== null ? item : {}),
                inlayHints: {
                  parameterNames: cfg.get<boolean>("inlayHints.parameterNames") ?? true,
                  variableTypes: cfg.get<boolean>("inlayHints.variableTypes") ?? true,
                },
                ruff: {
                  enabled: cfg.get<boolean>("ruff.enabled") ?? true,
                  executablePath: cfg.get<string>("ruff.executablePath") ?? "ruff",
                },
              };
            }
            return item;
          });
        },
      },
    },
  };

  client = new LanguageClient(
    "basilisk",
    "Basilisk Type Checker",
    serverOptions,
    clientOptions
  );

  updateStatusBar("starting");

  client.onDidChangeState((event) => {
    switch (event.newState) {
      case State.Running:
        outputChannel?.appendLine("Basilisk language server is running.");
        updateStatusBar("ready");
        break;
      case State.Stopped:
        outputChannel?.appendLine("Basilisk language server stopped.");
        updateStatusBar("stopped");
        break;
    }
  });

  // Notify server of configuration changes so it can update inlay hint behaviour.
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("basilisk") && client?.isRunning()) {
        client.sendNotification("workspace/didChangeConfiguration", {
          settings: buildServerSettings(),
        });
      }
    })
  );

  client.start().catch((error: Error) => {
    const msg =
      `Basilisk: Failed to start language server. ` +
      `Is '${executablePath}' installed and on PATH? ${error.message}`;
    vscode.window.showErrorMessage(msg);
    outputChannel?.appendLine(msg);
    updateStatusBar("error");
  });

  context.subscriptions.push(client);
}

/** Build the settings object forwarded to the LSP server. */
function buildServerSettings(): Record<string, unknown> {
  const cfg = vscode.workspace.getConfiguration("basilisk");
  return {
    basilisk: {
      inlayHints: {
        parameterNames: cfg.get<boolean>("inlayHints.parameterNames") ?? true,
        variableTypes: cfg.get<boolean>("inlayHints.variableTypes") ?? true,
      },
      ruff: {
        enabled: cfg.get<boolean>("ruff.enabled") ?? true,
        executablePath: cfg.get<string>("ruff.executablePath") ?? "ruff",
      },
    },
  };
}

/** Run `ruff check --select I --fix` on the active file to organize imports. */
function organizeImports(): void {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "python") {
    vscode.window.showWarningMessage("Basilisk: Open a Python file to organize imports.");
    return;
  }

  const cfg = vscode.workspace.getConfiguration("basilisk");
  const ruffEnabled = cfg.get<boolean>("ruff.enabled") ?? true;
  if (!ruffEnabled) {
    vscode.window.showWarningMessage("Basilisk: Ruff integration is disabled. Enable basilisk.ruff.enabled to organize imports.");
    return;
  }

  const ruffPath = cfg.get<string>("ruff.executablePath") ?? "ruff";
  const filePath = editor.document.uri.fsPath;

  execFile(
    ruffPath,
    ["check", "--select", "I", "--fix", filePath],
    { cwd: workspaceRoot() },
    (error, _stdout, stderr) => {
      if (error && typeof error.code === "number" && error.code > 1) {
        vscode.window.showWarningMessage(
          `Basilisk: Failed to run ruff for import organization. ` +
            `Is '${ruffPath}' installed and on PATH? (${error.message})`
        );
        outputChannel?.appendLine(`organizeImports error: ${stderr}`);
        return;
      }
      outputChannel?.appendLine(`Imports organized in ${path.basename(filePath)}`);
    }
  );
}

function startSubprocessMode(
  context: vscode.ExtensionContext,
  executablePath: string
): void {
  const collection =
    vscode.languages.createDiagnosticCollection(COLLECTION_NAME);
  context.subscriptions.push(collection);

  // Check on open.
  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => {
      if (doc.languageId === "python") {
        checkDocument(doc, collection, executablePath);
      }
    })
  );

  // Check on save.
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "python") {
        checkDocument(doc, collection, executablePath);
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
      checkDocument(doc, collection, executablePath);
    }
  }
}

function checkDocument(
  doc: vscode.TextDocument,
  collection: vscode.DiagnosticCollection,
  executablePath: string
): void {
  const cfg = vscode.workspace.getConfiguration("basilisk");
  const enabled = cfg.get<boolean>("enabled") ?? true;

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
          `https://www.basilisk-python.dev/errors/${item.code}`
        ),
      };

      return diag;
    });
}

function workspaceRoot(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}
