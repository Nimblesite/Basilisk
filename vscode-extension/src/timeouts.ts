// Implements [VSIX-ARCHITECTURE]. See docs/specs/VSIX-SPEC.md#VSIX-ARCHITECTURE
/**
 * Canonical timeout constants for the Basilisk VS Code extension.
 *
 * Three knobs. No others. Anything that feels like it needs a fourth is a
 * design smell — fix the underlying slowness, do not invent a new bucket.
 */

/** Interval between polls. */
export const POLL_INTERVAL_MS = 10;

/** Max time to wait for a single command/event to settle at runtime.
 *  If a wait exceeds this, the operation is broken. */
export const WAIT_MS = 1_000;

/** Startup / cold-init timeout. Anything slower than this is a bug. */
export const STARTUP_TIMEOUT_MS = 10_000;
