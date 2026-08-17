// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
/**
 * Per-session debuggee state fed by the DAP tracker: output and stop-state.
 *
 * **Output**: memory injection scripts `print('__BASILISK_MEM*__' + json)` —
 * and debugpy delivers that stdout as DAP `output` events, **not** in the
 * `evaluate` response result (the debuggee's stdout is redirected). So to
 * recover a marker payload after running a script, we accumulate the
 * session's output here and slice out what arrived after the `evaluate` was
 * issued. See `dap-evaluate.ts`.
 *
 * **Stop-state**: debugpy answers `stackTrace` for a RUNNING thread with a
 * sampled frame whose id is not evaluable (`evaluate` then fails with
 * "Unable to find thread for evaluation"), so "is anything paused?" cannot
 * be probed via requests — it must be tracked from `stopped`/`continued`
 * events, exactly as VS Code's own debug UI does. The `continued` event is
 * OPTIONAL per the DAP spec ("a debug adapter is not expected to send this
 * event in response to a request that implies that execution continues"),
 * so a successful resume-implying RESPONSE (`continue`, steps) must clear
 * the bookkeeping too — otherwise there is a stale window between the
 * response and the (late, optional) event where a courier evaluates against
 * a sampled frame of a running thread and fails.
 */

/** Cap per-session buffer so a long-lived session can't grow it unbounded. */
import { booleanField, numberField, rawField, stringField } from "./unknown-shape";

const MAX_BUFFER_CHARS = 1_000_000;

/** sessionId → accumulated output text. */
const buffers = new Map<string, string>();

/** sessionId → thread ids reported stopped (empty/absent = running). */
const stoppedThreads = new Map<string, Set<number>>();

/** Sentinel for a `stopped` event carrying `allThreadsStopped` but no id. */
export const ALL_THREADS = -1;

/**
 * Record a `stopped`/`continued` event from the DAP tracker. A continue
 * always invalidates the ALL_THREADS marker — when in doubt we prefer
 * "running" (an honest "pause first" beats an unevaluable stale frame).
 */
export function trackSuspensionEvent(
  sessionId: string,
  event: "stopped" | "continued",
  body: unknown,
): void {
  const threadId = numberField(body, "threadId");
  if (event === "stopped") {
    const set = stoppedThreads.get(sessionId) ?? new Set<number>();
    if (threadId !== undefined) { set.add(threadId); }
    if (booleanField(body, "allThreadsStopped") === true) { set.add(ALL_THREADS); }
    if (set.size > 0) { stoppedThreads.set(sessionId, set); }
    return;
  }
  if (booleanField(body, "allThreadsContinued") !== false || threadId === undefined) {
    stoppedThreads.delete(sessionId);
    return;
  }
  const set = stoppedThreads.get(sessionId);
  set?.delete(threadId);
  set?.delete(ALL_THREADS);
  if (set?.size === 0) { stoppedThreads.delete(sessionId); }
}

/** Thread ids currently stopped (may contain [`ALL_THREADS`]); empty = running. */
export function stoppedThreadIds(sessionId: string): readonly number[] {
  return [...(stoppedThreads.get(sessionId) ?? [])];
}

/** Requests whose successful response means execution resumed (DAP spec:
 *  the `continued` event is optional after these, so the response is the
 *  only guaranteed signal). */
const RESUME_COMMANDS = new Set([
  "continue", "reverseContinue", "next", "stepIn", "stepOut", "stepBack", "goto", "restartFrame",
]);

/** Resume commands whose response covers every thread unless the adapter
 *  says otherwise (`allThreadsContinued` defaults to true per the spec). */
const ALL_THREAD_RESUMES = new Set(["continue", "reverseContinue"]);

/** sessionId → seq of an in-flight resume request → its command + threadId. */
const pendingResumes = new Map<string, Map<number, { command: string; threadId?: number }>>();

/** Remember an outgoing resume-implying request (DAP tracker, editor → adapter). */
export function trackResumeRequest(sessionId: string, message: unknown): void {
  const seq = numberField(message, "seq");
  if (stringField(message, "type") !== "request" || seq === undefined) { return; }
  const command = stringField(message, "command");
  if (command === undefined || !RESUME_COMMANDS.has(command)) { return; }
  const pending = pendingResumes.get(sessionId) ?? new Map<number, { command: string; threadId?: number }>();
  pending.set(seq, { command, threadId: numberField(rawField(message, "arguments"), "threadId") });
  pendingResumes.set(sessionId, pending);
}

/**
 * Clear stop-state when a resume-implying request SUCCEEDS (adapter → editor).
 * A failed resume did not move anything, so the pause survives. A `continue`
 * clears every thread unless the adapter narrows it (`allThreadsContinued:
 * false`); a step clears only the stepped thread — its own `stopped` event
 * re-arms the bookkeeping when the step lands.
 */
export function trackResumeResponse(sessionId: string, message: unknown): void {
  const requestSeq = numberField(message, "request_seq");
  if (stringField(message, "type") !== "response" || requestSeq === undefined) { return; }
  const pending = pendingResumes.get(sessionId);
  const request = pending?.get(requestSeq);
  if (pending === undefined || request === undefined) { return; }
  pending.delete(requestSeq);
  if (pending.size === 0) { pendingResumes.delete(sessionId); }
  if (booleanField(message, "success") !== true) { return; }
  const allThreads =
    ALL_THREAD_RESUMES.has(request.command) &&
    booleanField(rawField(message, "body"), "allThreadsContinued") !== false;
  trackSuspensionEvent(sessionId, "continued", {
    threadId: request.threadId,
    allThreadsContinued: allThreads,
  });
}

/** Append a chunk of debuggee output for a session (called by the DAP tracker). */
export function appendDebugOutput(sessionId: string, text: string): void {
  const combined = (buffers.get(sessionId) ?? "") + text;
  buffers.set(
    sessionId,
    combined.length > MAX_BUFFER_CHARS
      ? combined.slice(combined.length - MAX_BUFFER_CHARS)
      : combined,
  );
}

/** Current length of a session's output buffer — a cursor for [`debugOutputSince`]. */
export function debugOutputCursor(sessionId: string): number {
  return (buffers.get(sessionId) ?? "").length;
}

/** Output appended after `cursor` (everything, if the buffer was trimmed past it). */
export function debugOutputSince(sessionId: string, cursor: number): string {
  const all = buffers.get(sessionId) ?? "";
  return cursor < all.length ? all.slice(cursor) : "";
}

/** Drop a session's buffer and stop-state (called when the debug session ends). */
export function clearDebugOutput(sessionId: string): void {
  buffers.delete(sessionId);
  stoppedThreads.delete(sessionId);
  pendingResumes.delete(sessionId);
}
