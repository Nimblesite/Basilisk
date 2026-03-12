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
 * 2. **Structural line stops**: debugpy stops on `try:`, `except:`,
 *    `finally:`, and `else:` lines during stepOver. This proxy detects
 *    these stops and auto-steps past them.
 *
 * 3. **Single-connection adapter**: `debugpy.adapter --port` accepts
 *    exactly ONE TCP connection. The proxy owns that connection —
 *    VS Code talks to the proxy, never directly to debugpy.
 *
 * 4. **Attach mode resilience**: In attach mode, if debugpy doesn't
 *    respond to the `attach` request (e.g. no target process), the
 *    proxy synthesizes a success response so the session starts.
 */

import * as vscode from "vscode";
import * as net from "net";
import * as fs from "fs";
import { logger } from "./logger";

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

/** Lines matching these patterns are structural — debugpy stops on them but no work happens. */
const STRUCTURAL_LINE_RE = /^\s*(try\s*:|except(\s.*)?:|finally\s*:|else\s*:)\s*(#.*)?$/;

/**
 * Message-based DAP proxy. Connects to a debugpy TCP server and relays
 * messages bidirectionally, with the ability to inject extra requests.
 */
export class DebugAdapterProxy implements vscode.DebugAdapter {
  private readonly emitter = new vscode.EventEmitter<vscode.DebugProtocolMessage>();
  readonly onDidSendMessage = this.emitter.event;

  private socket: net.Socket | undefined;
  private buffer = Buffer.alloc(0);

  private pendingStepOutSeq: number | undefined;
  private awaitingStepOutStop = false;
  private stepOutThreadId: number | undefined;
  private injectedSeq = 900_000;

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

  constructor(
    private readonly host: string,
    private readonly port: number,
  ) {}

  handleMessage(message: vscode.DebugProtocolMessage): void {
    const msg = message as unknown as DapMessage;

    if (msg.type === "request" && msg.command === "stepOut") {
      this.pendingStepOutSeq = msg.seq;
      this.stepOutThreadId = msg.arguments?.threadId as number | undefined;
      this.awaitingStepOutStop = false;
    }

    if (msg.type === "request" && msg.command === "next") {
      this.pendingNextSeq = msg.seq;
      this.nextThreadId = msg.arguments?.threadId as number | undefined;
      this.awaitingNextStop = false;
      logger.debug(`[DAP Proxy] outgoing next seq=${msg.seq}`);
    }

    // Detect attach mode.
    if (msg.type === "request" && msg.command === "attach") {
      this.isAttachMode = true;
      this.pendingAttachSeq = msg.seq;
      // Set a timeout: if debugpy doesn't respond within 3s, fake a response.
      this.attachResponseTimer = setTimeout(() => {
        if (this.pendingAttachSeq !== undefined) {
          logger.warn("[DAP Proxy] attach response timeout — injecting success");
          this.emitter.fire({
            type: "response",
            command: "attach",
            request_seq: this.pendingAttachSeq,
            seq: 0,
            success: true,
            body: {},
          } as unknown as vscode.DebugProtocolMessage);
          this.pendingAttachSeq = undefined;
        }
      }, 3000);
    }

    this.sendRaw(message);
  }

  async start(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.socket = net.createConnection(this.port, this.host, () => {
        logger.info(`[DAP Proxy] connected to ${this.host}:${this.port}`);
        resolve();
      });
      this.socket.on("data", (chunk) => this.onData(chunk));
      this.socket.on("error", (err) => {
        logger.error(`[DAP Proxy] socket error: ${err.message}`);
        reject(err);
      });
      this.socket.on("close", () => {
        logger.info("[DAP Proxy] socket closed");
      });
    });
  }

  dispose(): void {
    if (this.attachResponseTimer) {
      clearTimeout(this.attachResponseTimer);
    }
    this.socket?.destroy();
    this.emitter.dispose();
  }

  private sendRaw(message: vscode.DebugProtocolMessage): void {
    if (!this.socket || this.socket.destroyed) return;
    const json = JSON.stringify(message);
    const header = `Content-Length: ${Buffer.byteLength(json, "utf-8")}\r\n\r\n`;
    this.socket.write(header + json);
  }

  private onData(chunk: Buffer): void {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    for (;;) {
      const headerEnd = this.buffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) break;
      const headerStr = this.buffer.subarray(0, headerEnd).toString("utf-8");
      const match = headerStr.match(/Content-Length:\s*(\d+)/i);
      if (!match) {
        this.buffer = this.buffer.subarray(1);
        continue;
      }
      const bodyLen = parseInt(match[1], 10);
      const bodyStart = headerEnd + 4;
      if (this.buffer.length < bodyStart + bodyLen) break;
      const body = this.buffer.subarray(bodyStart, bodyStart + bodyLen).toString("utf-8");
      this.buffer = this.buffer.subarray(bodyStart + bodyLen);

      let msg: DapMessage;
      try { msg = JSON.parse(body) as DapMessage; } catch { continue; }
      this.processIncoming(msg);
    }
  }

  private processIncoming(msg: DapMessage): void {
    // ── stepOut auto-next ──────────────────────────────────────────────
    // Arm auto-next when stepOut response arrives.
    if (
      msg.type === "response" &&
      msg.command === "stepOut" &&
      msg.request_seq === this.pendingStepOutSeq &&
      msg.success
    ) {
      this.awaitingStepOutStop = true;
      logger.debug("[DAP Proxy] stepOut ok — arming auto-next");
    }

    // After stepOut, debugpy stops at the call-site BEFORE the assignment.
    // Inject an extra `next` to complete it, swallowing this intermediate stop.
    if (msg.type === "event" && msg.event === "stopped" && this.awaitingStepOutStop) {
      this.awaitingStepOutStop = false;
      const tid = (msg.body as { threadId?: number })?.threadId ?? this.stepOutThreadId;
      logger.info(`[DAP Proxy] injecting next after stepOut (thread ${tid})`);
      this.injectedSeq++;
      this.sendRaw({
        type: "request",
        command: "next",
        seq: this.injectedSeq,
        arguments: { threadId: tid },
      } as unknown as vscode.DebugProtocolMessage);
      return; // swallow this stopped event
    }

    // ── stepOver structural line skipping ──────────────────────────────
    // Arm structural-line check when next response arrives.
    if (
      msg.type === "response" &&
      msg.command === "next" &&
      msg.success
    ) {
      logger.debug(`[DAP Proxy] next response: request_seq=${msg.request_seq}, pendingNextSeq=${this.pendingNextSeq}, match=${msg.request_seq === this.pendingNextSeq}`);
      if (msg.request_seq === this.pendingNextSeq) {
        this.awaitingNextStop = true;
        logger.debug("[DAP Proxy] next ok — arming structural line check");
      }
    }

    // When stopped after a stepOver, check if we're on a structural line.
    if (msg.type === "event" && msg.event === "stopped") {
      logger.debug(`[DAP Proxy] stopped event: awaitingNextStop=${this.awaitingNextStop}, awaitingStepOutStop=${this.awaitingStepOutStop}`);
    }
    if (msg.type === "event" && msg.event === "stopped" && this.awaitingNextStop) {
      this.awaitingNextStop = false;
      const tid = (msg.body as { threadId?: number })?.threadId ?? this.nextThreadId;
      // Request stack trace to check the current line.
      this.pendingStoppedMsg = msg;
      this.injectedSeq++;
      this.pendingStackTraceSeq = this.injectedSeq;
      logger.debug(`[DAP Proxy] holding stopped event, requesting stackTrace seq=${this.injectedSeq} thread=${tid}`);
      this.sendRaw({
        type: "request",
        command: "stackTrace",
        seq: this.injectedSeq,
        arguments: { threadId: tid, startFrame: 0, levels: 1 },
      } as unknown as vscode.DebugProtocolMessage);
      return; // hold the stopped event until we check the line
    }

    // Handle response to our injected stackTrace.
    if (msg.type === "response" && msg.command === "stackTrace") {
      logger.debug(`[DAP Proxy] stackTrace response: request_seq=${msg.request_seq}, pendingStackTraceSeq=${this.pendingStackTraceSeq}, match=${msg.request_seq === this.pendingStackTraceSeq}`);
    }
    if (
      msg.type === "response" &&
      msg.command === "stackTrace" &&
      msg.request_seq === this.pendingStackTraceSeq
    ) {
      this.pendingStackTraceSeq = undefined;
      const stoppedMsg = this.pendingStoppedMsg;
      this.pendingStoppedMsg = undefined;

      if (stoppedMsg && msg.success) {
        const frames = (msg.body as { stackFrames?: Array<{ line?: number; source?: { path?: string } }> })?.stackFrames;
        if (frames && frames.length > 0) {
          const line = frames[0].line;
          const filePath = frames[0].source?.path;
          if (line !== undefined && filePath && this.isStructuralLine(filePath, line)) {
            const tid = (stoppedMsg.body as { threadId?: number })?.threadId ?? this.nextThreadId;
            logger.info(`[DAP Proxy] skipping structural line ${line} in ${filePath.split("/").pop()}`);
            // Re-arm for the next stop and inject another next.
            this.awaitingNextStop = true;
            this.injectedSeq++;
            this.pendingNextSeq = this.injectedSeq;
            this.sendRaw({
              type: "request",
              command: "next",
              seq: this.injectedSeq,
              arguments: { threadId: tid },
            } as unknown as vscode.DebugProtocolMessage);
            return; // swallow this stopped event + stackTrace response
          }
        }
      }

      // Not a structural line — forward the original stopped event.
      if (stoppedMsg) {
        this.emitter.fire(stoppedMsg as unknown as vscode.DebugProtocolMessage);
      }
      return; // don't forward the stackTrace response itself
    }

    // Swallow the response to our injected next (from both stepOut and structural skip).
    if (msg.type === "response" && msg.command === "next" && msg.request_seq !== undefined && msg.request_seq >= 900_000) {
      logger.debug("[DAP Proxy] swallowed injected next response");
      // If this was from structural skip, the awaitingNextStop is already re-armed.
      return;
    }

    // ── attach mode handling ──────────────────────────────────────────
    // Clear the attach timeout if we get a real response.
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

    // ── terminated event handling ─────────────────────────────────────
    // Track exited event.
    if (msg.type === "event" && msg.event === "exited") {
      this.sawExitedEvent = true;
    }

    // When terminated arrives, inject exited if missing, then forward
    // terminated after a short delay so VS Code can clear activeDebugSession.
    if (msg.type === "event" && msg.event === "terminated") {
      if (!this.sawExitedEvent) {
        logger.debug("[DAP Proxy] injecting exited event before terminated");
        this.emitter.fire({
          type: "event",
          event: "exited",
          seq: 0,
          body: { exitCode: 0 },
        } as unknown as vscode.DebugProtocolMessage);
      }
      // Delay the terminated event slightly to let VS Code process exited first.
      setTimeout(() => {
        this.emitter.fire(msg as unknown as vscode.DebugProtocolMessage);
      }, 50);
      return;
    }

    // Everything else → VS Code.
    this.emitter.fire(msg as unknown as vscode.DebugProtocolMessage);
  }

  /**
   * Check if a line in a source file is a structural line (try/except/finally/else)
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
    if (idx < 0 || idx >= lines.length) return false;
    return STRUCTURAL_LINE_RE.test(lines[idx]);
  }
}
