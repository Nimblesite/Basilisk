// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
/**
 * DAP `evaluate` bridge for memory profiling.
 *
 * The LSP holds no DAP connection — the editor owns it — so memory profiling is
 * a courier round-trip: the LSP hands us a Python injection script, we run it in
 * the debuggee via DAP `evaluate`, and post the raw output back to the LSP
 * (`basilisk.memory.ingest`). These are internal helpers, NOT registered
 * commands: the LSP owns commands; the editor only shuttles bytes (CLAUDE.md
 * command-ownership rule).
 *
 * debugpy can only `evaluate` against a *stopped* frame, so memory profiling
 * requires the debuggee to be paused at a breakpoint — [`currentStoppedFrameId`]
 * resolves that frame (or null when nothing is paused).
 */

import * as vscode from "vscode";
import { Logger } from "./logger";
import { debugOutputCursor, debugOutputSince } from "./dap-output";

/** The Basilisk debug adapter type. */
const DEBUG_TYPE = "basilisk-debug";

/** Prefix shared by every memory-script output marker (`__BASILISK_MEM*__`). */
const MARKER_PREFIX = "__BASILISK_MEM";
/** How long to wait for a script's (possibly large, chunked) marker output. */
const MARKER_WAIT_MS = 4000;
/** Poll interval while waiting for marker output. */
const MARKER_POLL_MS = 25;

/** Return the active Basilisk debug session, or undefined. */
function activeBasiliskSession(): vscode.DebugSession | undefined {
  const session = vscode.debug.activeDebugSession;
  return session?.type === DEBUG_TYPE ? session : undefined;
}

/**
 * Evaluate a Python expression/statement in the active Basilisk debug session
 * and return its textual output.
 *
 * Injection scripts `print()` their `__BASILISK_MEM*__` marker payloads, and
 * debugpy delivers that to DAP `output` events (the debuggee's stdout is
 * redirected) — **not** in the `evaluate` response. So we snapshot the output
 * cursor, run the evaluate, and then recover whatever the script printed (with
 * a short wait, since the `output` event can land just after the response). The
 * evaluate `result` is included too, in case an adapter does echo it. Returns
 * null when there is no active Basilisk session or the request fails.
 */
export async function evaluateInDebugSession(
  expression: string,
  frameId?: number,
  context: "repl" | "watch" | "hover" = "repl",
): Promise<string | null> {
  const session = activeBasiliskSession();
  if (session === undefined) { return null; }

  const cursor = debugOutputCursor(session.id);
  try {
    const request: Record<string, unknown> = { expression, context };
    if (frameId !== undefined) { request.frameId = frameId; }
    const response = (await session.customRequest("evaluate", request)) as { result?: string };
    const direct = response.result ?? "";
    if (direct.includes(MARKER_PREFIX)) { return direct; }
    const printed = await waitForMarkerOutput(session.id, cursor);
    return printed.length > 0 ? printed : direct;
  } catch (err: unknown) {
    Logger.warn(`[Memory] evaluate failed: ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

/**
 * Wait for printed marker output to arrive via `output` events.
 *
 * The payload is a single `print()`ed line (`marker + json.dumps(...)` + `\n`)
 * but debugpy can split it across several `output` events, so we wait until the
 * marker line is **newline-terminated** — otherwise a large JSON snapshot is
 * truncated mid-string. `json.dumps` (no indent) emits no embedded newlines, so
 * the first `\n` after the marker reliably ends the payload.
 */
async function waitForMarkerOutput(sessionId: string, cursor: number): Promise<string> {
  const deadline = Date.now() + MARKER_WAIT_MS;
  for (;;) {
    const out = debugOutputSince(sessionId, cursor);
    const markerAt = out.indexOf(MARKER_PREFIX);
    // The payload line is complete once a newline follows the marker (the
    // `print()` terminator); `includes(.., markerAt)` searches from the marker.
    const complete = markerAt !== -1 && out.includes("\n", markerAt);
    if (complete || Date.now() >= deadline) {
      return out;
    }
    await new Promise<void>((resolve) => setTimeout(resolve, MARKER_POLL_MS));
  }
}

/**
 * Resolve a frameId for a currently-stopped thread, or null if nothing is
 * paused. debugpy rejects `evaluate` without a stopped frame, so memory
 * profiling requires the debuggee to be paused at a breakpoint.
 */
export async function currentStoppedFrameId(): Promise<number | null> {
  const session = activeBasiliskSession();
  if (session === undefined) { return null; }

  try {
    const threads = (await session.customRequest("threads")) as { threads?: { id: number }[] };
    for (const thread of threads.threads ?? []) {
      const frameId = await topFrameIdIfStopped(session, thread.id);
      if (frameId !== null) { return frameId; }
    }
    return null;
  } catch (err: unknown) {
    Logger.warn(
      `[Memory] could not resolve a stopped frame: ${err instanceof Error ? err.message : String(err)}`,
    );
    return null;
  }
}

/** Top frameId of `threadId` if it is stopped, else null (running threads error). */
async function topFrameIdIfStopped(
  session: vscode.DebugSession,
  threadId: number,
): Promise<number | null> {
  try {
    const stack = (await session.customRequest("stackTrace", {
      threadId,
      startFrame: 0,
      levels: 1,
    })) as { stackFrames?: { id: number }[] };
    return stack.stackFrames?.[0]?.id ?? null;
  } catch {
    return null; // thread not suspended
  }
}
