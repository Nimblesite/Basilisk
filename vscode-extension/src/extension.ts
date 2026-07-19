// Implements [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * Basilisk VS Code Extension
 *
 * Supports both subprocess mode (basilisk check --output json) and
 * LSP mode (basilisk lsp) based on configuration.
 */

import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";
import { Logger, bindLogger, CompositeSink, FileLogSink, nullSink } from "./logger";
import type { LogSink } from "./logger";
import { startLspClient } from "./lsp-client";
import { createDebugAdapterFactory, BasiliskDebugAdapterTrackerFactory, createBasiliskDebugConfigProvider } from "./debug-adapter";
import { startSubprocessMode } from "./subprocess-mode";
import { registerTestExplorer } from "./test-explorer";
import { registerModuleExplorer } from "./module-explorer";
import { registerInfoPanel } from "./info-panel";
import { registerPythonProcesses } from "./process-explorer";
import { registerConfigurationEditor } from "./configuration-editor-registration";
import { createStore, type Store } from "./store";
import { registerProfiler, disposeProfiler } from "./profiler";
import { registerMemoryProfiler, disposeMemoryProfiler } from "./memory-profiler";
import { registerMemoryAutopilot, disposeMemoryAutopilot, notifyDebuggeePause } from "./memory-autopilot";
import { reportRuntimeFailure, resolveBasiliskRuntime } from "./shipwright-runtime";

/** Priority for the Basilisk status bar item (higher = further left). */
const STATUS_BAR_PRIORITY = 100;

/** Length of an abbreviated session ID prefix for logging. */
const SESSION_ID_PREFIX_LEN = 8;

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

// Implements [EXTACT] — wires up the Basilisk activity sidebar (Modules + Basilisk
// info panels) plus the profiling/memory UI and the Getting Started walkthrough.
/**
 * Register activity panels, profiler UI, memory profiler, and walkthrough.
 * Called once on the first activation only.
 */
function registerPanelsAndCommands(context: vscode.ExtensionContext, s: Store): void {
  // Set context key so panel visibility conditions work.
  const hasWorkspace = (vscode.workspace.workspaceFolders?.length ?? 0) > 0;
  void vscode.commands.executeCommand("setContext", "basilisk.hasWorkspace", hasWorkspace);

  // Activity bar panels — register once (tree view IDs must be unique).
  // The Modules panel (module-explorer) now carries the folded type-health
  // rollup, so there is no separate Type Health panel [EXTACT-MODULES].
  const moduleResult = registerModuleExplorer(context, s);
  singletonDisposables.push(...moduleResult.disposables);

  const infoPanelResult = registerInfoPanel(context, s);
  singletonDisposables.push(...infoPanelResult.disposables);

  // Editor-area configuration shell. Capability gating and all mutations are
  // delegated to the LSP; this registration owns only VS Code lifecycle/UI.
  const configurationEditor = registerConfigurationEditor(s);
  singletonDisposables.push(...configurationEditor.disposables);

  // Python Processes panel — LSP-driven process picker for one-click profiling (#62).
  const processesResult = registerPythonProcesses(context, s);
  singletonDisposables.push(...processesResult.disposables);

  // Profiler UI — status bar, commands, decorations, flamegraph webview.
  const profilerDisposables = registerProfiler(s);
  singletonDisposables.push(...profilerDisposables);

  // Memory profiler UI — commands, reference graph webview, memory dashboard.
  const memoryDisposables = registerMemoryProfiler(s);
  singletonDisposables.push(...memoryDisposables);

  // Memory autopilot — auto snapshot+diff on every pause / interval, so the leak
  // hunt is "set a breakpoint and press Continue" ([PROFILE-MEMORY-AUTOPILOT]).
  singletonDisposables.push(...registerMemoryAutopilot(s));

  // Implements [EXTACT-INFO-GETTING-STARTED] — the Getting Started items open the
  // built-in `basilisk.gettingStarted` walkthrough (contributes.walkthroughs in
  // package.json) directly via this command.
  // Walkthrough command.
  singletonDisposables.push(
    vscode.commands.registerCommand("basilisk.openWalkthrough", () => {
      void vscode.commands.executeCommand(
        "workbench.action.openWalkthrough",
        "Nimblesite.basilisk#basilisk.gettingStarted",
      );
    }),
  );

  // Implements [VSIX-STATUS-BAR] — clicking the always-visible status bar item
  // opens a quick-pick so configuration is reachable from anywhere in the UI,
  // not only the settings cog buried in the BASILISK info panel title bar.
  singletonDisposables.push(
    vscode.commands.registerCommand("basilisk.statusMenu", async () => handleStatusMenu()),
  );
}

// Implements [VSIX-STATUS-BAR] — quick-pick shown when the status bar item is
// clicked. Configuration is listed first (the primary reason a user reaches for
// it); Show Output and Restart Server remain one keystroke away.
async function handleStatusMenu(): Promise<void> {
  const items: readonly { label: string; command: string }[] = [
    { label: "$(settings-gear) Open Configuration Editor", command: "basilisk.openConfigurationEditor" },
    { label: "$(output) Show Output", command: "basilisk.showOutput" },
    { label: "$(debug-restart) Restart Language Server", command: "basilisk.restartServer" },
  ];
  const pick = await vscode.window.showQuickPick(items, { placeHolder: "Basilisk" });
  if (pick !== undefined) {
    await vscode.commands.executeCommand(pick.command);
  }
}

export function deactivate(): Promise<void> | undefined {
  disposeProfiler();
  disposeMemoryProfiler();
  disposeMemoryAutopilot();
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

// Implements [VSIX-OUTPUT-CHANNELS] — creates the main "Basilisk" output channel
// and the file log sink. DEVIATION: the spec names the file sink
// "/tmp/basilisk-debug-trace.log", but for security (js/insecure-temporary-file)
// the log lives at context.logUri/basilisk-debug-trace.log (per-extension private
// dir), not world-writable /tmp. The "Basilisk LSP Trace" channel is created in
// lsp-client.ts.
function initLogging(context: vscode.ExtensionContext, s: Store): void {
  const logChannel = vscode.window.createOutputChannel("Basilisk", { log: true });
  s.setOutputChannel(logChannel);
  // Logs live in the extension's PRIVATE per-extension log directory
  // (context.logUri) — never the world-writable OS temp dir. A predictable name
  // in shared /tmp is open to symlink redirection and cross-user disclosure
  // (js/insecure-temporary-file); the per-extension dir is owned by this user
  // and is where [VSIX-OUTPUT-CHANNELS] expects logs to live. VS Code may not
  // have created logUri on disk yet, so ensure it exists first.
  const logDir = context.logUri.fsPath;
  fs.mkdirSync(logDir, { recursive: true });
  const logFilePath = path.join(logDir, "basilisk-debug-trace.log");
  const fileSink = new FileLogSink(logFilePath);
  const compositeSink = new CompositeSink([new VscodeLogSink(logChannel), fileSink]);
  s.setLogSink(compositeSink);
  bindLogger(() => s.logSink.value ?? nullSink);
  logChannel.info(`Log file: ${logFilePath}`);
  context.subscriptions.push(logChannel);
}

// Implements [VSIX-STATUS-BAR] — creates the persistent status bar item whose
// text/state is driven by updateStatusBar (server state) and
// updateStatusBarDiagnostics (per-file error/warning counts).
function initStatusBar(context: vscode.ExtensionContext, s: Store): void {
  const item = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    STATUS_BAR_PRIORITY
  );
  item.command = "basilisk.statusMenu";
  s.setStatusBarItem(item);
  context.subscriptions.push(item);
}

// Implements [VSIX-PYTHON-DEBUGGER-DAP-ARCHITECTURE] / [VSIX-PYTHON-DEBUGGER-START]
// — registers the `basilisk-debug` adapter-descriptor factory, the (Dynamic +
// default) config provider, and the tracker factory. The matching activation
// events (onDebug, onDebugResolve/onDebugDynamicConfigurations:basilisk-debug)
// are declared in vscode-extension/package.json so these register before a Python
// file is opened. [VSIX-PYTHON-DEBUGGER-DAP-TRACKER]: tracker callbacks feed PID +
// pause signals to the store.
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
  // Let users start debugging with NO launch.json: the Dynamic provider lists a
  // "Python (Basilisk)" config in the Run-and-Debug picker, and resolve fills in
  // the current file for an empty/partial config (F5 / the big Run button).
  const debugConfigProvider = createBasiliskDebugConfigProvider();
  singletonDisposables.push(
    vscode.debug.registerDebugConfigurationProvider(
      "basilisk-debug",
      debugConfigProvider,
      vscode.DebugConfigurationProviderTriggerKind.Dynamic
    ),
    vscode.debug.registerDebugConfigurationProvider("basilisk-debug", debugConfigProvider)
  );
  singletonDisposables.push(
    vscode.debug.registerDebugAdapterTrackerFactory(
      "basilisk-debug",
      new BasiliskDebugAdapterTrackerFactory({
        // The tracker captures the debuggee's PID (from the DAP `process` event)
        // so the CPU profiler can attach to the SAME process the debugger drives.
        onDebuggeeProcessId: (sessionId, pid) => { s.setDebuggeeProcessId(sessionId, pid); },
        // …and every pause drives the memory autopilot ([PROFILE-MEMORY-AUTOPILOT-PAUSE]).
        onStopped: (sessionId) => { notifyDebuggeePause(sessionId); },
      })
    )
  );
  // Forget the debuggee PID when its session ends so stale mappings can't
  // misdirect a later profile attach.
  context.subscriptions.push(
    vscode.debug.onDidTerminateDebugSession((session) => {
      s.clearDebuggeeProcessId(session.id);
    })
  );
  registerDebugLifecycleLogging(context);
}

function registerDebugLifecycleLogging(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.debug.onDidStartDebugSession((session) => {
      Logger.info(`Debug session started: id=${session.id}, name=${session.name}, type=${session.type}`);
      // Gate debug-only commands (memory profiling needs a paused debuggee).
      if (session.type === "basilisk-debug") {
        void vscode.commands.executeCommand("setContext", "basilisk.debugging", true);
      }
    })
  );
  context.subscriptions.push(
    vscode.debug.onDidTerminateDebugSession((session) => {
      const activeId = vscode.debug.activeDebugSession?.id ?? "undefined";
      Logger.info(
        `[Lifecycle] onDidTerminateDebugSession: terminated=${session.id.slice(0, SESSION_ID_PREFIX_LEN)}, ` +
        `active=${activeId === "undefined" ? "correctly undefined" : `STILL SET (${activeId.slice(0, SESSION_ID_PREFIX_LEN)})`}`
      );
      // Clear the debug context once no Basilisk debug session remains active.
      // Symmetric with the type-gated set above: stays true only while a
      // basilisk-debug session is active (ignores other debuggers' sessions).
      if (vscode.debug.activeDebugSession?.type !== "basilisk-debug") {
        void vscode.commands.executeCommand("setContext", "basilisk.debugging", false);
      }
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

// Implements [VSIX-STATUS-BAR] — server-state faces: starting → $(sync~spin)
// ("analyzing"), ready → $(check), error → $(error) (server failed/not running),
// stopped → $(circle-slash). Note: the spec lists only check/warning/error/
// sync~spin; "stopped" uses $(circle-slash) (not in the spec's enumerated list).
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
      item.tooltip = "Basilisk language server running — click to configure";
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

// Implements [VSIX-STATUS-BAR] — per-file diagnostic count face. DEVIATION from
// spec text: the spec shows "$(warning) Basilisk (3) — errors in current file",
// but errors use the $(error) icon (red errorBackground) and warnings use
// $(warning) (warningBackground); no issues → $(check). The spec's example
// conflates the warning icon with an error count.
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

// Implements [VSIX-ERROR-RECOVERY] — resolves the binary then starts LSP mode or,
// when basilisk.useLsp is false, the subprocess fallback ([VSIX-CONFIGURATION-
// SETTINGS-VS-CODE-ONLY]). On failure it surfaces a user-visible error
// (reportRuntimeFailure) and flips the status bar to the error face.
// [VSIX-BINARY-RESOLUTION] is delegated to resolveBasiliskRuntime (Shipwright).
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
