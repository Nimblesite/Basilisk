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

import { numberField, recordArrayField, stringField } from "./unknown-shape";
import * as vscode from "vscode";
import * as fs from "fs";
import { Logger } from "./logger";
import { ALL_THREADS, debugOutputCursor, debugOutputSince, stoppedThreadIds } from "./dap-output";
import { POLL_INTERVAL_MS, STARTUP_TIMEOUT_MS, WAIT_MS, delay } from "./timeouts";
/** The Basilisk debug adapter type. */
const DEBUG_TYPE = "basilisk-debug";

/** Prefix shared by every injection-script output marker (`__BASILISK_MEM*__`,
 *  `__BASILISK_CPU_ACK__`). */
const MARKER_PREFIX = "__BASILISK_";

/** Marker a memory script prints when it hands its (large) payload back via a
 *  temp file instead of stdout — see [PROFILE-MEMORY-COURIER] and
 *  `scripts.rs::emit_via_file_helper`. The text after it is the file path. */
const FILE_PAYLOAD_MARKER = "__BASILISK_MEM_FILE__";
/**
 * How long to wait for a script's (possibly large, chunked) marker output.
 *
 * This budget is for DELIVERY, not for work: the evaluate has already returned,
 * and we are waiting on the marker line to travel debuggee stdout -> debugpy ->
 * adapter -> DAP `output` event -> extension host. The wait loop below returns
 * the instant the line is complete, so a larger ceiling costs nothing when the
 * pipeline is prompt — it only changes what happens when it is slow.
 *
 * The previous 4s was calibrated on a prompt machine, and on the win32 CI runner
 * it expired mid-delivery: the wait then returned marker-LESS output, which the
 * courier posted on to `basilisk.memory.ingest`, and the LSP rejected it with
 * "no recognized __BASILISK_MEM*__ marker in script output"
 * (profiler/memory/session.rs). That reads as a broken injection script rather
 * than as a wait that gave up, which is why the same test failed three different
 * ways across consecutive runs ([VSIX-CI-PLATFORM-COVERAGE]).
 *
 * 20s is a judgement, not a measurement — no per-platform delivery figure was
 * taken. It is bounded from both sides: comfortably above the sibling
 * `FILE_PAYLOAD_WAIT_MS` render budget's granularity, and small enough that even
 * three fully-expired legs stay inside the courier round-trip's own 90s test
 * budget. Nothing asserts how QUICKLY a marker arrives.
 */
const MARKER_WAIT_MS = 20_000;
/** Poll interval while waiting for marker output. */
const MARKER_POLL_MS = 25;
/** How long to wait for the debuggee's render worker to fill the payload file.
 *  The snapshot/diff evaluate returns as soon as the C-level capture is done
 *  (so it never stalls past pydevd's 3 s evaluation budget); the per-trace
 *  aggregation happens on a debuggee worker thread and can take a while on
 *  multi-million-trace heaps ([PROFILE-MEMORY-COURIER]). */
const FILE_PAYLOAD_WAIT_MS = 60_000;
/** Poll interval while the payload file is still an empty reservation. */
const FILE_PAYLOAD_POLL_MS = 100;

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
    const response: unknown = await session.customRequest("evaluate", request);
    const direct = stringField(response, "result") ?? "";
    if (direct.includes(MARKER_PREFIX)) { return await resolveMarkerFilePayload(direct); }
    const printed = await waitForMarkerOutput(session.id, cursor);
    return await resolveMarkerFilePayload(printed.length > 0 ? printed : direct);
  } catch (err: unknown) {
    Logger.warn(`[Memory] evaluate failed: ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

/**
 * Resolve a file-handoff payload. Large memory snapshots are written to a temp
 * file by the injection script — which prints only `__BASILISK_MEM_FILE__<path>`
 * — because debugpy truncates a single `print()` at ~20 KB
 * ([PROFILE-MEMORY-COURIER]). When that marker is present, read the file (the
 * real `__BASILISK_MEM*__ + json` payload), delete it, and return its contents.
 * Anything else (CPU acks, small OK markers) passes through untouched.
 *
 * The snapshot/diff scripts print the path while a debuggee worker thread is
 * still rendering the payload (that render is seconds of pure Python on big
 * heaps — running it inside the evaluate stalled the debugger past pydevd's
 * 3 s budget, the original bug): the reserved file exists but is EMPTY until
 * the worker atomically `os.replace`s the whole payload in. So an empty file
 * means "still rendering" and is polled; any non-empty content is complete by
 * construction. A missing file is still an immediate, honest failure.
 */
export async function resolveMarkerFilePayload(out: string): Promise<string> {
  const at = out.indexOf(FILE_PAYLOAD_MARKER);
  if (at === -1) { return out; }
  const path = out.slice(at + FILE_PAYLOAD_MARKER.length).split(/\r?\n/, 1)[0]?.trim() ?? "";
  if (path === "") { return out; }
  const deadline = Date.now() + FILE_PAYLOAD_WAIT_MS;
  for (;;) {
    let contents: string;
    try {
      contents = await fs.promises.readFile(path, "utf8");
    } catch (err: unknown) {
      Logger.warn(`[Memory] could not read payload file: ${err instanceof Error ? err.message : String(err)}`);
      return out;
    }
    if (contents.length > 0) {
      await fs.promises.unlink(path).catch(() => undefined);
      return contents;
    }
    if (Date.now() >= deadline) {
      Logger.warn(`[Memory] payload file stayed empty for ${FILE_PAYLOAD_WAIT_MS}ms: ${path}`);
      await fs.promises.unlink(path).catch(() => undefined);
      return out;
    }
    await delay(FILE_PAYLOAD_POLL_MS);
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
    if (complete) {
      return out;
    }
    if (Date.now() >= deadline) {
      // Say WHY we gave up. Returning quietly hands marker-less (or truncated)
      // output to the ingest leg, which rejects it as an unrecognised marker —
      // blaming the injection script for a wait that ran out. Distinguish the
      // two cases the caller cannot: nothing arrived at all, versus a marker
      // that arrived but never terminated.
      Logger.warn(
        markerAt === -1
          ? `[Memory] no marker in ${out.length} bytes of debuggee output after ${MARKER_WAIT_MS}ms`
          : `[Memory] marker output still unterminated after ${MARKER_WAIT_MS}ms (${out.length} bytes) — payload likely truncated`,
      );
      return out;
    }
    await delay(MARKER_POLL_MS);
  }
}

/**
 * Resolve a frameId for a currently-stopped thread, or null if nothing is
 * paused. debugpy rejects `evaluate` without a stopped frame, so memory
 * profiling requires the debuggee to be paused at a breakpoint.
 *
 * "Is anything paused?" cannot be probed with requests: debugpy answers
 * `stackTrace` for a RUNNING thread with a sampled frame whose id is not
 * evaluable (`evaluate` then fails with "Unable to find thread for
 * evaluation"). So this gates on the tracker's `stopped`/`continued`
 * bookkeeping (dap-output.ts) and only then asks for the top frame.
 */
export async function currentStoppedFrameId(): Promise<number | null> {
  const session = activeBasiliskSession();
  if (session === undefined) { return null; }

  const stopped = stoppedThreadIds(session.id);
  if (stopped.length === 0) { return null; }

  try {
    const candidates = stopped.includes(ALL_THREADS) ? await allThreadIds(session) : stopped;
    for (const threadId of candidates) {
      const frameId = await topFrameIdIfStopped(session, threadId);
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

/** Every thread id the debuggee reports (for `allThreadsStopped` stops). */
async function allThreadIds(session: vscode.DebugSession): Promise<number[]> {
  const threads: unknown = await session.customRequest("threads");
  return recordArrayField(threads, "threads")
    .map((thread) => numberField(thread, "id"))
    .filter((id): id is number => id !== undefined);
}

/** A stopped, evaluable frame plus how to release it when the caller is done. */
export interface AcquiredFrame {
  readonly frameId: number;
  /** Resume the debuggee — only if acquiring paused it. */
  readonly release: () => Promise<void>;
}

/** Backoff between transparent-pause attempts, letting the debuggee progress
 *  out of interpreter/debugger bootstrap frames into user code. */
const PAUSE_RETRY_BACKOFF_MS = 150;

/**
 * Acquire an evaluable stopped frame, transparently pausing the debuggee
 * when it is running — IDE-grade memory snapshots must not demand a manual
 * breakpoint ([PROFILE-MEMORY-HOWTO]). When acquisition pauses the program,
 * `release` resumes it; when the user was already stopped at a breakpoint,
 * `release` is a no-op and their pause is preserved.
 *
 * A pause landing while the debuggee is still inside interpreter/debugger
 * bootstrap (a launch is only milliseconds old) suspends it in frames
 * `justMyCode` hides — `stackTrace` reports **zero frames**, and since the
 * thread now sits parked there, it would stay unevaluable forever. So a
 * transparent pause is a retry loop: pause, briefly wait for an evaluable
 * frame, and when none appears resume and re-pause after a backoff — the
 * program progresses into user code between attempts.
 */
export async function acquireStoppedFrame(): Promise<AcquiredFrame | null> {
  const session = activeBasiliskSession();
  if (session === undefined) { return null; }

  const existing = await currentStoppedFrameId();
  if (existing !== null) {
    // The user's own pause: nothing to release.
    return { frameId: existing, release: async () => { await Promise.resolve(); } };
  }

  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  for (let attempt = 1; Date.now() < deadline; attempt += 1) {
    if (!(await pauseDebuggee(session))) { return null; }
    const frameId = await waitForFrameUntil(Math.min(deadline, Date.now() + WAIT_MS));
    if (frameId !== null) {
      Logger.info(`[Memory] transparently paused the debuggee for evaluation (attempt ${attempt})`);
      return { frameId, release: async () => resumeDebuggee(session) };
    }
    // Paused, but no evaluable frame (bootstrap / hidden frames): resume so
    // the program can reach user code, then try again.
    Logger.info(`[Memory] pause landed in non-user frames (attempt ${attempt}) — resuming to retry`);
    await resumeDebuggee(session);
    await delay(PAUSE_RETRY_BACKOFF_MS);
  }
  return null;
}

/** Poll for an evaluable stopped frame until `deadlineMs` (epoch), else null. */
async function waitForFrameUntil(deadlineMs: number): Promise<number | null> {
  while (Date.now() < deadlineMs) {
    const frameId = await currentStoppedFrameId();
    if (frameId !== null) { return frameId; }
    await delay(POLL_INTERVAL_MS);
  }
  return null;
}

/** Ask debugpy to pause the first reported thread. */
async function pauseDebuggee(session: vscode.DebugSession): Promise<boolean> {
  try {
    const ids = await allThreadIds(session);
    if (ids.length === 0) { return false; }
    await session.customRequest("pause", { threadId: ids[0] });
    return true;
  } catch (err: unknown) {
    Logger.warn(`[Memory] pause failed: ${err instanceof Error ? err.message : String(err)}`);
    return false;
  }
}

/** Resume the first stopped (or reported) thread after a transparent pause. */
async function resumeDebuggee(session: vscode.DebugSession): Promise<void> {
  try {
    const stopped = stoppedThreadIds(session.id).filter((id) => id !== ALL_THREADS);
    const threadId = stopped[0] ?? (await allThreadIds(session))[0];
    if (threadId === undefined) { return; }
    await session.customRequest("continue", { threadId });
    Logger.info("[Memory] resumed the debuggee after evaluation");
  } catch (err: unknown) {
    Logger.warn(`[Memory] resume failed: ${err instanceof Error ? err.message : String(err)}`);
  }
}

/**
 * Poll for a stopped frame until the startup budget runs out. Shared by the
 * memory track-on-launch flow and the cooperative CPU sampler injection,
 * both of which wait for the `stopOnEntry` pause before evaluating.
 */
export async function waitForStoppedFrame(): Promise<number | null> {
  return waitForFrameUntil(Date.now() + STARTUP_TIMEOUT_MS);
}

/** Top frameId of `threadId` if it is stopped, else null (running threads error). */
async function topFrameIdIfStopped(
  session: vscode.DebugSession,
  threadId: number,
): Promise<number | null> {
  try {
    const stack: unknown = await session.customRequest("stackTrace", {
      threadId,
      startFrame: 0,
      levels: 1,
    });
    return numberField(recordArrayField(stack, "stackFrames")[0], "id") ?? null;
  } catch {
    return null; // thread not suspended
  }
}
