// Implements [PROFILE-PROCESSES-REACTIVE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-REACTIVE
/**
 * The centralised, reactive state of the Python Processes panel (#148).
 *
 * The panel used to own its data privately — `this.processes`, `this.fetched`,
 * sort/group/filter fields — and refresh itself on a panel-local timer. Per the
 * architecture (CLAUDE.md: all mutable state lives in the single store as
 * Signals; no panel polls itself), that state is hoisted here: the store owns
 * one `ProcessPanelState` Signal, the store-side poll (process-poll.ts) feeds
 * it, and the tree provider renders it as a pure projection, subscribing via
 * `subscribeRevision` exactly like the Modules panel.
 *
 * Defined outside store.ts so the store file only grows by the signal wiring,
 * not the type surface and action bodies (the profiler-state.ts pattern).
 */

import type { Signal } from "@preact/signals-core";
import {
  GROUP_CYCLE,
  SORT_CYCLE,
  type GroupMode,
  type ProcessInfo,
  type SortMode,
} from "./process-explorer-rows";

/**
 * The process-fetch lifecycle, mirrored to the `basilisk.processesState` context
 * key so the empty-state welcome never lies: "No Python processes running" shows
 * only after a fetch actually succeeded (`loaded`), while a still-loading or
 * errored fetch says so honestly ([PROFILE-PROCESSES-PANEL], #147).
 */
export type ProcessesFetchState = "loading" | "loaded" | "error";

/**
 * Reactive snapshot of everything the Python Processes panel renders from.
 * Copy-on-write: actions replace the whole object (bumping `revision`) so the
 * backing Signal notifies subscribers.
 */
export interface ProcessPanelState {
  /** The last process table fetched from the LSP (empty until a fetch lands). */
  readonly list: readonly ProcessInfo[];
  /** Fetch lifecycle behind the welcome's empty/loading/error honesty (#147). */
  readonly fetch: ProcessesFetchState;
  /** Active sort mode (cycled from the title bar). */
  readonly sortMode: SortMode;
  /** Active grouping mode (cycled from the title bar). */
  readonly groupMode: GroupMode;
  /** The user's search filter, pre-normalised (trimmed, lowercased). */
  readonly filterText: string;
  /** PID of the active Basilisk debuggee — the only row that can be memory-tracked. */
  readonly activeDebuggeePid: number | undefined;
  /**
   * Monotonic change counter, bumped by every action. Panels subscribe to it via
   * `subscribeRevision` ([EXTACT-REACTIVE-STATE]) and re-render on each bump.
   */
  readonly revision: number;
}

/** The initial state: nothing fetched yet, so the welcome honestly says "loading". */
export const IDLE_PROCESS_PANEL: ProcessPanelState = {
  list: [],
  fetch: "loading",
  sortMode: "cpu",
  groupMode: "none",
  filterText: "",
  activeDebuggeePid: undefined,
  revision: 0,
};

/** The only way to mutate the process-panel state — exposed on the Store. */
export interface ProcessPanelActions {
  /**
   * A fetch could not run (LSP not running yet). Stays honestly "loading" —
   * never asserting the definitive "no processes" (#147) — and clears any rows
   * from a previous server session so the tree never shows stale processes.
   */
  processesLoading(): void;
  /** A fetch succeeded; only now is an empty list a genuine "no processes" (#147). */
  processesLoaded(list: readonly ProcessInfo[]): void;
  /** A fetch failed — the welcome says "couldn't load", not "no processes" (#147). */
  processesFetchFailed(): void;
  /** Advance to the next sort mode and return it (for the status-bar hint). */
  cycleProcessSort(): SortMode;
  /** Advance to the next grouping mode and return it (for the status-bar hint). */
  cycleProcessGroup(): GroupMode;
  /** Set the search filter (normalised here so every reader sees one form). */
  setProcessFilter(text: string): void;
  /** Mark which PID is the active Basilisk debuggee; `undefined` clears it. */
  setActiveDebuggeePid(pid: number | undefined): void;
}

/** Build the process-panel actions over the store's backing Signal. */
export function createProcessPanelActions(panel: Signal<ProcessPanelState>): ProcessPanelActions {
  function patch(next: Partial<ProcessPanelState>): void {
    panel.value = { ...panel.value, ...next, revision: panel.value.revision + 1 };
  }
  return {
    processesLoading() {
      patch({ list: [], fetch: "loading" });
    },
    processesLoaded(list) {
      patch({ list, fetch: "loaded" });
    },
    processesFetchFailed() {
      patch({ list: [], fetch: "error" });
    },
    cycleProcessSort() {
      const idx = SORT_CYCLE.indexOf(panel.value.sortMode);
      const sortMode = SORT_CYCLE[(idx + 1) % SORT_CYCLE.length];
      patch({ sortMode });
      return sortMode;
    },
    cycleProcessGroup() {
      const idx = GROUP_CYCLE.indexOf(panel.value.groupMode);
      const groupMode = GROUP_CYCLE[(idx + 1) % GROUP_CYCLE.length];
      patch({ groupMode });
      return groupMode;
    },
    setProcessFilter(text) {
      patch({ filterText: text.trim().toLowerCase() });
    },
    setActiveDebuggeePid(pid) {
      if (panel.value.activeDebuggeePid === pid) { return; }
      patch({ activeDebuggeePid: pid });
    },
  };
}
