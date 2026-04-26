/**
 * DAP proxy that sits between VS Code and debugpy.
 *
 * debugpy.adapter has quirks that this proxy smooths over:
 *
 * 1. **stepOut lands before assignment**: After `stepOut` from a called
 *    function, debugpy stops at the call-site line BEFORE the return
 *    value is assigned. This proxy injects an automatic `next` to
 *    complete the statement.
 *
 * 2. **Structural line stops**: debugpy stops on `try:` lines during
 *    stepOver. This proxy detects these stops and auto-steps past them.
 *
 * 3. **Single-connection adapter**: `debugpy.adapter --port` accepts
 *    exactly ONE TCP connection. The proxy owns that connection —
 *    VS Code talks to the proxy, never directly to debugpy.
 *
 * 4. **Attach mode resilience**: In attach mode, if debugpy doesn't
 *    respond to the `attach` request (e.g. no target process), the
 *    proxy synthesizes a success response so the session starts.
 *
 * Architecture: The proxy is a TCP server. VS Code connects to the proxy
 * via DebugAdapterServer, and the proxy connects to debugpy. This ensures
 * VS Code manages its own TCP lifecycle, giving it clean session teardown
 * (activeDebugSession is cleared before onDidTerminateDebugSession fires).
 */

import * as net from "net";
import * as fs from "fs";
import { Logger } from "./logger";
import { WAIT_MS } from "./timeouts";

/** Minimal shape of a DAP message for type narrowing. */
interface DapMessage {
  type: string;
  seq?: number;
  request_seq?: number;
  command?: string;
  event?: string;
  success?: boolean;
  body?: unknown;
  arguments?: Record<string, unknown>;
}

/**
 * Lines matching this pattern are structural — debugpy stops on them but
 * no meaningful user code executes. Only `try:` is skipped; `except` lines
 * ARE useful stops because they indicate exception handling flow and the
 * test suite counts them as part of the stepping sequence.
 */
/** Length of the `\r\n\r\n` separator between DAP header and body. */
const DAP_HEADER_SEPARATOR_LEN = 4;

const STRUCTURAL_LINE_RE = /^\s*(try\s*:)\s*(#.*)?$/;

/**
 * TCP-based DAP proxy. Listens on a local port for VS Code to connect,
 * and relays messages to/from a debugpy TCP server with quirk fixes.
 *
 * Usage:
 *   const proxy = new DapTcpProxy(debugpyHost, debugpyPort);
 *   const proxyPort = await proxy.start();
 *   return new vscode.DebugAdapterServer(proxyPort);
 */
export class DapTcpProxy {
  private server: net.Server | undefined;
  private clientSocket: net.Socket | undefined;
  private debugpySocket: net.Socket | undefined;
  private clientBuffer = Buffer.alloc(0);
  private debugpyBuffer = Buffer.alloc(0);

  private pendingStepOutSeq: number | undefined;
  private awaitingStepOutStop = false;
  private stepOutThreadId: number | undefined;
  /** Sequence number base for injected DAP requests — must not collide with VS Code's seqs. */
  private static readonly INJECTED_SEQ_BASE = 900_000;
  private injectedSeq = DapTcpProxy.INJECTED_SEQ_BASE;

  /** Track pending next (stepOver) requests from VS Code for structural line skipping. */
  private pendingNextSeq: number | undefined;
  private awaitingNextStop = false;
  private nextThreadId: number | undefined;

  /** Cache of source file lines for structural line detection. */
  private readonly sourceCache = new Map<string, string[]>();

  /** Track whether we're in attach mode for special handling. */
  private isAttachMode = false;

  /** Track pending attach request for timeout-based response injection. */
  private pendingAttachSeq: number | undefined;
  private attachResponseTimer: ReturnType<typeof setTimeout> | undefined;

  /** Track whether we've seen an exited event before terminated. */
  private sawExitedEvent = false;

  /** Track pending injected stackTrace requests. */
  private pendingStackTraceSeq: number | undefined;
  private pendingStoppedMsg: DapMessage | undefined;
  private sawTerminatedEvent = false;

  /** Track whether we forwarded disconnect to debugpy after already responding to VS Code. */
  private sawDisconnectForwarded = false;

  constructor(
    private readonly debugpyHost: string,
    private readonly debugpyPort: number,
  ) {}

  /**
   * Start the proxy: connect to debugpy and listen on a random local port.
   * Returns the port number VS Code should connect to.
   */
  public async start(): Promise<number> {
    // First connect to debugpy
    await this.connectToDebugpy();

    // Then start our server for VS Code to connect to
    return this.startServer();
  }

  private async connectToDebugpy(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.debugpySocket = net.createConnection(this.debugpyPort, this.debugpyHost, () => {
        Logger.info(`[DAP Proxy] connected to debugpy at ${this.debugpyHost}:${this.debugpyPort}`);
        resolve();
      });
      this.debugpySocket.on("data", (chunk) => { this.onDebugpyData(chunk); });
      this.debugpySocket.on("error", (err) => {
        Logger.error(`[DAP Proxy] debugpy socket error: ${err.message}`);
        reject(err);
      });
      this.debugpySocket.on("close", () => {
        Logger.info("[DAP Proxy] debugpy socket closed");
      });
    });
  }

  private async startServer(): Promise<number> {
    return new Promise((resolve, reject) => {
      this.server = net.createServer((socket) => {
        Logger.info("[DAP Proxy] VS Code connected to proxy");
        this.clientSocket = socket;
        socket.on("data", (chunk) => { this.onClientData(chunk); });
        socket.on("error", (err) => {
          Logger.error(`[DAP Proxy] client socket error: ${err.message}`);
        });
        socket.on("close", () => {
          Logger.info("[DAP Proxy] client socket closed");
        });
      });
      this.server.listen(0, "127.0.0.1", () => {
        const addr = this.server?.address();
        if (addr !== undefined && addr !== null && typeof addr !== "string") {
          Logger.info(`[DAP Proxy] listening on port ${addr.port}`);
          resolve(addr.port);
        } else {
          reject(new Error("Failed to get proxy server address"));
        }
      });
      this.server.on("error", (err) => {
        Logger.error(`[DAP Proxy] server error: ${err.message}`);
        reject(err);
      });
    });
  }

  public dispose(): void {
    Logger.info("[DAP Proxy] disposing");
    if (this.attachResponseTimer) {
      clearTimeout(this.attachResponseTimer);
    }
    this.clientSocket?.destroy();
    this.debugpySocket?.destroy();
    this.server?.close();
  }

  // ── Message framing: client → proxy ──────────────────────────────────

  private onClientData(chunk: Buffer): void {
    this.clientBuffer = Buffer.concat([this.clientBuffer, chunk]);
    for (;;) {
      const headerEnd = this.clientBuffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) {break;}
      const headerStr = this.clientBuffer.subarray(0, headerEnd).toString("utf-8");
      const match = /Content-Length:\s*(\d+)/i.exec(headerStr);
      if (!match) {
        this.clientBuffer = this.clientBuffer.subarray(1);
        continue;
      }
      const bodyLen = parseInt(match[1], 10);
      const bodyStart = headerEnd + DAP_HEADER_SEPARATOR_LEN;
      if (this.clientBuffer.length < bodyStart + bodyLen) {break;}
      const body = this.clientBuffer.subarray(bodyStart, bodyStart + bodyLen).toString("utf-8");
      this.clientBuffer = this.clientBuffer.subarray(bodyStart + bodyLen);

      let msg: DapMessage;
      try { msg = JSON.parse(body) as DapMessage; } catch { continue; }
      this.processFromClient(msg);
    }
  }

  // ── Message framing: debugpy → proxy ─────────────────────────────────

  private onDebugpyData(chunk: Buffer): void {
    this.debugpyBuffer = Buffer.concat([this.debugpyBuffer, chunk]);
    for (;;) {
      const headerEnd = this.debugpyBuffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) {break;}
      const headerStr = this.debugpyBuffer.subarray(0, headerEnd).toString("utf-8");
      const match = /Content-Length:\s*(\d+)/i.exec(headerStr);
      if (!match) {
        this.debugpyBuffer = this.debugpyBuffer.subarray(1);
        continue;
      }
      const bodyLen = parseInt(match[1], 10);
      const bodyStart = headerEnd + DAP_HEADER_SEPARATOR_LEN;
      if (this.debugpyBuffer.length < bodyStart + bodyLen) {break;}
      const body = this.debugpyBuffer.subarray(bodyStart, bodyStart + bodyLen).toString("utf-8");
      this.debugpyBuffer = this.debugpyBuffer.subarray(bodyStart + bodyLen);

      let msg: DapMessage;
      try { msg = JSON.parse(body) as DapMessage; } catch { continue; }
      this.processFromDebugpy(msg);
    }
  }

  // ── Send helpers ─────────────────────────────────────────────────────

  private sendToDebugpy(msg: DapMessage): void {
    if (!this.debugpySocket || this.debugpySocket.destroyed) {return;}
    const json = JSON.stringify(msg);
    const header = `Content-Length: ${Buffer.byteLength(json, "utf-8")}\r\n\r\n`;
    this.debugpySocket.write(header + json);
  }

  private sendToClient(msg: DapMessage): void {
    if (!this.clientSocket || this.clientSocket.destroyed) {return;}
    const json = JSON.stringify(msg);
    const header = `Content-Length: ${Buffer.byteLength(json, "utf-8")}\r\n\r\n`;
    this.clientSocket.write(header + json);
  }

  // ── Process messages from VS Code (client → debugpy) ─────────────────

  private processFromClient(msg: DapMessage): void {
    if (msg.type === "request") {
      Logger.debug(`[DAP Proxy] client → debugpy: ${msg.command} seq=${msg.seq}`);
    }

    // After terminated event, respond to disconnect immediately without
    // round-tripping to debugpy (which may already be dead).
    // After responding, close the client socket so VS Code's adapter-exit
    // path runs, which clears activeDebugSession synchronously.
    if (msg.type === "request" && msg.command === "disconnect" && this.sawTerminatedEvent) {
      Logger.info("[DAP Proxy] fast disconnect response (post-termination)");
      this.sendToClient({
        type: "response",
        command: "disconnect",
        request_seq: msg.seq,
        seq: 0,
        success: true,
        body: {},
      });
      Logger.info("[DAP Proxy] disconnect response sent to client, forwarding to debugpy");
      // Also forward to debugpy for cleanup, swallowing its duplicate response.
      this.sawDisconnectForwarded = true;
      this.sendToDebugpy(msg);
      return;
    }

    if (msg.type === "request" && msg.command === "stepOut") {
      this.pendingStepOutSeq = msg.seq;
      this.stepOutThreadId = msg.arguments?.threadId as number | undefined;
      this.awaitingStepOutStop = false;
    }

    if (msg.type === "request" && msg.command === "next") {
      this.pendingNextSeq = msg.seq;
      this.nextThreadId = msg.arguments?.threadId as number | undefined;
      this.awaitingNextStop = false;
      Logger.debug(`[DAP Proxy] outgoing next seq=${msg.seq}`);
    }

    // Detect attach mode.
    if (msg.type === "request" && msg.command === "attach") {
      this.isAttachMode = true;
      this.pendingAttachSeq = msg.seq;
      // Set a timeout: if debugpy doesn't respond within 3s, fake a response.
      this.attachResponseTimer = setTimeout(() => {
        if (this.pendingAttachSeq !== undefined) {
          Logger.warn("[DAP Proxy] attach response timeout — injecting success");
          this.sendToClient({
            type: "response",
            command: "attach",
            request_seq: this.pendingAttachSeq,
            seq: 0,
            success: true,
            body: {},
          });
          this.pendingAttachSeq = undefined;
        }
      }, WAIT_MS);
    }

    this.sendToDebugpy(msg);
  }

  // ── Process messages from debugpy (debugpy → client) ─────────────────

  private processFromDebugpy(msg: DapMessage): void {
    this.handleStepOutResponse(msg);

    if (this.handleStepOutStop(msg)) {return;}
    if (this.handleNextResponse(msg)) {return;}
    if (this.handleStoppedAfterNext(msg)) {return;}
    if (this.handleStructuralLineCheck(msg)) {return;}
    if (this.handleSwallowedResponses(msg)) {return;}

    this.handleAttachResponse(msg);
    if (this.handleTerminationEvents(msg)) {return;}

    this.sendToClient(msg);
  }

  // ── stepOut auto-next ──────────────────────────────────────────────

  /** Arm auto-next when stepOut response arrives. */
  private handleStepOutResponse(msg: DapMessage): void {
    if (
      msg.type === "response" &&
      msg.command === "stepOut" &&
      msg.request_seq === this.pendingStepOutSeq &&
      msg.success
    ) {
      this.awaitingStepOutStop = true;
      Logger.debug("[DAP Proxy] stepOut ok — arming auto-next");
    }
  }

  /** After stepOut, inject an extra `next` to complete the assignment. */
  private handleStepOutStop(msg: DapMessage): boolean {
    if (msg.type !== "event" || msg.event !== "stopped" || !this.awaitingStepOutStop) {
      return false;
    }
    this.awaitingStepOutStop = false;
    const tid = (msg.body as { threadId?: number })?.threadId ?? this.stepOutThreadId;
    Logger.info(`[DAP Proxy] injecting next after stepOut (thread ${tid})`);
    this.injectedSeq++;
    this.sendToDebugpy({
      type: "request",
      command: "next",
      seq: this.injectedSeq,
      arguments: { threadId: tid },
    });
    return true;
  }

  // ── stepOver structural line skipping ──────────────────────────────

  /** Arm structural line check when next response arrives. Returns true if stopped event was logged. */
  private handleNextResponse(msg: DapMessage): boolean {
    if (msg.type === "response" && msg.command === "next" && msg.success) {
      Logger.debug(`[DAP Proxy] next response: request_seq=${msg.request_seq}, pending=${this.pendingNextSeq}`);
      if (msg.request_seq === this.pendingNextSeq) {
        this.awaitingNextStop = true;
        Logger.debug("[DAP Proxy] next ok — arming structural line check");
      }
    }
    if (msg.type === "event" && msg.event === "stopped") {
      Logger.debug(`[DAP Proxy] stopped: awaitNext=${this.awaitingNextStop}, awaitStepOut=${this.awaitingStepOutStop}`);
    }
    return false;
  }

  /** When stopped after stepOver, hold the event and request stackTrace. */
  private handleStoppedAfterNext(msg: DapMessage): boolean {
    if (msg.type !== "event" || msg.event !== "stopped" || !this.awaitingNextStop) {
      return false;
    }
    this.awaitingNextStop = false;
    const tid = (msg.body as { threadId?: number })?.threadId ?? this.nextThreadId;
    this.pendingStoppedMsg = msg;
    this.injectedSeq++;
    this.pendingStackTraceSeq = this.injectedSeq;
    Logger.debug(`[DAP Proxy] holding stopped, requesting stackTrace seq=${this.injectedSeq} thread=${tid}`);
    this.sendToDebugpy({
      type: "request",
      command: "stackTrace",
      seq: this.injectedSeq,
      arguments: { threadId: tid, startFrame: 0, levels: 1 },
    });
    return true;
  }

  /** Handle stackTrace response: skip structural lines or forward the stopped event. */
  private handleStructuralLineCheck(msg: DapMessage): boolean {
    if (msg.type === "response" && msg.command === "stackTrace") {
      Logger.debug(`[DAP Proxy] stackTrace response: req=${msg.request_seq}, pending=${this.pendingStackTraceSeq}`);
    }
    if (
      msg.type !== "response" ||
      msg.command !== "stackTrace" ||
      msg.request_seq !== this.pendingStackTraceSeq
    ) {
      return false;
    }

    this.pendingStackTraceSeq = undefined;
    const stoppedMsg = this.pendingStoppedMsg;
    this.pendingStoppedMsg = undefined;

    if (stoppedMsg && msg.success && this.trySkipStructuralLine(msg, stoppedMsg)) {
      return true;
    }

    if (stoppedMsg) {
      this.sendToClient(stoppedMsg);
    }
    return true; // always consume the stackTrace response
  }

  /** Check if the top frame is a structural line and inject a skip if so. */
  private trySkipStructuralLine(stackMsg: DapMessage, stoppedMsg: DapMessage): boolean {
    const frames = (stackMsg.body as { stackFrames?: { line?: number; source?: { path?: string } }[] })?.stackFrames;
    if (!frames || frames.length === 0) {return false;}

    const line = frames[0].line;
    const filePath = frames[0].source?.path;
    if (line === undefined || filePath === undefined || filePath === "") {return false;}
    if (!this.isStructuralLine(filePath, line)) {return false;}

    const tid = (stoppedMsg.body as { threadId?: number })?.threadId ?? this.nextThreadId;
    Logger.info(`[DAP Proxy] skipping structural line ${line} in ${filePath.split("/").pop()}`);
    this.awaitingNextStop = true;
    this.injectedSeq++;
    this.pendingNextSeq = this.injectedSeq;
    this.sendToDebugpy({
      type: "request",
      command: "next",
      seq: this.injectedSeq,
      arguments: { threadId: tid },
    });
    return true;
  }

  // ── Response swallowing ────────────────────────────────────────────

  /** Swallow injected next responses and duplicate disconnect responses. */
  private handleSwallowedResponses(msg: DapMessage): boolean {
    if (msg.type === "response" && msg.command === "next" && msg.request_seq !== undefined && msg.request_seq >= DapTcpProxy.INJECTED_SEQ_BASE) {
      Logger.debug("[DAP Proxy] swallowed injected next response");
      return true;
    }
    if (msg.type === "response" && msg.command === "disconnect" && this.sawDisconnectForwarded) {
      Logger.debug("[DAP Proxy] swallowed duplicate disconnect response");
      this.sawDisconnectForwarded = false;
      return true;
    }
    return false;
  }

  // ── Attach and termination handling ────────────────────────────────

  /** Clear attach timeout when debugpy responds. */
  private handleAttachResponse(msg: DapMessage): void {
    if (
      msg.type === "response" &&
      msg.command === "attach" &&
      msg.request_seq === this.pendingAttachSeq
    ) {
      if (this.attachResponseTimer) {
        clearTimeout(this.attachResponseTimer);
        this.attachResponseTimer = undefined;
      }
      this.pendingAttachSeq = undefined;
    }
  }

  /** Handle exited, thread, and terminated events. Returns true if consumed. */
  private handleTerminationEvents(msg: DapMessage): boolean {
    if (msg.type !== "event") {return false;}

    if (msg.event === "exited") {
      this.sawExitedEvent = true;
      Logger.info(`[DAP Proxy] exited, code=${(msg.body as { exitCode?: number })?.exitCode}`);
    }

    if (msg.event === "thread") {
      const body = msg.body as { reason?: string; threadId?: number };
      Logger.info(`[DAP Proxy] thread: reason=${body.reason}, id=${body.threadId}`);
    }

    if (msg.event === "terminated") {
      Logger.info(`[DAP Proxy] terminated, sawExited=${this.sawExitedEvent}`);
      if (!this.sawExitedEvent) {
        Logger.info("[DAP Proxy] injecting exited event before terminated");
        this.sendToClient({ type: "event", event: "exited", seq: 0, body: { exitCode: 0 } });
      }
      this.sawTerminatedEvent = true;
      this.sendToClient(msg);
      return true;
    }

    return false;
  }

  /**
   * Check if a line in a source file is a structural line (try:)
   * that debugpy stops on but doesn't execute meaningful code.
   */
  private isStructuralLine(filePath: string, lineNumber: number): boolean {
    let lines = this.sourceCache.get(filePath);
    if (!lines) {
      try {
        const content = fs.readFileSync(filePath, "utf-8");
        lines = content.split("\n");
        this.sourceCache.set(filePath, lines);
      } catch {
        return false;
      }
    }
    const idx = lineNumber - 1; // DAP lines are 1-based
    if (idx < 0 || idx >= lines.length) {return false;}
    return STRUCTURAL_LINE_RE.test(lines[idx]);
  }
}
