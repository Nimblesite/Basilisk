// Implements [VSIX-LSP-CLIENT-CONFIGURATION]. See docs/specs/VSIX-SPEC.md#VSIX-LSP-CLIENT-CONFIGURATION
/**
 * LSP client setup and lifecycle for the Basilisk VS Code extension.
 *
 * State transitions are handled inside the Store via onDidChangeState.
 * This module handles IO side effects (logging, config forwarding, tab
 * tracking) by reacting to the store's lspState signal.
 */

import * as vscode from "vscode";
import {
  LanguageClient,
  type LanguageClientOptions,
  type ServerOptions,
  CloseAction,
  ErrorAction,
  RevealOutputChannelOn,
} from "vscode-languageclient/node";
import { effect } from "@preact/signals-core";
import { Logger } from "./logger";
import { createLspTraceChannel } from "./lsp-trace";
import { BASILISK_DOCUMENT_SELECTOR } from "./lsp-document-selector";
import { CONFIGURATION_EDITOR_COMMAND } from "./configuration-editor";
import { type Store, type LspState } from "./store";

/** Maximum LSP errors before shutting down the server. */
const MAX_LSP_ERRORS_BEFORE_SHUTDOWN = 3;

/** Read all Basilisk settings from the VS Code configuration. */
function readInlayHints(cfg: vscode.WorkspaceConfiguration): Record<string, unknown> {
  return {
    parameterNames: cfg.get<boolean>("inlayHints.parameterNames") ?? true,
    variableTypes: cfg.get<boolean>("inlayHints.variableTypes") ?? true,
  };
}

function readUvSettings(cfg: vscode.WorkspaceConfiguration): Record<string, unknown> {
  return {
    enabled: cfg.get<boolean>("uv.enabled") ?? true,
    executablePath: cfg.get<string>("uv.executablePath") ?? "",
    autoSync: cfg.get<boolean>("uv.autoSync") ?? false,
  };
}

function readTestExplorerSettings(cfg: vscode.WorkspaceConfiguration): Record<string, unknown> {
  return {
    enabled: cfg.get<boolean>("testExplorer.enabled") ?? true,
    framework: cfg.get<string>("testExplorer.framework") ?? "auto",
    pytestPath: cfg.get<string>("testExplorer.pytestPath") ?? "pytest",
    args: cfg.get<string[]>("testExplorer.args") ?? [],
    autoDiscoverOnSave: cfg.get<boolean>("testExplorer.autoDiscoverOnSave") ?? true,
    useUvRun: cfg.get<boolean>("testExplorer.useUvRun") ?? true,
  };
}

// Implements [VSIX-CONFIGURATION-SETTINGS] — reads the basilisk.* settings whose
// package.json schema is declared in vscode-extension/package.json (contributes.
// configuration); these are forwarded to the LSP server as initializationOptions
// and on didChangeConfiguration. [VSIX-CONFIGURATION-SETTINGS-VS-CODE-ONLY]:
// basilisk.useLsp / basilisk.trace.server are consumed here and in extension.ts,
// not sent to the server.
// Implements the editor-setting source of [ANALYSIS-CONFIG-SRC] — `analysisMode`
// (default "wholeModule") is read from the workspace setting, the highest-priority
// config source ([ANALYSIS-CONFIG-PRI]), and forwarded to the server.
export function readBasiliskSettings(): Record<string, unknown> {
  const cfg = vscode.workspace.getConfiguration("basilisk");
  // The "Type Checking" toggle (`basilisk.enabled`) MUST reach the server — the
  // LSP is authoritative for diagnostics, so it clears/suppresses them when the
  // toggle is off. Omitting it here left the toggle a cosmetic no-op (GitHub
  // #65 / #119). Implements [ANALYSIS-ENABLED] (server side) and the
  // [EXTACT-INFO-FEATURE-STATUS] "Type Checking" effect.
  const enabled = cfg.get<boolean>("enabled") ?? true;
  // [LSPARCH-DIAGNOSTIC-SCOPE]: `basilisk.analyze` is the per-user editor
  // opt-out that restricts publication to check scope (pep rules only). It is
  // relayed as initializationOptions.basilisk.analyze; project configuration
  // grades rules and never selects commands.
  const analyze = cfg.get<boolean>("analyze") ?? true;
  return {
    enabled,
    analysisMode: cfg.get<string>("analysisMode") ?? "wholeModule",
    basilisk: {
      enabled,
      analyze,
      python: cfg.get<string>("python") ?? "",
      analysisMode: cfg.get<string>("analysisMode") ?? "wholeModule",
      inlayHints: readInlayHints(cfg),
      formatter: cfg.get<string>("formatter") ?? "ruff",
    },
    formatter: cfg.get<string>("formatter") ?? "ruff",
    uv: readUvSettings(cfg),
    testExplorer: readTestExplorerSettings(cfg),
  };
}

function buildServerSettings(): Record<string, unknown> {
  return { basilisk: readBasiliskSettings() };
}

export type StatusBarUpdater = (state: "starting" | "ready" | "error" | "stopped") => void;

interface LspClientOptions {
  context: vscode.ExtensionContext;
  executablePath: string;
  outputChannel: vscode.LogOutputChannel | undefined;
}

// Implements [VSIX-LSP-CLIENT-CONFIGURATION] — builds ServerOptions/clientOptions
// and starts the LanguageClient ("basilisk lsp" over stdio). The executablePath
// arrives from binary resolution ([VSIX-BINARY-RESOLUTION], delegated to
// Shipwright in shipwright-runtime.ts).
export function startLspClient(
  options: LspClientOptions,
  store: Store,
  updateStatusBar: StatusBarUpdater
): void {
  const { context, executablePath, outputChannel } = options;
  const serverOptions: ServerOptions = {
    command: executablePath,
    args: ["lsp"],
    options: {
      // Point the server at the bundled debugpy so debugging works without the
      // user installing debugpy into their interpreter. The server ignores this
      // when the directory is absent (e.g. a dev build), falling back to the
      // interpreter's own debugpy.
      env: { ...process.env, BASILISK_DEBUGPY_PATH: context.asAbsolutePath("bundled/debugpy") },
    },
  };

  // [VSIX-OUTPUT-CHANNELS] "Basilisk LSP Trace" channel — surfaces LSP
  // communication when basilisk.trace.server is enabled.
  const traceChannel = createLspTraceChannel();
  context.subscriptions.push(traceChannel);

  const clientOptions = buildClientOptions(outputChannel, traceChannel, updateStatusBar);

  const lspClient = new LanguageClient(
    "basilisk",
    "Basilisk Type Checker",
    serverOptions,
    clientOptions
  );

  // Remove the built-in ExecuteCommandFeature to prevent it from calling
  // vscode.commands.registerCommand for server-advertised commands.
  // This avoids "command already exists" crashes on extension reload.
  // All command execution is handled by the executeCommand middleware.
  removeExecuteCommandFeature(lspClient);

  // setClient wires up onDidChangeState internally — the store owns
  // all state transitions (server commands, ready handle, lspState).
  store.setClient(context, lspClient);

  updateStatusBar("starting");
  bindLspStateEffects(store, updateStatusBar);
  registerConfigForwarding(context, store);
  registerTabTracking(context, store);

  lspClient.start().catch((error: unknown) => {
    const errorMessage = error instanceof Error ? error.message : String(error);
    const msg =
      `Basilisk: Failed to start language server. ` +
      `Shipwright selected '${executablePath}'. ${errorMessage}`;
    vscode.window.showErrorMessage(msg);
    Logger.error(msg);
    updateStatusBar("error");
  });

  context.subscriptions.push(lspClient);
}

/** Map store lspState to status bar + logging side effects. */
const LSP_STATE_LOG: Record<LspState, string> = {
  idle: "",
  starting: "Basilisk language server is starting...",
  running: "Basilisk language server is running.",
  stopped: "Basilisk language server stopped.",
};

const LSP_STATE_TO_STATUS: Record<LspState, "starting" | "ready" | "stopped" | undefined> = {
  idle: undefined,
  starting: "starting",
  running: "ready",
  stopped: "stopped",
};

function bindLspStateEffects(store: Store, updateStatusBar: StatusBarUpdater): void {
  effect(() => {
    const state = store.lspState.value;
    const logMsg = LSP_STATE_LOG[state];
    if (logMsg !== "") {
      Logger.info(logMsg);
    }
    const statusBarState = LSP_STATE_TO_STATUS[state];
    if (statusBarState !== undefined) {
      updateStatusBar(statusBarState);
    }
    // [EXTACT-EDITORS-VSCODE] basilisk.serverState context key — gates the
    // server-dependent Modules-toolbar buttons (Fix All / Organize Imports /
    // Restart) so they only render with a live handler behind them (#103).
    void vscode.commands.executeCommand("setContext", "basilisk.serverState", state);
  });
}

function buildClientOptions(
  outputCh: vscode.LogOutputChannel | undefined,
  traceCh: vscode.LogOutputChannel,
  updateStatusBar: StatusBarUpdater
): LanguageClientOptions {
  // Implements [VSIX-LSP-CLIENT-CONFIGURATION] — documentSelector (python files),
  // synchronize.configurationSection "basilisk", initializationOptions, and the
  // trace channel wiring per the spec's client-options shape.
  return {
    documentSelector: BASILISK_DOCUMENT_SELECTOR,
    synchronize: {
      configurationSection: "basilisk",
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.{py,pyi}"),
    },
    initializationOptions: readBasiliskSettings(),
    traceOutputChannel: traceCh,
    outputChannel: outputCh,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    // Implements [VSIX-ERROR-RECOVERY] — errorHandler shuts the server down after
    // MAX_LSP_ERRORS_BEFORE_SHUTDOWN (3) errors and auto-restarts on close.
    errorHandler: {
      error: (error, _message, count) => {
        Logger.error(`LSP error: ${error.message ?? error}`);
        if (count !== undefined && count < MAX_LSP_ERRORS_BEFORE_SHUTDOWN) {
          return { action: ErrorAction.Continue };
        }
        updateStatusBar("error");
        return { action: ErrorAction.Shutdown };
      },
      closed: () => {
        Logger.warn("LSP connection closed. Restarting...");
        return { action: CloseAction.Restart };
      },
    },
    middleware: {
      executeCommand: executeCommandMiddleware,
      // eslint-disable-next-line max-params -- vscode-languageclient fixes the 4-arg middleware signature.
      provideHover: async (document, position, token, next) =>
        trustConfigureSeverityLinks(await next(document, position, token)),
      workspace: {
        configuration: async (params, token, next) => {
          const results = await next(params, token);
          if (!Array.isArray(results)) {
            return results;
          }
          return (results as unknown[]).map((item: unknown, idx: number) => {
            const section = params.items[idx]?.section;
            if (section === "basilisk" || section?.startsWith("basilisk.")) {
              return {
                ...(typeof item === "object" && item !== null ? item as Record<string, unknown> : {}),
                ...readBasiliskSettings(),
              };
            }
            return item as Record<string, unknown>;
          });
        },
      },
    },
  };
}

// Implements the client wiring of [EXTACT-HEALTH-CONTEXT-MENU] (Fix All in File /
// Adopt File / Un-adopt File), the file-scoped half of [AUTOFIX-MASS-VSCODE]
// (`basilisk.fixFile` Safe tier / `basilisk.fixFileAll` all tier), and
// [AUTOFIX-ADOPTION-VSCODE] (`basilisk.adoptFile` / `basilisk.unadoptFile`) —
// these server-advertised, file-scoped commands get the active editor's URI
// injected so they act on the right file.
/** Commands that need the active editor URI injected as the first arg. */
const EDITOR_URI_COMMANDS = new Set([
  "basilisk.fixFile",
  "basilisk.fixFileAll",
  "basilisk.adoptFile",
  "basilisk.unadoptFile",
]);

// Implements the client UI of [LSPUV-COMMANDS] (the `{package}`-taking uv
// commands) — prompts the user for the package name before the server runs uv.
/** Commands that prompt the user for a package name before execution. */
const PACKAGE_COMMANDS: Record<string, { prompt: string; placeholder: string }> = {
  "basilisk.uv.add": { prompt: "Package name to add", placeholder: "e.g. requests" },
  "basilisk.uv.addDev": { prompt: "Dev package name to add", placeholder: "e.g. pytest" },
  "basilisk.uv.remove": { prompt: "Package name to remove", placeholder: "e.g. requests" },
};

/** Post-execution toast messages keyed by command name. */
const TOAST_MESSAGES: Record<string, string> = {
  "basilisk.uv.sync": "Basilisk: uv sync complete.",
  "basilisk.uv.lock": "Basilisk: uv lock complete.",
  "basilisk.uv.createEnv": "Basilisk: Virtual environment created.",
};

type NextFn = (command: string, args: unknown[]) => Thenable<unknown>;

/**
 * The LSP embeds `command:basilisk.openConfigurationEditor` links in hover
 * markdown for non-PEP diagnostics (the Configure Severity deep link,
 * [CONFIGEDITOR-VSIX-EXPERIENCE]). VS Code renders LSP hover markdown
 * untrusted by default, which strips command links — so trust hover content
 * for exactly that one command and nothing else.
 */
export function trustConfigureSeverityLinks<T extends vscode.Hover | null | undefined>(
  hover: T,
): T {
  if (hover === null || hover === undefined) { return hover; }
  for (const content of hover.contents) {
    if (content instanceof vscode.MarkdownString) {
      content.isTrusted = { enabledCommands: [CONFIGURATION_EDITOR_COMMAND] };
    }
  }
  return hover;
}

function activeOrVisibleFileEditor(): vscode.TextEditor | undefined {
  const active = vscode.window.activeTextEditor;
  if (active?.document.uri.scheme === "file") {
    return active;
  }

  return vscode.window.visibleTextEditors.find(
    (editor) => editor.document.uri.scheme === "file" && editor.document.languageId === "python"
  ) ?? vscode.window.visibleTextEditors.find((editor) => editor.document.uri.scheme === "file");
}

/**
 * Middleware for `workspace/executeCommand`. Injects client-side UI (editor
 * URI resolution, input prompts, toast notifications) around server-advertised
 * commands. This is the correct place for client-side behavior — server
 * commands are never pre-registered with `registerCommand()`.
 *
 * See LSP-ARCHITECTURE-SPEC.md § Command Registration Rule.
 */
async function executeCommandMiddleware(
  command: string,
  args: unknown[],
  next: NextFn
): Promise<unknown> {
  if (EDITOR_URI_COMMANDS.has(command)) {
    const editor = activeOrVisibleFileEditor();
    if (editor === undefined) { return undefined; }
    args = [editor.document.uri.toString()];
  }

  const pkgCmd = PACKAGE_COMMANDS[command];
  if (pkgCmd !== undefined && (args.length === 0 || args[0] === undefined)) {
    // Only prompt if the LSP didn't already provide the package name
    // (e.g. when invoked from the command palette, not from a code action).
    const packageName = await vscode.window.showInputBox({
      prompt: pkgCmd.prompt,
      placeHolder: pkgCmd.placeholder,
    });
    if (packageName === undefined || packageName === "") { return undefined; }
    args = [{ package: packageName }];
  }

  const result: unknown = await next(command, args);

  // Only show an optimistic success toast when the LSP reports the command
  // actually succeeded. A failed uv command (e.g. `uv add _pydevd_bundle`)
  // already surfaces its own error toast from the server; showing "Added X"
  // as well would contradict it and lie about the outcome.
  // Implements [LSPUV-ACTIONS-EXECUTION]. See issue #84.
  if (uvCommandSucceeded(result)) {
    showUvSuccessToast(command, args, pkgCmd !== undefined);
  }

  return result;
}

/**
 * True when an LSP `basilisk.uv.*` result reports success.
 *
 * Every uv command handler returns `{ success, stdout, stderr }`; failures and
 * "no pyproject.toml" no-ops set `success: false`. Treat anything that isn't an
 * explicit `success: true` as a non-success so we never show a misleading
 * success toast over the server's error toast (issue #84).
 */
function uvCommandSucceeded(result: unknown): boolean {
  return (
    typeof result === "object" &&
    result !== null &&
    (result as { success?: unknown }).success === true
  );
}

/** Show the post-execution success toast for a uv command, if it warrants one. */
function showUvSuccessToast(command: string, args: unknown[], isPackageCmd: boolean): void {
  const staticToast = TOAST_MESSAGES[command];
  if (staticToast !== undefined) {
    vscode.window.showInformationMessage(staticToast);
    return;
  }
  if (isPackageCmd && args.length > 0) {
    // A code action passes the package as a bare string (e.g. ["types-six"]);
    // the command-palette prompt path passes [{ package: "..." }]. Accept both.
    const arg = args[0];
    const pkg = typeof arg === "string"
      ? arg
      : (arg as { package?: string } | undefined)?.package;
    const verb = command === "basilisk.uv.remove" ? "Removed" :
      command === "basilisk.uv.addDev" ? "Added dev dependency" : "Added";
    vscode.window.showInformationMessage(`Basilisk: ${verb} ${pkg}.`);
  }
}

/**
 * Create a VS Code command handler that routes through the executeCommand
 * middleware and then sends `workspace/executeCommand` to the LSP server.
 *
 * This replaces the vscode-languageclient `ExecuteCommandFeature` which was
 * removed to prevent double-registration crashes. The store calls this for
 * each server-advertised command and registers the result with
 * `vscode.commands.registerCommand`.
 */
export function createServerCommandHandler(
  client: LanguageClient,
  command: string
): (...args: unknown[]) => Promise<unknown> {
  return async (...args: unknown[]) => {
    async function next(cmd: string, a: unknown[]): Promise<unknown> {
      return client.sendRequest("workspace/executeCommand", {
        command: cmd,
        arguments: a,
      });
    }
    return executeCommandMiddleware(command, args, next);
  };
}

function registerConfigForwarding(context: vscode.ExtensionContext, store: Store): void {
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      const lspClient = store.client.value;
      if (e.affectsConfiguration("basilisk") && lspClient?.isRunning() === true) {
        void lspClient.sendNotification("workspace/didChangeConfiguration", {
          settings: buildServerSettings(),
        });
      }
    })
  );
}

function registerTabTracking(context: vscode.ExtensionContext, store: Store): void {
  let knownOpenUris = collectOpenPythonUris();

  context.subscriptions.push(
    vscode.window.tabGroups.onDidChangeTabs(() => {
      const lspClient = store.client.value;
      if (lspClient?.isRunning() !== true) {return;}

      const currentUris = collectOpenPythonUris();

      const mode = vscode.workspace.getConfiguration("basilisk").get<string>("analysisMode") ?? "wholeModule";
      for (const uriStr of knownOpenUris) {
        if (currentUris.has(uriStr)) {
          continue;
        }
        const uri = vscode.Uri.parse(uriStr);
        // Implements the tab-close clause of [ANALYSIS-PUBLISH]
        // (docs/specs/LSP-ANALYSIS-MODES-SPEC.md). VS Code disposes closed
        // documents lazily, so the language client's own didClose can lag a
        // tab close by an unbounded amount. Send a synthetic didClose
        // whenever the server would clear diagnostics on close: every file in
        // openFilesOnly, and out-of-workspace files in the whole-workspace
        // modes — otherwise their stale diagnostics linger in the Problems
        // panel (GitHub #264). In-workspace files in wholeModule/crossModule
        // keep their diagnostics by design.
        const inWorkspace = vscode.workspace.getWorkspaceFolder(uri) !== undefined;
        if (mode === "openFilesOnly" || !inWorkspace) {
          void lspClient.sendNotification("textDocument/didClose", {
            textDocument: { uri: uri.toString() },
          });
        }
      }

      knownOpenUris = currentUris;
    })
  );
}

/**
 * Remove the built-in ExecuteCommandFeature from a LanguageClient.
 *
 * The library's ExecuteCommandFeature calls vscode.commands.registerCommand
 * for every server-advertised command. On extension reload, the old
 * registrations persist and the re-registration throws "command already
 * exists", killing the client. Since all command execution flows through
 * our executeCommand middleware, the feature is unnecessary.
 */
function removeExecuteCommandFeature(client: LanguageClient): void {
  const METHOD = "workspace/executeCommand";
  const internals = client as unknown as {
    _features: { registrationType?: { method?: string } }[];
    _dynamicFeatures: Map<string, unknown>;
  };

  const idx = internals._features.findIndex(
    (f) => f.registrationType?.method === METHOD
  );
  if (idx !== -1) {
    internals._features.splice(idx, 1);
  }

  // The feature is also stored in _dynamicFeatures — if left there,
  // the client's handleRegistrationRequest path can still call register()
  // on it, which triggers vscode.commands.registerCommand and crashes
  // with "command already exists" on reload.
  internals._dynamicFeatures.delete(METHOD);
}

function collectOpenPythonUris(): Set<string> {
  const uris = new Set<string>();
  for (const group of vscode.window.tabGroups.all) {
    for (const tab of group.tabs) {
      const input = tab.input;
      if (input instanceof vscode.TabInputText) {
        if (input.uri.scheme === "file" && input.uri.fsPath.endsWith(".py")) {
          uris.add(input.uri.toString());
        }
      }
    }
  }
  return uris;
}
