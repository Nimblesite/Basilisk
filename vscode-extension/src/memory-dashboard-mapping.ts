// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-INGEST
/**
 * Mapping from raw `basilisk.memory.ingest` results to the dashboard's
 * strongly typed shapes. Pure converters — no VS Code APIs, no state.
 * Extracted from memory-profiler.ts to satisfy the 500 LOC file limit.
 */

import type { MemoryDashboardSnapshot, MemoryDiffData } from "./memory-dashboard";

/** A tagged ingest result returned by `basilisk.memory.ingest`. */
export interface MemoryIngestResult {
  kind: "snapshot" | "diff" | "gc" | "refs" | "objects" | "ack";
  [field: string]: unknown;
}

/** Coerce an `unknown` JSON field to a string (never an object stringification). */
export function asString(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

/** Coerce an `unknown` JSON field to a finite number. */
export function asNumber(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

/** Map an ingest snapshot result to the dashboard's snapshot shape. */
export function toDashboardSnapshot(result: MemoryIngestResult): MemoryDashboardSnapshot {
  return {
    memorySessionId: asString(result.memorySessionId),
    snapshotId: asString(result.snapshotId),
    currentMemory: asNumber(result.currentMemory),
    peakMemory: asNumber(result.peakMemory),
    gcObjects: asNumber(result.gcObjects),
    gcCounts: Array.isArray(result.gcCounts) ? (result.gcCounts as number[]) : [],
    topAllocations: (Array.isArray(result.topAllocations)
      ? result.topAllocations
      : []) as MemoryDashboardSnapshot["topAllocations"],
    timeline: [],
    heapProfilePath: asString(result.heapProfilePath),
  };
}

/** Map an ingest diff result to the dashboard's diff shape (lowercasing confidence). */
export function toDashboardDiff(result: MemoryIngestResult): MemoryDiffData {
  const leaks = Array.isArray(result.suspectedLeaks) ? result.suspectedLeaks : [];
  return {
    totalGrowth: asNumber(result.totalGrowth),
    totalFreed: asNumber(result.totalFreed),
    netGrowth: asNumber(result.netGrowth),
    grownAllocations: [],
    suspectedLeaks: leaks.map((raw) => {
      const leak = raw as Record<string, unknown>;
      return {
        file: asString(leak.file),
        line: asNumber(leak.line),
        sizeGrowth: asNumber(leak.sizeGrowth),
        countGrowth: asNumber(leak.countGrowth),
        currentSize: asNumber(leak.currentSize),
        currentCount: asNumber(leak.currentCount),
        confidence: asString(leak.confidence, "low").toLowerCase() as MemoryDiffData["suspectedLeaks"][number]["confidence"],
        reason: asString(leak.reason),
      };
    }),
  };
}
