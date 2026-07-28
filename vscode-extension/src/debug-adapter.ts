// Implements [VSIX-PYTHON-DEBUGGER-DAP]. See docs/specs/VSIX-SPEC.md#VSIX-PYTHON-DEBUGGER-DAP
/**
 * Debug adapter factory, DAP tracker, and logging utilities for Basilisk.
 */

import { asRecord, booleanField, isRecord, numberField, rawField, recordArrayField, recordField, stringField } from "./unknown-shape";
import * as vscode from "vscode";
import * as net from "net";
import { type LanguageClient } from "vscode-languageclient/node";
import { Logger } from "./logger";
import { DapTcpProxy } from "./dap-proxy";
import type { Result } from "./result";
import {
  appendDebugOutput,
  clearDebugOutput,
  trackResumeRequest,
  trackResumeResponse,
  trackSuspensionEvent,
} from "./dap-output";

/** Max number of variables to log inline before switching to a count summary. */
const MAX_INLINE_VARS = 10;

/** Length of an abbreviated session ID prefix. */
const SESSION_ID_PREFIX_LEN = 8;

// ── DAP message summarization ─────────────────────────────────────────────

/** Compact summary of DAP request arguments for logging. */
export function summarizeArgs(args: unknown): string {
  if (!isRecord(args)) {return "";}
  const obj = args;
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
    const bps = recordArrayField(obj, "breakpoints");
    parts.push(`bps=[${bps.map((b) => numberField(b, "line")).join(",")}]`);
  }
  if ("source" in obj) {
    const path = stringField(recordField(obj, "source"), "path");
    if (path !== undefined && path !== "") {parts.push(`src=${path.split("/").pop()}`);}
  }
}

/** Compact summary of DAP response/event body for logging. */
export function summarizeBody(body: unknown): string {
  if (!isRecord(body)) {return "";}
  const obj = body;
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
    const frames = recordArrayField(obj, "stackFrames");
    if (frames.length > 0) {
      parts.push(`frames=[${frames.map((f) => `${String(stringField(f, "name"))}:${String(numberField(f, "line"))}`).join(", ")}]`);
    }
  }
  if ("scopes" in obj) {
    const scopes = recordArrayField(obj, "scopes");
    parts.push(`scopes=[${scopes.map((sc) => String(stringField(sc, "name"))).join(", ")}]`);
  }
  if ("variables" in obj) {
    const vars = recordArrayField(obj, "variables");
    if (vars.length <= MAX_INLINE_VARS) {
      parts.push(`vars=[${vars.map((v) => `${String(stringField(v, "name"))}=${String(stringField(v, "value"))}`).join(", ")}]`);
    } else {
      parts.push(`vars=[${vars.length} items]`);
    }
  }
  if ("threads" in obj) {
    const threads = recordArrayField(obj, "threads");
    parts.push(`threads=[${threads.map((t) => `${String(numberField(t, "id"))}:${String(stringField(t, "name"))}`).join(", ")}]`);
  }
}

// ── DAP message tracker ───────────────────────────────────────────────────

/** Callbacks the DAP tracker fires on profiler-relevant debuggee events. */
export interface DebugTrackerCallbacks {
  /** Receives `(sessionId, pid)` once debugpy emits its `process` event. */
  readonly onDebuggeeProcessId?: DebuggeeProcessIdCallback;
  /**
   * Receives `(sessionId, body)` on every `stopped` event — the memory autopilot
   * captures on pause off this signal ([PROFILE-MEMORY-AUTOPILOT-PAUSE]). Fired
   * AFTER the suspension bookkeeping is recorded, so a handler can immediately
   * resolve the stopped frame.
   */
  readonly onStopped?: (sessionId: string, body: unknown) => void;
}

// Implements [VSIX-PYTHON-DEBUGGER-DAP-TRACKER] — single observability point for
// debugpy → VS Code traffic. Captures the `process` event (systemProcessId, used
// by the CPU profiler) and `output` events (__BASILISK_MEM*__ payloads for the
// memory round-trip).
/**
 * Factory that creates per-session DAP message trackers.
 *
 * The tracker is the single observability point for debugpy → VS Code traffic,
 * so it captures the debuggee `process` event (the PID the CPU profiler targets —
 * "same process"), `output` events (the marker payloads the memory round-trip
 * recovers), and `stopped` events (suspension bookkeeping + the autopilot's
 * pause trigger). Callbacks, when supplied, route those out.
 */
export class BasiliskDebugAdapterTrackerFactory
  implements vscode.DebugAdapterTrackerFactory
{
  constructor(private readonly callbacks: DebugTrackerCallbacks = {}) {}

  public createDebugAdapterTracker(
    session: vscode.DebugSession
  ): vscode.ProviderResult<vscode.DebugAdapterTracker> {
    return new BasiliskDebugAdapterTracker(session, this.callbacks);
  }
}

class BasiliskDebugAdapterTracker implements vscode.DebugAdapterTracker {
  private readonly sessionId: string;
  private readonly fullSessionId: string;
  private readonly sessionName: string;

  constructor(
    session: vscode.DebugSession,
    private readonly callbacks: DebugTrackerCallbacks
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
    if (stringField(message, "type") === "request") {
      const command = stringField(message, "command");
      const seq = numberField(message, "seq");
      Logger.debug(`[DAP ${this.sessionId}] --> ${command} #${seq} ${summarizeArgs(rawField(message, "arguments"))}`);
      // Resume bookkeeping: a successful continue/step RESPONSE implies the
      // thread runs (the `continued` event is optional per the DAP spec), so
      // in-flight resume requests are remembered here and matched below.
      trackResumeRequest(this.fullSessionId, message);
    }
  }

  public onDidSendMessage(message: unknown): void {
    const msg = asRecord(message);
    const type = stringField(msg, "type");
    const success = booleanField(msg, "success");
    if (type === "response") {
      const command = stringField(msg, "command");
      const requestSeq = numberField(msg, "request_seq");
      const text = `[DAP ${this.sessionId}] <-- ${command} #${requestSeq} success=${success} ${summarizeBody(rawField(msg, "body"))}`;
      if (success === true) {
        Logger.debug(text);
      } else {
        Logger.warn(text);
      }
      // A successful resume response clears the stopped bookkeeping NOW —
      // waiting for the optional `continued` event leaves a stale window
      // where couriers evaluate against a sampled frame of a running thread.
      trackResumeResponse(this.fullSessionId, message);
    } else if (type === "event") {
      this.handleEvent(stringField(msg, "event"), rawField(msg, "body"));
    }
  }

  /** Capture profiler-relevant events; log the rest. */
  private handleEvent(event: string | undefined, body: unknown): void {
    if (event === "output") {
      // Capture debuggee stdout/stderr so the memory round-trip can recover
      // the `__BASILISK_MEM*__` marker its injection scripts print (debugpy
      // delivers print() output here, not in the evaluate result).
      const text = stringField(body, "output");
      if (text !== undefined) {
        appendDebugOutput(this.fullSessionId, text);
      }
      return;
    }
    if (event === "process") {
      // The debuggee's OS PID — captured so the CPU profiler can attach to the
      // SAME process the debugger drives (DAP: body.systemProcessId).
      const pid = numberField(body, "systemProcessId");
      if (pid !== undefined && this.callbacks.onDebuggeeProcessId !== undefined) {
        Logger.info(`[DAP ${this.sessionId}] debuggee systemProcessId=${pid}`);
        this.callbacks.onDebuggeeProcessId(this.fullSessionId, pid);
      }
      return;
    }
    Logger.debug(`[DAP ${this.sessionId}] <-- event:${event} ${summarizeBody(body)}`);
    if (event === "stopped" || event === "continued") {
      // Pause bookkeeping for the memory/cooperative couriers — see
      // `currentStoppedFrameId` (dap-evaluate.ts) for why this can't be probed.
      trackSuspensionEvent(this.fullSessionId, event, body);
    }
    if (event === "stopped") {
      // The memory autopilot captures on every genuine user pause
      // ([PROFILE-MEMORY-AUTOPILOT-PAUSE]). Fired after the bookkeeping above so
      // the handler can resolve the now-stopped frame straight away.
      this.callbacks.onStopped?.(this.fullSessionId, body);
    }
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

// Implements [VSIX-PYTHON-DEBUGGER-DAP-PROXY] Quirk 3 — single-connection slot
// protection: a bind-based liveness probe (EADDRINUSE = alive) is non-destructive
// (it does not consume debugpy's one TCP slot). handleAttachMode respawns debugpy
// via the LSP when the port is dead.
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

// Implements [VSIX-PYTHON-DEBUGGER-DAP-FEATURES] (Attach) + [VSIX-PYTHON-DEBUGGER-
// DAP-LAUNCH-CONFIGURATIONS] (request:"attach", connect:{host,port}) — connects to
// the user-specified debugpy host:port via the proxy, respawning debugpy through
// the LSP if the slot is dead (Quirk 3).
/** Handle attach mode: connect to user-specified host:port, respawning if needed. */
async function handleAttachMode(
  config: vscode.DebugConfiguration,
  lspClient: LanguageClient | undefined
): Promise<vscode.DebugAdapterDescriptor> {
  const connectInfo = asRecord(config.connect);
  // Falls back to the IPv4 literal, never the name `localhost`: the server
  // side binds `127.0.0.1`, and on Windows `localhost` resolves to `::1`
  // first, where nothing listens ([LSPDEBUG-START]).
  let host = stringField(connectInfo, "host") ?? "127.0.0.1";
  let port = numberField(connectInfo, "port") ?? 0;
  Logger.info(`[Basilisk Debug] Attach mode → ${host}:${port}`);

  const alive = await isPortAlive(host, port);
  if (!alive && lspClient) {
    Logger.warn(`[Basilisk Debug] Port ${port} is dead — respawning debugpy adapter`);
    try {
      const result = await lspClient.sendRequest<{ host: string; port: number } | null>(
        "workspace/executeCommand",
        {
          command: "basilisk.startDebugSession",
          arguments: [{ python: stringField(config, "python") ?? null }],
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

// Implements [VSIX-PYTHON-DEBUGGER-DAP-ARCHITECTURE] — the LSP spawns
// `debugpy.adapter --port <free>` via basilisk.startDebugSession; the proxy then
// connects to that port and is returned to VS Code as a DebugAdapterServer.
/** Handle launch mode: ask LSP to spawn debugpy. */
async function handleLaunchMode(
  config: vscode.DebugConfiguration,
  lspClient: LanguageClient
): Promise<vscode.DebugAdapterDescriptor> {
  const configuredPython =
    stringField(config, "python") ??
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

/**
 * How long a debug launch waits for the language server to come up.
 *
 * Sized for a cold start, not a warm one: on win32 spawning the server binary
 * and completing the handshake takes ~10s, and a user who opens a project and
 * immediately presses F5 is inside that window every time.
 */
const LSP_READY_FOR_DEBUG_MS = 60_000;

/**
 * Create a debug adapter factory bound to the given LSP readiness accessor.
 *
 * The accessor waits for the client to reach Running rather than handing back
 * whatever reference exists. A client that merely EXISTS may still be
 * `Starting`, and a request sent into that state is never answered and never
 * rejected — the debug session just hangs, with nothing written anywhere to
 * say why ([VSIX-CI-PLATFORM-COVERAGE-CLASSES]).
 */
export function createDebugAdapterFactory(
  ensureLspReady: (timeoutMs: number) => Promise<Result<LanguageClient>>
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

      const ready = await ensureLspReady(LSP_READY_FOR_DEBUG_MS);
      if (!ready.ok) {
        // Attach mode tolerates a missing client (it can connect to an
        // already-running debugpy), so only a launch is fatal here.
        if (config.request === "attach" && config.connect !== undefined && config.connect !== null) {
          Logger.warn(`[Basilisk Debug] attaching without a ready LSP: ${ready.error.message}`);
          return handleAttachMode(config, undefined);
        }
        Logger.error(`[Basilisk Debug] LSP not ready: ${ready.error.message}`);
        throw new Error(
          `Basilisk: the language server is not running, so the debug session cannot start ` +
          `(${ready.error.message}). Check the Basilisk output channel.`
        );
      }

      if (config.request === "attach" && config.connect !== undefined && config.connect !== null) {
        return handleAttachMode(config, ready.value);
      }
      return handleLaunchMode(config, ready.value);
    },
  };
}

// ── Debug configuration provider ──────────────────────────────────────────

/** A config field is "blank" when undefined (VS Code's empty `{}`) or empty. */
function isBlank(value: string | undefined): boolean {
  return value === undefined || value === "";
}

/**
 * VS Code's own substitution variable for the active editor's file, resolved by
 * VS Code before the config reaches the adapter. It is a literal `${file}` on
 * the wire, never a JavaScript template placeholder — hence the one disable.
 */
// eslint-disable-next-line no-template-curly-in-string -- VS Code variable syntax, not a template literal
export const ACTIVE_FILE_VARIABLE = "${file}";

// Implements [VSIX-PYTHON-DEBUGGER-DAP-LAUNCH-CONFIGURATIONS] (launch shape) —
// the zero-config "launch" configuration (type/request/program) offered in the
// Run-and-Debug picker and used to fill an empty/partial config.
/** The default launch config for the current file. */
function defaultLaunchConfig(): vscode.DebugConfiguration {
  return {
    name: "Python: Current File (Basilisk)",
    type: "basilisk-debug",
    request: "launch",
    program: ACTIVE_FILE_VARIABLE,
    console: "internalConsole",
    redirectOutput: true,
    justMyCode: true,
  };
}

/**
 * Synthesize/complete a runnable `basilisk-debug` config (program defaulting).
 *
 * This is what makes "Run and Debug" / F5 work **without a launch.json**: VS
 * Code calls the provider with an empty config (no type), and for a Python file
 * we synthesize a launch of the current file. A partial config missing
 * `program` defaults to `${file}`. Pure (no VS Code APIs).
 */
function withProgramDefaults(
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
    isBlank(stringField(config, "program"))
  ) {
    return { ...config, program: ACTIVE_FILE_VARIABLE };
  }
  return config;
}

// Implements [VSIX-PYTHON-DEBUGGER-START] — pure config defaulting for the
// factory-based `basilisk-debug` debugger: fills an empty/partial config so F5 /
// "Run and Debug" launch the active Python file with no launch.json.
/**
 * Resolve a runnable `basilisk-debug` config, defaulting `program` and marking
 * profiling runs.
 *
 * When the global `basilisk.profiler.profileOnLaunch` setting is on, every
 * CPU-profilable basilisk-debug launch is a profiling run, so it is marked
 * `profileOnLaunch: true`. That flag makes the DAP proxy neutralise the user's
 * breakpoints so the run completes instead of stopping interactively
 * ([PROFILE-LAUNCH-NOSTOP], #145) — matching `shouldProfileOnLaunch`'s two
 * equivalent triggers (the explicit launch arg, or this global setting).
 *
 * A "Run & Track Memory" launch (`memoryTrackOnLaunch`) is explicitly excluded:
 * it is not a CPU run, and stamping it would (a) strip its breakpoints and
 * (b) make the CPU sampler auto-start alongside tracemalloc, the two fighting
 * over the single entry pause (dap-1). Pure (the setting is passed in) so it
 * stays unit-testable; the active language id is passed in too.
 */
export function applyDebugConfigDefaults(
  config: vscode.DebugConfiguration,
  activeLanguageId: string | undefined,
  profileOnLaunchGlobal = false,
): vscode.DebugConfiguration {
  const resolved = withProgramDefaults(config, activeLanguageId);
  if (
    profileOnLaunchGlobal &&
    resolved.type === "basilisk-debug" &&
    resolved.request === "launch" &&
    resolved.profileOnLaunch !== true &&
    resolved.memoryTrackOnLaunch !== true
  ) {
    return { ...resolved, profileOnLaunch: true };
  }
  return resolved;
}

// Implements [VSIX-PYTHON-DEBUGGER-START] — the DebugConfigurationProvider for
// `basilisk-debug` (registered Dynamic + default in extension.ts), offering a
// "Python: Current File (Basilisk)" entry and resolving empty/partial configs.
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
      return applyDebugConfigDefaults(
        config,
        vscode.window.activeTextEditor?.document.languageId,
        vscode.workspace.getConfiguration("basilisk").get<boolean>("profiler.profileOnLaunch", false),
      );
    },
  };
}
