// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
/**
 * Per-session capture of debuggee output (DAP `output` events).
 *
 * Memory injection scripts `print('__BASILISK_MEM*__' + json)` — and debugpy
 * delivers that stdout as DAP `output` events, **not** in the `evaluate`
 * response result (the debuggee's stdout is redirected). So to recover a
 * marker payload after running a script, we accumulate the session's output
 * here (fed by the debug adapter tracker) and slice out what arrived after the
 * `evaluate` was issued. See `dap-evaluate.ts`.
 */

/** Cap per-session buffer so a long-lived session can't grow it unbounded. */
const MAX_BUFFER_CHARS = 1_000_000;

/** sessionId → accumulated output text. */
const buffers = new Map<string, string>();

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

/** Drop a session's buffer (called when the debug session ends). */
export function clearDebugOutput(sessionId: string): void {
  buffers.delete(sessionId);
}
