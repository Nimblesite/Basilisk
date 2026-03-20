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
  RevealOutputChannelOn,
  State,
} from "vscode-languageclient/node";
import { logger, setLogBackend, FileLogSink } from "./logger";
import type { LogSink } from "./logger";
import { DapTcpProxy } from "./dap-proxy";

let client: LanguageClient | undefined;
let statusBarItem: vscode.StatusBarItem | undefined;
let outputChannel: vscode.OutputChannel | undefined;

/** Adapts a VS Code LogOutputChannel to our LogSink interface. */
class VscodeLogSink implements LogSink {
  constructor(private readonly channel: vscode.LogOutputChannel) {}
  trace(message: string): void { this.channel.trace(message); }
  debug(message: string): void { this.channel.debug(message); }
  info(message: string): void { this.channel.info(message); }
  warn(message: string): void { this.channel.warn(message); }
  error(message: string): void { this.channel.error(message); }
}

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
  const logChannel = vscode.window.createOutputChannel("Basilisk", { log: true });
  outputChannel = logChannel;

  // Always write logs to a file so they're visible after headless test runs.
  const logFilePath = path.join(os.tmpdir(), "basilisk-debug-trace.log");
  const fileSink = new FileLogSink(logFilePath);
  setLogBackend([new VscodeLogSink(logChannel), fileSink]);
  logChannel.info(`Log file: ${logFilePath}`);

  context.subscriptions.push(logChannel);

  // Status bar item — shows server state and diagnostic count.
  statusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    100
  );
  statusBarItem.command = "basilisk.showOutput";
  context.subscriptions.push(statusBarItem);

  const cfg = vscode.workspace.getConfiguration("basilisk");
  const configuredPath =
    process.env.BASILISK_EXECUTABLE_PATH ??
    cfg.get<string>("executablePath") ??
    "basilisk";
  const executablePath = resolveExecutablePath(configuredPath);
  const useLsp = cfg.get<boolean>("useLsp") ?? true;

  logger.info(`Basilisk executable: ${executablePath}`);

  // Register commands safely — avoids "command already exists" errors when
  // the extension re-activates after an LSP crash/restart cycle.
  safeRegisterCommand(context, "basilisk.restartServer", async () => {
    if (!client) {
      vscode.window.showWarningMessage("Basilisk: No language server to restart.");
      return;
    }
    try {
      logger.info("Restarting Basilisk language server...");
      await client.stop();
      await client.start();
      logger.info("Basilisk language server restarted.");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      logger.error(`Restart failed: ${msg}`);
      vscode.window.showErrorMessage(`Basilisk: Failed to restart server: ${msg}`);
    }
  });

  safeRegisterCommand(context, "basilisk.showOutput", () => {
    outputChannel?.show();
  });

  safeRegisterCommand(context, "basilisk.fixFile", async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.uri.scheme !== "file") {
      return;
    }
    const uri = editor.document.uri;

    if (client) {
      // LSP mode — send executeCommand to the server.
      await client.sendRequest("workspace/executeCommand", {
        command: "basilisk.fixFile",
        arguments: [uri.toString()],
      });
    } else {
      // Fallback — trigger VS Code's built-in "fix all" code action.
      await vscode.commands.executeCommand("editor.action.fixAll");
    }
  });

  if (useLsp) {
    // In LSP mode, the LanguageClient automatically registers
    // basilisk.organizeImports via the server's executeCommandProvider.
    // Do NOT register it manually — that would conflict.
    startLspClient(context, executablePath);

    // Register the debug adapter factory — it asks the LSP to spawn debugpy
    // and returns a TCP port for the editor's DAP client to connect to.
    context.subscriptions.push(
      vscode.debug.registerDebugAdapterDescriptorFactory(
        "basilisk-debug",
        new BasiliskDebugAdapterFactory()
      )
    );

    // Register a DAP message tracker for comprehensive debug logging.
    context.subscriptions.push(
      vscode.debug.registerDebugAdapterTrackerFactory(
        "basilisk-debug",
        new BasiliskDebugAdapterTrackerFactory()
      )
    );

    // Log debug session lifecycle events.
    context.subscriptions.push(
      vscode.debug.onDidStartDebugSession((session) => {
        logger.info(
          `Debug session started: id=${session.id}, name=${session.name}, type=${session.type}`
        );
      })
    );
    context.subscriptions.push(
      vscode.debug.onDidTerminateDebugSession((session) => {
        const activeId = vscode.debug.activeDebugSession?.id ?? "undefined";
        const sameSession = activeId === session.id;
        logger.info(
          `[Lifecycle] onDidTerminateDebugSession fired: ` +
          `terminated=${session.id.slice(0, 8)}, name="${session.name}", ` +
          `activeDebugSession=${activeId.slice(0, 8)}, ` +
          `sameSession=${sameSession}`
        );
        logger.info(
          `[Lifecycle] This is the VS Code API race: activeDebugSession ` +
          `should be undefined here but is ${activeId === "undefined" ? "correctly undefined" : "STILL SET (id=" + activeId.slice(0, 8) + ")"}`
        );
      })
    );
    context.subscriptions.push(
      vscode.debug.onDidChangeActiveDebugSession((session) => {
        logger.info(
          `[Lifecycle] onDidChangeActiveDebugSession: ${session ? `id=${session.id.slice(0, 8)}, name="${session.name}"` : "→ NONE (session cleared)"}`
        );
      })
    );
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
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.{py,pyi}"),
    },
    initializationOptions: readBasiliskSettings(),
    traceOutputChannel: traceChannel,
    outputChannel: outputChannel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    errorHandler: {
      error: (error, _message, count) => {
        logger.error(`LSP error: ${error.message ?? error}`);
        if (count !== undefined && count < 3) {
          return { action: ErrorAction.Continue };
        }
        updateStatusBar("error");
        return { action: ErrorAction.Shutdown };
      },
      closed: () => {
        logger.warn("LSP connection closed. Restarting...");
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
          return results.map((item, idx) => {
            const section = params.items[idx]?.section;
            if (section === "basilisk" || section?.startsWith("basilisk.")) {
              return {
                ...(typeof item === "object" && item !== null ? item : {}),
                ...readBasiliskSettings(),
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
        logger.info("Basilisk language server is running.");
        updateStatusBar("ready");
        break;
      case State.Stopped:
        logger.info("Basilisk language server stopped.");
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

  // Track open Python URIs so we can detect when an editor tab closes.
  // VS Code does not reliably fire workspace.onDidCloseTextDocument when
  // an editor tab is closed — the document stays alive in memory. We use
  // the tab model to detect when a Python file is no longer open in any
  // tab and manually send textDocument/didClose to the server.
  let knownOpenUris = new Set<string>();

  context.subscriptions.push(
    vscode.window.tabGroups.onDidChangeTabs(() => {
      if (!client?.isRunning()) return;

      // Collect all Python file URIs currently open in tabs.
      const currentUris = new Set<string>();
      for (const group of vscode.window.tabGroups.all) {
        for (const tab of group.tabs) {
          const input = tab.input;
          if (input instanceof vscode.TabInputText) {
            if (input.uri.scheme === "file" && input.uri.fsPath.endsWith(".py")) {
              currentUris.add(input.uri.toString());
            }
          }
        }
      }

      // Any URI that was known-open but is no longer in any tab has been closed.
      // Only send didClose in openFilesOnly mode — in wholeModule/crossModule
      // diagnostics should persist for closed files.
      const mode = vscode.workspace.getConfiguration("basilisk").get<string>("analysisMode") ?? "wholeModule";
      if (mode === "openFilesOnly") {
        for (const uriStr of knownOpenUris) {
          if (!currentUris.has(uriStr)) {
            const uri = vscode.Uri.parse(uriStr);
            client.sendNotification("textDocument/didClose", {
              textDocument: { uri: uri.toString() },
            });
          }
        }
      }

      knownOpenUris = currentUris;
    })
  );

  // Seed the set with currently open Python tabs.
  for (const group of vscode.window.tabGroups.all) {
    for (const tab of group.tabs) {
      const input = tab.input;
      if (input instanceof vscode.TabInputText) {
        if (input.uri.scheme === "file" && input.uri.fsPath.endsWith(".py")) {
          knownOpenUris.add(input.uri.toString());
        }
      }
    }
  }

  client.start().catch((error: Error) => {
    const msg =
      `Basilisk: Failed to start language server. ` +
      `Is '${executablePath}' installed and on PATH? ${error.message}`;
    vscode.window.showErrorMessage(msg);
    logger.error(msg);
    updateStatusBar("error");
  });

  context.subscriptions.push(client);
}

/** Read all Basilisk settings from the VS Code configuration. */
function readBasiliskSettings(): Record<string, unknown> {
  const cfg = vscode.workspace.getConfiguration("basilisk");
  return {
    analysisMode: cfg.get<string>("analysisMode") ?? "wholeModule",
    basilisk: {
      python: cfg.get<string>("python") ?? "",
      analysisMode: cfg.get<string>("analysisMode") ?? "wholeModule",
      inlayHints: {
        parameterNames: cfg.get<boolean>("inlayHints.parameterNames") ?? true,
        variableTypes: cfg.get<boolean>("inlayHints.variableTypes") ?? true,
      },
      ruff: {
        enabled: cfg.get<boolean>("ruff.enabled") ?? true,
        executablePath: cfg.get<string>("ruff.executablePath") ?? "ruff",
      },
    },
    ruff: {
      enabled: cfg.get<boolean>("ruff.enabled") ?? true,
      executablePath: cfg.get<string>("ruff.executablePath") ?? "ruff",
    },
  };
}

/** Build the settings object forwarded to the LSP server. */
function buildServerSettings(): Record<string, unknown> {
  return { basilisk: readBasiliskSettings() };
}

/** Run `ruff check --select I --fix` on the active file to organize imports. */
function organizeImports(): void {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "python") {
    vscode.window.showWarningMessage("Basilisk: Open a Python file to organize imports.");
    return;
  }

  const settings = readBasiliskSettings() as { ruff?: { enabled?: boolean; executablePath?: string } };
  if (!settings.ruff?.enabled) {
    vscode.window.showWarningMessage("Basilisk: Ruff integration is disabled. Enable basilisk.ruff.enabled to organize imports.");
    return;
  }

  const ruffPath = settings.ruff?.executablePath ?? "ruff";
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
        logger.error(`organizeImports error: ${stderr}`);
        return;
      }
      logger.info(`Imports organized in ${path.basename(filePath)}`);
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

// ── Debug adapter tracker (DAP message logging) ──────────────────────────

/**
 * Factory that creates per-session DAP message trackers. Every DAP message
 * to/from debugpy is logged to the Basilisk output channel so we can
 * diagnose stepping, termination, and connection issues.
 */
class BasiliskDebugAdapterTrackerFactory
  implements vscode.DebugAdapterTrackerFactory
{
  createDebugAdapterTracker(
    session: vscode.DebugSession
  ): vscode.ProviderResult<vscode.DebugAdapterTracker> {
    return new BasiliskDebugAdapterTracker(session);
  }
}

class BasiliskDebugAdapterTracker implements vscode.DebugAdapterTracker {
  private readonly sessionId: string;
  private readonly sessionName: string;
  private readonly session: vscode.DebugSession;

  constructor(session: vscode.DebugSession) {
    this.sessionId = session.id.slice(0, 8);
    this.sessionName = session.name;
    this.session = session;
  }

  onWillStartSession(): void {
    logger.info(`[DAP ${this.sessionId}] session "${this.sessionName}" starting`);
  }

  onWillStopSession(): void {
    logger.info(`[DAP ${this.sessionId}] session "${this.sessionName}" stopping`);
  }

  onWillReceiveMessage(message: unknown): void {
    const msg = message as { type?: string; command?: string; seq?: number; arguments?: unknown };
    if (msg.type === "request") {
      logger.debug(
        `[DAP ${this.sessionId}] --> ${msg.command} #${msg.seq} ${summarizeArgs(msg.arguments)}`
      );
    }
  }

  onDidSendMessage(message: unknown): void {
    const msg = message as {
      type?: string;
      command?: string;
      event?: string;
      seq?: number;
      request_seq?: number;
      success?: boolean;
      body?: unknown;
    };
    if (msg.type === "response") {
      const text = `[DAP ${this.sessionId}] <-- ${msg.command} #${msg.request_seq} success=${msg.success} ${summarizeBody(msg.body)}`;
      if (msg.success) {
        logger.debug(text);
      } else {
        logger.warn(text);
      }
    } else if (msg.type === "event") {
      logger.debug(
        `[DAP ${this.sessionId}] <-- event:${msg.event} ${summarizeBody(msg.body)}`
      );
      if (msg.event === "terminated") {
        logger.info(`[DAP ${this.sessionId}] program terminated`);
      }
    }
  }

  onError(error: Error): void {
    logger.error(`[DAP ${this.sessionId}] ${error.message}`);
  }

  onExit(code: number | undefined, signal: string | undefined): void {
    logger.warn(`[DAP ${this.sessionId}] exit code=${code ?? "?"}, signal=${signal ?? "none"}`);
  }
}

/** Compact summary of DAP request arguments for logging. */
function summarizeArgs(args: unknown): string {
  if (!args || typeof args !== "object") return "";
  const obj = args as Record<string, unknown>;
  const parts: string[] = [];
  if ("threadId" in obj) parts.push(`thread=${obj.threadId}`);
  if ("expression" in obj) parts.push(`expr="${obj.expression}"`);
  if ("frameId" in obj) parts.push(`frame=${obj.frameId}`);
  if ("context" in obj) parts.push(`ctx=${obj.context}`);
  if ("program" in obj) parts.push(`program=${String(obj.program).split("/").pop()}`);
  if ("lines" in obj) parts.push(`lines=${JSON.stringify(obj.lines)}`);
  if ("breakpoints" in obj) {
    const bps = obj.breakpoints as Array<{ line?: number }>;
    parts.push(`bps=[${bps.map((b) => b.line).join(",")}]`);
  }
  if ("source" in obj) {
    const src = obj.source as { path?: string };
    if (src.path) parts.push(`src=${src.path.split("/").pop()}`);
  }
  return parts.length > 0 ? `{${parts.join(", ")}}` : "";
}

/** Compact summary of DAP response/event body for logging. */
function summarizeBody(body: unknown): string {
  if (!body || typeof body !== "object") return "";
  const obj = body as Record<string, unknown>;
  const parts: string[] = [];
  if ("reason" in obj) parts.push(`reason=${obj.reason}`);
  if ("threadId" in obj) parts.push(`thread=${obj.threadId}`);
  if ("allThreadsStopped" in obj) parts.push(`allStopped=${obj.allThreadsStopped}`);
  if ("line" in obj) parts.push(`line=${obj.line}`);
  if ("name" in obj) parts.push(`name=${obj.name}`);
  if ("result" in obj) parts.push(`result=${obj.result}`);
  if ("stackFrames" in obj) {
    const frames = obj.stackFrames as Array<{ name?: string; line?: number }>;
    if (frames.length > 0) {
      parts.push(`frames=[${frames.map((f) => `${f.name}:${f.line}`).join(", ")}]`);
    }
  }
  if ("scopes" in obj) {
    const scopes = obj.scopes as Array<{ name?: string }>;
    parts.push(`scopes=[${scopes.map((s) => s.name).join(", ")}]`);
  }
  if ("variables" in obj) {
    const vars = obj.variables as Array<{ name?: string; value?: string }>;
    if (vars.length <= 10) {
      parts.push(`vars=[${vars.map((v) => `${v.name}=${v.value}`).join(", ")}]`);
    } else {
      parts.push(`vars=[${vars.length} items]`);
    }
  }
  if ("threads" in obj) {
    const threads = obj.threads as Array<{ id?: number; name?: string }>;
    parts.push(`threads=[${threads.map((t) => `${t.id}:${t.name}`).join(", ")}]`);
  }
  return parts.length > 0 ? `{${parts.join(", ")}}` : "";
}

// ── Debug adapter factory ─────────────────────────────────────────────────

/**
 * Asks the Basilisk LSP to spawn debugpy on a free TCP port, then tells
 * VS Code to connect its DAP client to that port. No process spawning in
 * TypeScript — the LSP handles everything.
 */
class BasiliskDebugAdapterFactory
  implements vscode.DebugAdapterDescriptorFactory
{
  async createDebugAdapterDescriptor(
    session: vscode.DebugSession
  ): Promise<vscode.DebugAdapterDescriptor> {
    const config = session.configuration;

    logger.info(
      `[Basilisk Debug] createDebugAdapterDescriptor called — ` +
      `type=${config.type}, request=${config.request}, ` +
      `program=${config.program ?? "(none)"}`
    );

    // Attach mode: connect directly to a user-specified host:port.
    // debugpy.adapter in --port mode accepts exactly ONE TCP connection.
    // If something probed the port before us (e.g. a readiness check),
    // that adapter is dead. Ask the LSP to spawn a fresh one, then connect.
    if (config.request === "attach" && config.connect) {
      let host = config.connect.host || "localhost";
      let port = config.connect.port as number;
      logger.info(`[Basilisk Debug] Attach mode → ${host}:${port}`);

      // Non-destructive check: is the port still alive?
      const alive = await this.isPortAlive(host, port);
      if (!alive && client) {
        logger.warn(`[Basilisk Debug] Port ${port} is dead — respawning debugpy adapter`);
        try {
          const result = (await vscode.commands.executeCommand(
            "basilisk.startDebugSession",
            { python: config.python || null }
          )) as { host: string; port: number; sessionId: string } | null;
          if (result && typeof result.port === "number") {
            logger.info(`[Basilisk Debug] Respawned debugpy on ${result.host}:${result.port}`);
            host = result.host;
            port = result.port;
          }
        } catch (err: unknown) {
          const msg = err instanceof Error ? err.message : String(err);
          logger.error(`[Basilisk Debug] Respawn failed: ${msg}`);
        }
      }

      const proxy = new DapTcpProxy(host, port);
      const proxyPort = await proxy.start();
      logger.info(`[Basilisk Debug] attach proxy listening on port ${proxyPort}`);
      return new vscode.DebugAdapterServer(proxyPort);
    }

    // Launch mode: ask the running LSP to spawn debugpy.
    if (!client) {
      throw new Error(
        "Basilisk: LSP client is not running. Cannot start debug session."
      );
    }

    // Resolve Python: launch config > basilisk.python setting > auto-detect (LSP side)
    const configuredPython =
      config.python ||
      vscode.workspace.getConfiguration("basilisk").get<string>("python") ||
      null;

    logger.info(
      `Requesting LSP to spawn debugpy (python: ${configuredPython ?? "auto-detect"})...`
    );

    let result: { host: string; port: number; sessionId: string } | null;
    try {
      // Use vscode.commands.executeCommand which the vscode-languageclient
      // bridges to workspace/executeCommand on the LSP server automatically.
      result = (await vscode.commands.executeCommand(
        "basilisk.startDebugSession",
        { python: configuredPython }
      )) as { host: string; port: number; sessionId: string } | null;

      if (!result || typeof result.port !== "number") {
        throw new Error(
          "LSP returned null for basilisk.startDebugSession. " +
          "Check the Basilisk output channel for details."
        );
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      logger.error(`Debug session start failed: ${msg}`);
      // Surface actionable errors to the user.
      if (msg.includes("debugpy not found") || msg.includes("pip install debugpy")) {
        vscode.window.showErrorMessage(
          `Basilisk Debug: debugpy is not installed. Run: pip install debugpy`,
          "Install debugpy"
        ).then((choice) => {
          if (choice === "Install debugpy") {
            const terminal = vscode.window.createTerminal("Basilisk");
            terminal.show();
            terminal.sendText("pip install debugpy");
          }
        });
      } else if (msg.includes("No Python interpreter") || msg.includes("python")) {
        vscode.window.showErrorMessage(
          `Basilisk Debug: No Python interpreter found. Set basilisk.python or create a virtualenv.`
        );
      } else {
        vscode.window.showErrorMessage(`Basilisk Debug: Failed to start debug session: ${msg}`);
      }
      throw new Error(`Basilisk: ${msg}`);
    }

    logger.info(
      `LSP spawned debugpy on ${result.host}:${result.port} (session: ${result.sessionId})`
    );

    // Use a TCP DAP proxy so we can fix debugpy stepping quirks
    // (e.g. auto-next after stepOut to complete return-value assignment,
    // structural line skipping for try: statements).
    // TCP-based proxy ensures VS Code manages its own session lifecycle,
    // giving clean activeDebugSession teardown.
    const proxy = new DapTcpProxy(result.host, result.port);
    const proxyPort = await proxy.start();
    logger.info(`[Basilisk Debug] launch proxy listening on port ${proxyPort}`);
    return new vscode.DebugAdapterServer(proxyPort);
  }

  /**
   * Non-destructive port check — attempts to **bind** to the port.
   * If binding fails with EADDRINUSE, something is listening (returns true).
   * This avoids making a TCP connection that would consume debugpy's single slot.
   */
  private isPortAlive(_host: string, port: number): Promise<boolean> {
    const net = require("net") as typeof import("net");
    return new Promise((resolve) => {
      const server = net.createServer();
      server.once("error", (err: NodeJS.ErrnoException) => {
        if (err.code === "EADDRINUSE") {
          resolve(true); // Something is listening.
        } else {
          resolve(false);
        }
      });
      server.listen(port, "127.0.0.1", () => {
        // We could bind → port is free → nothing is listening.
        server.close(() => resolve(false));
      });
    });
  }
}
