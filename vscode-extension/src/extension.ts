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
import { Logger, bindLogger, CompositeSink, FileLogSink, nullSink } from "./logger";
import type { LogSink } from "./logger";
import { startLspClient } from "./lsp-client";
import { createDebugAdapterFactory, BasiliskDebugAdapterTrackerFactory } from "./debug-adapter";
import { createStore, type Store } from "./store";

/** Priority for the Basilisk status bar item (higher = further left). */
const STATUS_BAR_PRIORITY = 100;

/** Length of an abbreviated session ID prefix for logging. */
const SESSION_ID_PREFIX_LEN = 8;

/** Exit code returned by `basilisk check` on internal errors. */
const BASILISK_INTERNAL_ERROR_EXIT_CODE = 3;

let store: Store | undefined;

/** Adapts a VS Code LogOutputChannel to our LogSink interface. */
class VscodeLogSink implements LogSink {
  constructor(private readonly channel: vscode.LogOutputChannel) {}
  public trace(message: string): void { this.channel.trace(message); }
  public debug(message: string): void { this.channel.debug(message); }
  public info(message: string): void { this.channel.info(message); }
  public warn(message: string): void { this.channel.warn(message); }
  public error(message: string): void { this.channel.error(message); }
}

/** Returns the store — available after activate(). Tests use this to query internal state. */
export function getStore(): Store | undefined {
  return store;
}

export function activate(context: vscode.ExtensionContext): void {
  store = createStore();
  initLogging(context, store);
  initStatusBar(context, store);

  const executablePath = resolveExecutablePath(resolveConfiguredPath());
  const useLsp = vscode.workspace.getConfiguration("basilisk").get<boolean>("useLsp") ?? true;
  Logger.info(`Basilisk executable: ${executablePath}`);


  if (useLsp) {
    startLspClient({ context, executablePath, outputChannel: store.outputChannel.value }, store, updateStatusBar);
    registerDebugSupport(context, store);
  } else {
    startSubprocessMode(context, executablePath);
    updateStatusBar("ready");
  }

  context.subscriptions.push(
    vscode.languages.onDidChangeDiagnostics(() => { updateStatusBarDiagnostics(); })
  );
  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(() => { updateStatusBarDiagnostics(); })
  );
}

export function deactivate(): Promise<void> | undefined {
  const result = store?.client.value?.stop();
  store?.reset();
  store = undefined;
  return result;
}

// ── Initialization helpers ────────────────────────────────────────────────

function initLogging(context: vscode.ExtensionContext, s: Store): void {
  const logChannel = vscode.window.createOutputChannel("Basilisk", { log: true });
  s.setOutputChannel(logChannel);
  const logFilePath = path.join(os.tmpdir(), "basilisk-debug-trace.log");
  const fileSink = new FileLogSink(logFilePath);
  const compositeSink = new CompositeSink([new VscodeLogSink(logChannel), fileSink]);
  s.setLogSink(compositeSink);
  bindLogger(() => s.logSink.value ?? nullSink);
  logChannel.info(`Log file: ${logFilePath}`);
  context.subscriptions.push(logChannel);
}

function initStatusBar(context: vscode.ExtensionContext, s: Store): void {
  const item = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    STATUS_BAR_PRIORITY
  );
  item.command = "basilisk.showOutput";
  s.setStatusBarItem(item);
  context.subscriptions.push(item);
}

function resolveConfiguredPath(): string {
  const cfg = vscode.workspace.getConfiguration("basilisk");
  return process.env.BASILISK_EXECUTABLE_PATH ??
    cfg.get<string>("executablePath") ??
    "basilisk";
}

function registerDebugSupport(context: vscode.ExtensionContext, s: Store): void {
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory(
      "basilisk-debug",
      createDebugAdapterFactory(() => s.client.value)
    )
  );
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterTrackerFactory(
      "basilisk-debug",
      new BasiliskDebugAdapterTrackerFactory()
    )
  );
  registerDebugLifecycleLogging(context);
}

function registerDebugLifecycleLogging(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.debug.onDidStartDebugSession((session) => {
      Logger.info(`Debug session started: id=${session.id}, name=${session.name}, type=${session.type}`);
    })
  );
  context.subscriptions.push(
    vscode.debug.onDidTerminateDebugSession((session) => {
      const activeId = vscode.debug.activeDebugSession?.id ?? "undefined";
      Logger.info(
        `[Lifecycle] onDidTerminateDebugSession: terminated=${session.id.slice(0, SESSION_ID_PREFIX_LEN)}, ` +
        `active=${activeId === "undefined" ? "correctly undefined" : `STILL SET (${activeId.slice(0, SESSION_ID_PREFIX_LEN)})`}`
      );
    })
  );
  context.subscriptions.push(
    vscode.debug.onDidChangeActiveDebugSession((session) => {
      Logger.info(
        `[Lifecycle] onDidChangeActiveDebugSession: ${session ? `id=${session.id.slice(0, SESSION_ID_PREFIX_LEN)}, name="${session.name}"` : "→ NONE"}`
      );
    })
  );
}

// ── Status bar ────────────────────────────────────────────────────────────

function updateStatusBar(state: "starting" | "ready" | "error" | "stopped"): void {
  const item = store?.statusBarItem.value;
  if (!item) {return;}
  switch (state) {
    case "starting":
      item.text = "$(sync~spin) Basilisk";
      item.tooltip = "Basilisk language server starting...";
      item.backgroundColor = undefined;
      break;
    case "ready":
      item.text = "$(check) Basilisk";
      item.tooltip = "Basilisk language server running";
      item.backgroundColor = undefined;
      break;
    case "error":
      item.text = "$(error) Basilisk";
      item.tooltip = "Basilisk language server error";
      item.backgroundColor = new vscode.ThemeColor("statusBarItem.errorBackground");
      break;
    case "stopped":
      item.text = "$(circle-slash) Basilisk";
      item.tooltip = "Basilisk language server stopped";
      item.backgroundColor = undefined;
      break;
  }
  item.show();
}

function updateStatusBarDiagnostics(): void {
  const item = store?.statusBarItem.value;
  if (!item) {return;}
  const editor = vscode.window.activeTextEditor;
  if (editor?.document.languageId !== "python") {return;}
  const diagnostics = vscode.languages.getDiagnostics(editor.document.uri);
  const basiliskDiags = diagnostics.filter((d) => d.source === "basilisk");
  const errorCount = basiliskDiags.filter((d) => d.severity === vscode.DiagnosticSeverity.Error).length;
  const warnCount = basiliskDiags.filter((d) => d.severity === vscode.DiagnosticSeverity.Warning).length;

  if (errorCount > 0) {
    item.text = `$(error) Basilisk (${errorCount})`;
    item.tooltip = `Basilisk: ${errorCount} error(s), ${warnCount} warning(s)`;
    item.backgroundColor = new vscode.ThemeColor("statusBarItem.errorBackground");
  } else if (warnCount > 0) {
    item.text = `$(warning) Basilisk (${warnCount})`;
    item.tooltip = `Basilisk: ${warnCount} warning(s)`;
    item.backgroundColor = new vscode.ThemeColor("statusBarItem.warningBackground");
  } else {
    item.text = "$(check) Basilisk";
    item.tooltip = "Basilisk: No issues";
    item.backgroundColor = undefined;
  }
}

// ── Executable resolution ─────────────────────────────────────────────────

function resolveExecutablePath(configured: string): string {
  if (path.isAbsolute(configured)) {
    return configured;
  }

  if (configured.includes(path.sep) || configured.includes("/")) {
    const wsRoot = workspaceRoot();
    return wsRoot !== undefined && wsRoot !== "" ? path.resolve(wsRoot, configured) : configured;
  }

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

  return configured;
}

function workspaceRoot(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

// ── Subprocess mode ───────────────────────────────────────────────────────

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

function startSubprocessMode(
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
