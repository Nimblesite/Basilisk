// Implements [PROFILE-PROCESSES-REACTIVE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-REACTIVE
/**
 * The canonical, reactive profiling state and the actions that mutate it.
 *
 * Profiling used to keep its "am I running?" flags as module-level variables
 * inside profiler.ts (`activeSessionId`) and memory-profiler.ts
 * (`activeMemorySessionId`). Nothing on screen could react to them, so the
 * Python Processes panel kept offering "Run & Profile" while a session was
 * already running. This module hoists that state into a single object the
 * store owns as a Signal, so the status bar, the panel chrome, and the gating
 * context keys all derive from one source of truth (CLAUDE.md: "All state that
 * can change uses Signals for reactivity. No stale state on screen.").
 *
 * Defined outside store.ts so the (already large) store file only grows by the
 * signal wiring, not the type surface and the action bodies.
 */

import type { Signal } from "@preact/signals-core";

/** Lifecycle of a single profiling activity, surfaced reactively to the UI. */
export type ProfilerActivity = "idle" | "starting" | "active";

/**
 * Reactive snapshot of every profiling activity. Copy-on-write: actions
 * replace the whole object so the backing Signal notifies subscribers.
 */
export interface ProfilerSession {
  /** CPU sampling lifecycle. */
  readonly cpu: ProfilerActivity;
  /** PID being CPU-profiled (known once `cpu === "active"`). */
  readonly cpuPid: number | undefined;
  /** LSP CPU session id — routes stop/snapshot/progress. */
  readonly cpuSessionId: string | undefined;
  /** Live sample count from `basilisk/profiler/progress`. */
  readonly sampleCount: number;
  /** Live elapsed seconds from the same notification. */
  readonly durationSecs: number;
  /** Hottest function so far, for the live readout. */
  readonly topFunction: string | undefined;
  /** Memory-tracking (tracemalloc) lifecycle. */
  readonly memory: ProfilerActivity;
  /** LSP memory session id while tracking. */
  readonly memorySessionId: string | undefined;
}

/** The neutral state: nothing profiling, nothing tracking. */
export const IDLE_PROFILER_SESSION: ProfilerSession = {
  cpu: "idle",
  cpuPid: undefined,
  cpuSessionId: undefined,
  sampleCount: 0,
  durationSecs: 0,
  topFunction: undefined,
  memory: "idle",
  memorySessionId: undefined,
};

/** The only way to mutate profiling state — exposed on the Store. */
export interface ProfilerActions {
  /** A CPU start is in flight (click → before the first sample batch). */
  profilerStarting(): void;
  /** A CPU session is live for `pid`/`sessionId`; resets the live counters. */
  profilerActive(pid: number, sessionId: string): void;
  /** Live progress for the active CPU session (ignored when not active). */
  profilerProgress(sampleCount: number, durationSecs: number, topFunction: string): void;
  /** The CPU session ended (or its start failed). */
  profilerStopped(): void;
  /** A memory-tracking start is in flight. */
  memoryTrackingStarting(): void;
  /** Memory tracking is live for `sessionId`. */
  memoryTrackingActive(sessionId: string): void;
  /** Memory tracking ended (or its start failed). */
  memoryTrackingStopped(): void;
}

/** True while any CPU or memory profiling activity is starting or running. */
export function isProfilerBusy(session: ProfilerSession): boolean {
  return session.cpu !== "idle" || session.memory !== "idle";
}

/** Build the profiler actions over the store's backing Signal. */
export function createProfilerActions(profiler: Signal<ProfilerSession>): ProfilerActions {
  function patch(next: Partial<ProfilerSession>): void {
    profiler.value = { ...profiler.value, ...next };
  }
  return {
    profilerStarting() {
      patch({ cpu: "starting", cpuPid: undefined, cpuSessionId: undefined, sampleCount: 0, durationSecs: 0, topFunction: undefined });
    },
    profilerActive(pid, sessionId) {
      patch({ cpu: "active", cpuPid: pid, cpuSessionId: sessionId, sampleCount: 0, durationSecs: 0, topFunction: undefined });
    },
    profilerProgress(sampleCount, durationSecs, topFunction) {
      if (profiler.value.cpu !== "active") { return; }
      patch({ sampleCount, durationSecs, topFunction });
    },
    profilerStopped() {
      patch({ cpu: "idle", cpuPid: undefined, cpuSessionId: undefined, sampleCount: 0, durationSecs: 0, topFunction: undefined });
    },
    memoryTrackingStarting() {
      patch({ memory: "starting", memorySessionId: undefined });
    },
    memoryTrackingActive(sessionId) {
      patch({ memory: "active", memorySessionId: sessionId });
    },
    memoryTrackingStopped() {
      patch({ memory: "idle", memorySessionId: undefined });
    },
  };
}
