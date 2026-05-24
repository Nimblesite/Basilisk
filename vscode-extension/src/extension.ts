// Implements [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * Basilisk VS Code Extension
 *
 * Supports both subprocess mode (basilisk check --output json) and
 * LSP mode (basilisk lsp) based on configuration.
 */

import * as vscode from "vscode";
import { execFile } from "child_process";
import * as path from "path";
import * as os from "os";
import { Logger, bindLogger, CompositeSink, FileLogSink, nullSink } from "./logger";
import type { LogSink } from "./logger";
import { startLspClient } from "./lsp-client";
import { createDebugAdapterFactory, BasiliskDebugAdapterTrackerFactory } from "./debug-adapter";
import { registerTestExplorer } from "./test-explorer";
import { registerModuleExplorer } from "./module-explorer";
import { registerTypeHealth } from "./type-health";
import { registerInfoPanel } from "./info-panel";
import { createStore, type Store } from "./store";
import { registerProfiler, disposeProfiler } from "./profiler";
import { registerMemoryProfiler, disposeMemoryProfiler } from "./memory-profiler";
import { reportRuntimeFailure, resolveBasiliskRuntime } from "./shipwright-runtime";

/** Priority for the Basilisk status bar item (higher = further left). */
const STATUS_BAR_PRIORITY = 100;

/** Length of an abbreviated session ID prefix for logging. */
const SESSION_ID_PREFIX_LEN = 8;

/** Exit code returned by `basilisk check` on internal errors. */
const BASILISK_INTERNAL_ERROR_EXIT_CODE = 3;

let store: Store | undefined;

/**
 * Saved extension context — retained across deactivate/activate cycles so
 * that re-activation can re-initialize the extension without a fresh
 * context from VS Code.
 */
let savedContext: vscode.ExtensionContext | undefined;

/**
 * Set to true by deactivate(). While true, getStore() returns undefined.
 * Cleared by the next call to activate() or by the lazy re-init path
 * in getStore().
 */
let pendingReactivation = false;


/**
 * Disposables for one-time-only registrations (debug adapter factories,
 * lifecycle event listeners) that must be disposed before re-init.
 * Unlike context.subscriptions, we control disposal timing.
 */
let singletonDisposables: vscode.Disposable[] = [];

/** Adapts a VS Code LogOutputChannel to our LogSink interface. */
class VscodeLogSink implements LogSink {
  constructor(private readonly channel: vscode.LogOutputChannel) {}
  public trace(message: string): void { this.channel.trace(message); }
  public debug(message: string): void { this.channel.debug(message); }
  public info(message: string): void { this.channel.info(message); }
  public warn(message: string): void { this.channel.warn(message); }
  public error(message: string): void { this.channel.error(message); }
}

/**
 * Returns the store — available after activate().
 *
 * Handles two recovery paths for cross-session testing:
 *
 * 1. After deactivate(): first call returns undefined (proves cleanup).
 *    The NEXT call lazily re-initializes using the saved context.
 *
 * 2. After store.reset(): the store exists but is gutted (idle, no
 *    client). We null it and re-init so the caller gets a working store.
 *
 * Both paths are needed because VS Code's ext.activate() is a no-op
 * for an already-active extension — our activate() won't be re-called.
 */
export function getStore(): Store | undefined {
  // After deactivate(): first call returns undefined to prove cleanup.
  if (pendingReactivation) {
    pendingReactivation = false;
    return undefined;
  }

  // Lazy re-init after deactivate() or store.reset().
  // store.reset() sets store = undefined via its onReset callback.
  if (store === undefined && savedContext !== undefined) {
    initExtension(savedContext);
  }

  return store;
}

export function activate(context: vscode.ExtensionContext): void {
  savedContext = context;
  pendingReactivation = false;
  initExtension(context);
}

/**
 * Core initialization — extracted so both the initial activate() and
 * post-deactivate re-activation can share the same code path.
 */
/** Whether this is the first call to initExtension (full setup). */
let firstInit = true;

function initExtension(context: vscode.ExtensionContext): void {
  store = createStore(() => {
    // When store.reset() is called (e.g. test teardown), restart the
    // LSP client so commands get re-registered when the server reaches
    // Running state. This keeps the same store object alive.
    if (store !== undefined && savedContext !== undefined) {
      void startRuntime(savedContext, store);
    }
  });

  if (firstInit) {
    initLogging(context, store);
    initStatusBar(context, store);
  }

  const useLsp = vscode.workspace.getConfiguration("basilisk").get<boolean>("useLsp") ?? true;

  if (firstInit) {
    registerPanelsAndCommands(context, store);
  }

  if (useLsp) {
    if (firstInit) {
      // Debug adapter factories and test controller can only be registered
      // once. On re-init they are disposed+re-created via singletonDisposables.
      registerDebugSupport(context, store);
      const testController = registerTestExplorer(context, store);
      singletonDisposables.push(testController);
    }
  } else {
    updateStatusBar("starting");
  }

  void startRuntime(context, store);

  if (firstInit) {
    context.subscriptions.push(
      vscode.languages.onDidChangeDiagnostics(() => { updateStatusBarDiagnostics(); })
    );
    context.subscriptions.push(
      vscode.window.onDidChangeActiveTextEditor(() => { updateStatusBarDiagnostics(); })
    );
    firstInit = false;
  }
}

/**
 * Register activity panels, profiler UI, memory profiler, and walkthrough.
 * Called once on the first activation only.
 */
function registerPanelsAndCommands(context: vscode.ExtensionContext, s: Store): void {
  // Set context key so panel visibility conditions work.
  const hasWorkspace = (vscode.workspace.workspaceFolders?.length ?? 0) > 0;
  void vscode.commands.executeCommand("setContext", "basilisk.hasWorkspace", hasWorkspace);

  // Activity bar panels — register once (tree view IDs must be unique).
  const moduleResult = registerModuleExplorer(context, s);
  singletonDisposables.push(...moduleResult.disposables);

  const typeHealthResult = registerTypeHealth(context, s);
  singletonDisposables.push(...typeHealthResult.disposables);

  const infoPanelResult = registerInfoPanel(context, s);
  singletonDisposables.push(...infoPanelResult.disposables);

  // Profiler UI — status bar, commands, decorations, flamegraph webview.
  const profilerDisposables = registerProfiler(context, s);
  singletonDisposables.push(...profilerDisposables);

  // Memory profiler UI — commands, reference graph webview, memory dashboard.
  const memoryDisposables = registerMemoryProfiler(context, s);
  singletonDisposables.push(...memoryDisposables);

  // Walkthrough command.
  singletonDisposables.push(
    vscode.commands.registerCommand("basilisk.openWalkthrough", () => {
      void vscode.commands.executeCommand(
        "workbench.action.openWalkthrough",
        "Nimblesite.basilisk#basilisk.gettingStarted",
      );
    }),
  );
}

export function deactivate(): Promise<void> | undefined {
  disposeProfiler();
  disposeMemoryProfiler();
  const result = store?.client.value?.stop();
  // Set store = undefined BEFORE calling reset() so the onReset callback
  // (which checks `store !== undefined`) does NOT restart the LSP client.
  // Without this, reset() → onReset → startLspClient re-registers commands
  // on the dying store, and the new activate() gets "command already exists".
  const dyingStore = store;
  store = undefined;
  dyingStore?.reset();
  pendingReactivation = true;

  // Dispose singleton registrations (debug adapter factories, etc.)
  // so they can be re-registered on the next activation cycle.
  for (const d of singletonDisposables) {
    d.dispose();
  }
  singletonDisposables = [];

  // Allow full re-initialization on next activate() — without this,
  // initExtension() skips debug adapters, test explorer, activity
  // panels, logging, status bar, and event listeners.
  firstInit = true;

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

function registerDebugSupport(context: vscode.ExtensionContext, s: Store): void {
  // Debug adapter factories can only be registered once per type.
  // Push to singletonDisposables so deactivate() can dispose them
  // before re-init (context.subscriptions disposal is not in our control).
  singletonDisposables.push(
    vscode.debug.registerDebugAdapterDescriptorFactory(
      "basilisk-debug",
      createDebugAdapterFactory(() => s.client.value)
    )
  );
  singletonDisposables.push(
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
  // Set context key for panel visibility conditions.
  void vscode.commands.executeCommand("setContext", "basilisk.serverState", state === "ready" ? "running" : state);

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

// ── Runtime resolution ────────────────────────────────────────────────────

async function startRuntime(context: vscode.ExtensionContext, s: Store): Promise<void> {
  try {
    const runtime = await resolveBasiliskRuntime(context);
    s.setRuntimeResolution({
      componentId: runtime.componentId,
      path: runtime.executablePath,
      source: runtime.source,
      version: runtime.version,
    });
    Logger.info(
      `Basilisk executable: ${runtime.executablePath} ` +
      `(source=${runtime.source}, version=${runtime.version ?? "unknown"})`
    );
    if (vscode.workspace.getConfiguration("basilisk").get<boolean>("useLsp") ?? true) {
      startLspClient(
        { context, executablePath: runtime.executablePath, outputChannel: s.outputChannel.value },
        s,
        updateStatusBar
      );
    } else {
      startSubprocessMode(context, runtime.executablePath);
      updateStatusBar("ready");
    }
  } catch (error: unknown) {
    reportRuntimeFailure(error);
    updateStatusBar("error");
  }
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
