// Implements [PROFILE-NOTIFICATIONS-PROGRESS]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-NOTIFICATIONS-PROGRESS
/**
 * Shared formatting for the live profiling readouts (sample count, elapsed
 * time). Used by both the status-bar item (profiler-status.ts) and the Python
 * Processes panel chrome (process-reactivity.ts) so the two surfaces always
 * speak the same numbers — one place to change "12.3K samples (4s)".
 */

/** Seconds per minute — threshold for switching from "Ns" to "N.Mm" display. */
const SECONDS_PER_MINUTE = 60;
/** Threshold for switching from a raw count to "N.NK" display. */
const KILO_THRESHOLD = 1000;

/** Sample count as `500` or `12.3K`. */
export function formatSampleCount(sampleCount: number): string {
  return sampleCount >= KILO_THRESHOLD
    ? `${(sampleCount / KILO_THRESHOLD).toFixed(1)}K`
    : String(sampleCount);
}

/** Elapsed profiling time as `4s` or `1.5m`. */
export function formatProfileDuration(seconds: number): string {
  return seconds < SECONDS_PER_MINUTE
    ? `${seconds.toFixed(0)}s`
    : `${(seconds / SECONDS_PER_MINUTE).toFixed(1)}m`;
}
