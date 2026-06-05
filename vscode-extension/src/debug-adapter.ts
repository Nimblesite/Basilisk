// Implements [VSIX-PYTHON-DEBUGGER-DAP]. See docs/specs/VSIX-SPEC.md#VSIX-PYTHON-DEBUGGER-DAP
/**
 * Debug adapter factory, DAP tracker, and logging utilities for Basilisk.
 */

import * as vscode from "vscode";
import * as net from "net";
import { type LanguageClient } from "vscode-languageclient/node";
import { Logger } from "./logger";
import { DapTcpProxy } from "./dap-proxy";
import { appendDebugOutput, clearDebugOutput } from "./dap-output";

/** Max number of variables to log inline before switching to a count summary. */
const MAX_INLINE_VARS = 10;

/** Length of an abbreviated session ID prefix. */
const SESSION_ID_PREFIX_LEN = 8;

// ── DAP message summarization ─────────────────────────────────────────────

/** Compact summary of DAP request arguments for logging. */
export function summarizeArgs(args: unknown): string {
  if (args === null || args === undefined || typeof args !== "object") {return "";}
  const obj = args as Record<string, unknown>;
  const parts: string[] = [];
  if ("threadId" in obj) {parts.push(`thread=${String(obj.threadId)}`);}
  if ("expression" in obj) {parts.push(`expr="${String(obj.expression)}"`);}
  if ("frameId" in obj) {parts.push(`frame=${String(obj.frameId)}`);}
  if ("context" in obj) {parts.push(`ctx=${String(obj.context)}`);}
  if ("program" in obj) {parts.push(`program=${String(obj.program).split("/").pop()}`);}
  if ("lines" in obj) {parts.push(`lines=${JSON.stringify(obj.lines)}`);}
  summarizeBreakpointsAndSource(obj, parts);
  return parts.length > 0 ? `{${parts.join(", ")}}` : "";
}

function summarizeBreakpointsAndSource(obj: Record<string, unknown>, parts: string[]): void {
  if ("breakpoints" in obj) {
    const bps = obj.breakpoints as { line?: number }[];
    parts.push(`bps=[${bps.map((b) => b.line).join(",")}]`);
  }
  if ("source" in obj) {
    const src = obj.source as { path?: string };
    if (src.path !== undefined && src.path !== "") {parts.push(`src=${src.path.split("/").pop()}`);}
  }
}

/** Compact summary of DAP response/event body for logging. */
export function summarizeBody(body: unknown): string {
  if (body === null || body === undefined || typeof body !== "object") {return "";}
  const obj = body as Record<string, unknown>;
  const parts: string[] = [];
  summarizeScalarFields(obj, parts);
  summarizeCollectionFields(obj, parts);
  return parts.length > 0 ? `{${parts.join(", ")}}` : "";
}

function summarizeScalarFields(obj: Record<string, unknown>, parts: string[]): void {
  if ("reason" in obj) {parts.push(`reason=${String(obj.reason)}`);}
  if ("threadId" in obj) {parts.push(`thread=${String(obj.threadId)}`);}
  if ("allThreadsStopped" in obj) {parts.push(`allStopped=${String(obj.allThreadsStopped)}`);}
  if ("line" in obj) {parts.push(`line=${String(obj.line)}`);}
  if ("name" in obj) {parts.push(`name=${String(obj.name)}`);}
  if ("result" in obj) {parts.push(`result=${String(obj.result)}`);}
}

function summarizeCollectionFields(obj: Record<string, unknown>, parts: string[]): void {
  if ("stackFrames" in obj) {
    const frames = obj.stackFrames as { name?: string; line?: number }[];
    if (frames.length > 0) {
      parts.push(`frames=[${frames.map((f) => `${String(f.name)}:${String(f.line)}`).join(", ")}]`);
    }
  }
  if ("scopes" in obj) {
    const scopes = obj.scopes as { name?: string }[];
    parts.push(`scopes=[${scopes.map((s) => String(s.name)).join(", ")}]`);
  }
  if ("variables" in obj) {
    const vars = obj.variables as { name?: string; value?: string }[];
    if (vars.length <= MAX_INLINE_VARS) {
      parts.push(`vars=[${vars.map((v) => `${String(v.name)}=${String(v.value)}`).join(", ")}]`);
    } else {
      parts.push(`vars=[${vars.length} items]`);
    }
  }
  if ("threads" in obj) {
    const threads = obj.threads as { id?: number; name?: string }[];
    parts.push(`threads=[${threads.map((t) => `${String(t.id)}:${String(t.name)}`).join(", ")}]`);
  }
}

// ── DAP message tracker ───────────────────────────────────────────────────

/**
 * Factory that creates per-session DAP message trackers.
 *
 * The tracker is the single observability point for debugpy → VS Code traffic,
 * so it captures both the debuggee `process` event (the PID the CPU profiler
 * targets — "same process") and `output` events (the marker payloads the
 * memory round-trip recovers). `onDebuggeeProcessId`, when supplied, receives
 * `(sessionId, pid)` once the `process` event arrives.
 */
export class BasiliskDebugAdapterTrackerFactory
  implements vscode.DebugAdapterTrackerFactory
{
  constructor(private readonly onDebuggeeProcessId?: DebuggeeProcessIdCallback) {}

  public createDebugAdapterTracker(
    session: vscode.DebugSession
  ): vscode.ProviderResult<vscode.DebugAdapterTracker> {
    return new BasiliskDebugAdapterTracker(session, this.onDebuggeeProcessId);
  }
}

class BasiliskDebugAdapterTracker implements vscode.DebugAdapterTracker {
  private readonly sessionId: string;
  private readonly fullSessionId: string;
  private readonly sessionName: string;

  constructor(
    session: vscode.DebugSession,
    private readonly onDebuggeeProcessId?: DebuggeeProcessIdCallback
  ) {
    this.sessionId = session.id.slice(0, SESSION_ID_PREFIX_LEN);
    this.fullSessionId = session.id;
    this.sessionName = session.name;
  }

  public onWillStartSession(): void {
    Logger.info(`[DAP ${this.sessionId}] session "${this.sessionName}" starting`);
  }

  public onWillStopSession(): void {
    Logger.info(`[DAP ${this.sessionId}] session "${this.sessionName}" stopping`);
    clearDebugOutput(this.fullSessionId);
  }

  public onWillReceiveMessage(message: unknown): void {
    const msg = message as { type?: string; command?: string; seq?: number; arguments?: unknown };
    if (msg.type === "request") {
      Logger.debug(`[DAP ${this.sessionId}] --> ${msg.command} #${msg.seq} ${summarizeArgs(msg.arguments)}`);
    }
  }

  public onDidSendMessage(message: unknown): void {
    const msg = message as {
      type?: string; command?: string; event?: string;
      seq?: number; request_seq?: number; success?: boolean; body?: unknown;
    };
    if (msg.type === "response") {
      const text = `[DAP ${this.sessionId}] <-- ${msg.command} #${msg.request_seq} success=${msg.success} ${summarizeBody(msg.body)}`;
      if (msg.success) {
        Logger.debug(text);
      } else {
        Logger.warn(text);
      }
    } else if (msg.type === "event") {
      this.handleEvent(msg.event, msg.body);
    }
  }

  /** Capture profiler-relevant events; log the rest. */
  private handleEvent(event: string | undefined, body: unknown): void {
    if (event === "output") {
      // Capture debuggee stdout/stderr so the memory round-trip can recover
      // the `__BASILISK_MEM*__` marker its injection scripts print (debugpy
      // delivers print() output here, not in the evaluate result).
      const text = (body as { output?: string } | undefined)?.output;
      if (typeof text === "string") {
        appendDebugOutput(this.fullSessionId, text);
      }
      return;
    }
    if (event === "process") {
      // The debuggee's OS PID — captured so the CPU profiler can attach to the
      // SAME process the debugger drives (DAP: body.systemProcessId).
      const pid = (body as { systemProcessId?: number } | undefined)?.systemProcessId;
      if (typeof pid === "number" && this.onDebuggeeProcessId !== undefined) {
        Logger.info(`[DAP ${this.sessionId}] debuggee systemProcessId=${pid}`);
        this.onDebuggeeProcessId(this.fullSessionId, pid);
      }
      return;
    }
    Logger.debug(`[DAP ${this.sessionId}] <-- event:${event} ${summarizeBody(body)}`);
    if (event === "terminated") {
      Logger.info(`[DAP ${this.sessionId}] program terminated`);
    }
  }

  public onError(error: Error): void {
    Logger.error(`[DAP ${this.sessionId}] ${error.message}`);
  }

  public onExit(code: number | undefined, signal: string | undefined): void {
    Logger.warn(`[DAP ${this.sessionId}] exit code=${code ?? "?"}, signal=${signal ?? "none"}`);
  }
}

// ── Debug adapter factory ─────────────────────────────────────────────────

/**
 * Non-destructive port check — attempts to bind to the port.
 * If binding fails with EADDRINUSE, something is listening.
 */
async function isPortAlive(_host: string, port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const server = net.createServer();
    server.once("error", (err: NodeJS.ErrnoException) => {
      resolve(err.code === "EADDRINUSE");
    });
    server.listen(port, "127.0.0.1", () => {
      server.close(() => { resolve(false); });
    });
  });
}

/** Callback that receives the debuggee OS PID once debugpy emits its `process` event. */
export type DebuggeeProcessIdCallback = (sessionId: string, pid: number) => void;

/** Handle attach mode: connect to user-specified host:port, respawning if needed. */
async function handleAttachMode(
  config: vscode.DebugConfiguration,
  lspClient: LanguageClient | undefined
): Promise<vscode.DebugAdapterDescriptor> {
  const connectInfo = config.connect as { host?: string; port: number };
  let host = connectInfo.host ?? "localhost";
  let port = connectInfo.port;
  Logger.info(`[Basilisk Debug] Attach mode → ${host}:${port}`);

  const alive = await isPortAlive(host, port);
  if (!alive && lspClient) {
    Logger.warn(`[Basilisk Debug] Port ${port} is dead — respawning debugpy adapter`);
    try {
      const result = await lspClient.sendRequest<{ host: string; port: number } | null>(
        "workspace/executeCommand",
        {
          command: "basilisk.startDebugSession",
          arguments: [{ python: (config.python as string | undefined) ?? null }],
        }
      );
      if (result !== undefined && result !== null && typeof result.port === "number") {
        Logger.info(`[Basilisk Debug] Respawned debugpy on ${result.host}:${result.port}`);
        host = result.host;
        port = result.port;
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      Logger.error(`[Basilisk Debug] Respawn failed: ${msg}`);
    }
  }

  const proxy = new DapTcpProxy(host, port);
  const proxyPort = await proxy.start();
  Logger.info(`[Basilisk Debug] attach proxy listening on port ${proxyPort}`);
  return new vscode.DebugAdapterServer(proxyPort);
}

/** Send startDebugSession to LSP and handle errors. */
async function requestDebugSession(
  lspClient: LanguageClient,
  python: string | null
): Promise<{ host: string; port: number; sessionId: string }> {
  try {
    const result = await lspClient.sendRequest<{ host: string; port: number; sessionId: string } | null>(
      "workspace/executeCommand",
      { command: "basilisk.startDebugSession", arguments: [{ python }] }
    );
    if (!result || typeof result.port !== "number") {
      throw new Error(
        "LSP returned null for basilisk.startDebugSession. " +
        "Check the Basilisk output channel for details."
      );
    }
    return result;
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Debug session start failed: ${msg}`);
    showDebugError(msg);
    throw new Error(`Basilisk: ${msg}`);
  }
}

/** Show user-facing error messages for debug session failures. */
function showDebugError(msg: string): void {
  if (msg.includes("debugpy not found") || msg.includes("pip install debugpy")) {
    void vscode.window.showErrorMessage(
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
}

/** Handle launch mode: ask LSP to spawn debugpy. */
async function handleLaunchMode(
  config: vscode.DebugConfiguration,
  lspClient: LanguageClient
): Promise<vscode.DebugAdapterDescriptor> {
  const configuredPython =
    (config.python as string | undefined) ??
    vscode.workspace.getConfiguration("basilisk").get<string>("python") ??
    null;

  Logger.info(`Requesting LSP to spawn debugpy (python: ${configuredPython ?? "auto-detect"})...`);
  const result = await requestDebugSession(lspClient, configuredPython);
  Logger.info(`LSP spawned debugpy on ${result.host}:${result.port} (session: ${result.sessionId})`);

  const proxy = new DapTcpProxy(result.host, result.port);
  const proxyPort = await proxy.start();
  Logger.info(`[Basilisk Debug] launch proxy listening on port ${proxyPort}`);
  return new vscode.DebugAdapterServer(proxyPort);
}

/** Create a debug adapter factory bound to the given LSP client accessor. */
export function createDebugAdapterFactory(
  getClient: () => LanguageClient | undefined
): vscode.DebugAdapterDescriptorFactory {
  return {
    async createDebugAdapterDescriptor(
      session: vscode.DebugSession
    ): Promise<vscode.DebugAdapterDescriptor> {
      const config = session.configuration;
      Logger.info(
        `[Basilisk Debug] createDebugAdapterDescriptor called — ` +
        `type=${config.type}, request=${config.request}, ` +
        `program=${config.program ?? "(none)"}`
      );

      if (config.request === "attach" && config.connect !== undefined && config.connect !== null) {
        return handleAttachMode(config, getClient());
      }

      const lspClient = getClient();
      if (!lspClient) {
        throw new Error("Basilisk: LSP client is not running. Cannot start debug session.");
      }
      return handleLaunchMode(config, lspClient);
    },
  };
}

// ── Debug configuration provider ──────────────────────────────────────────

/** A config field is "blank" when undefined (VS Code's empty `{}`) or empty. */
function isBlank(value: string | undefined): boolean {
  return value === undefined || value === "";
}

/** The default launch config for the current file. */
function defaultLaunchConfig(): vscode.DebugConfiguration {
  return {
    name: "Python: Current File (Basilisk)",
    type: "basilisk-debug",
    request: "launch",
    program: "${file}",
    console: "internalConsole",
    redirectOutput: true,
    justMyCode: true,
  };
}

/**
 * Fill in a runnable `basilisk-debug` config from an empty or partial one.
 *
 * This is what makes "Run and Debug" / F5 work **without a launch.json**: VS
 * Code calls the provider with an empty config (no type), and for a Python file
 * we synthesize a launch of the current file. A partial config missing
 * `program` defaults to `${file}`. Pure (no VS Code APIs) so it is unit-testable;
 * the active language id is passed in.
 */
export function applyDebugConfigDefaults(
  config: vscode.DebugConfiguration,
  activeLanguageId: string | undefined,
): vscode.DebugConfiguration {
  // Empty config (F5 / "Run and Debug" with no launch.json — VS Code passes `{}`):
  // only synthesize one for a Python file, else leave it for VS Code to report
  // "open a file". Falsy check also tolerates blank fields from a stub config.
  if (isBlank(config.type) && isBlank(config.request) && isBlank(config.name)) {
    return activeLanguageId === "python" ? defaultLaunchConfig() : config;
  }
  // A launch config missing `program` targets the active file.
  if (
    config.type === "basilisk-debug" &&
    config.request === "launch" &&
    isBlank(config.program as string | undefined)
  ) {
    return { ...config, program: "${file}" };
  }
  return config;
}

/**
 * Provider that lets `basilisk-debug` start with no `launch.json`: it offers a
 * default configuration in the Run-and-Debug picker and resolves empty/partial
 * configs to a launch of the current file.
 */
export function createBasiliskDebugConfigProvider(): vscode.DebugConfigurationProvider {
  return {
    provideDebugConfigurations(): vscode.DebugConfiguration[] {
      return [defaultLaunchConfig()];
    },
    resolveDebugConfiguration(
      _folder: vscode.WorkspaceFolder | undefined,
      config: vscode.DebugConfiguration,
    ): vscode.DebugConfiguration {
      return applyDebugConfigDefaults(config, vscode.window.activeTextEditor?.document.languageId);
    },
  };
}
