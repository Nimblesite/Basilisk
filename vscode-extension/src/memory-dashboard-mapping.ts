// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-INGEST
/**
 * Mapping from raw `basilisk.memory.ingest` results to the dashboard's
 * strongly typed shapes. Pure converters — no VS Code APIs, no state.
 * Extracted from memory-profiler.ts to satisfy the 500 LOC file limit.
 */

import { numberArrayField, recordArrayField } from "./unknown-shape";
import type { MemoryDashboardSnapshot, MemoryDiffData } from "./memory-dashboard";
import type {
  LeakConfidence,
  MemoryAllocation,
  MemoryDiffResult,
  MemorySnapshotResult,
} from "./memory-decorations";

/** A tagged ingest result returned by `basilisk.memory.ingest`. */
export interface MemoryIngestResult {
  kind: "snapshot" | "diff" | "gc" | "refs" | "objects" | "ack";
  [field: string]: unknown;
}

/** Coerce an `unknown` JSON field to a string (never an object stringification). */
export function asString(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

/** The four leak-confidence grades the dashboard renders badges for. */
const CONFIDENCE_GRADES = ["definite", "high", "medium", "low"] as const;

/** A leak confidence grade, or `"low"` when the server sent something else. */
export function toConfidence(value: string): MemoryDiffData["suspectedLeaks"][number]["confidence"] {
  const lowered = value.toLowerCase();
  return CONFIDENCE_GRADES.find((grade) => grade === lowered) ?? "low";
}

/** Coerce an `unknown` JSON field to a finite number. */
export function asNumber(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

/** The four `LeakConfidence` grades the LSP reports, uppercase on the wire. */
const LEAK_CONFIDENCES = ["LOW", "MEDIUM", "HIGH", "DEFINITE"] as const;

/** A wire leak-confidence grade, or `"LOW"` when the server sent something else. */
function toLeakConfidence(value: string): LeakConfidence {
  const upper = value.toUpperCase();
  return LEAK_CONFIDENCES.find((grade) => grade === upper) ?? "LOW";
}

/** Read one allocation entry, defaulting every absent field. */
function toAllocation(raw: Record<string, unknown>): MemoryAllocation {
  return {
    file: asString(raw.file),
    line: asNumber(raw.line),
    size: asNumber(raw.size),
    count: asNumber(raw.count),
  };
}

/**
 * Decode the decoration-facing snapshot shape from a raw ingest result.
 *
 * `MemoryIngestResult` is an index-signature bag, so it structurally overlaps
 * `MemorySnapshotResult` without actually guaranteeing any of its fields.
 * Building the value field by field means a server that stops sending
 * `topAllocations` yields an empty list rather than an `undefined` the
 * decorations layer would iterate.
 */
export function toSnapshotResult(result: MemoryIngestResult): MemorySnapshotResult {
  return {
    memorySessionId: asString(result.memorySessionId),
    snapshotId: asString(result.snapshotId),
    currentMemory: asNumber(result.currentMemory),
    peakMemory: asNumber(result.peakMemory),
    topAllocations: recordArrayField(result, "topAllocations").map(toAllocation),
  };
}

/** Decode the decoration-facing diff shape from a raw ingest result. */
export function toDiffResult(result: MemoryIngestResult): MemoryDiffResult {
  return {
    totalGrowth: asNumber(result.totalGrowth),
    totalFreed: asNumber(result.totalFreed),
    netGrowth: asNumber(result.netGrowth),
    suspectedLeaks: recordArrayField(result, "suspectedLeaks").map((leak) => ({
      file: asString(leak.file),
      line: asNumber(leak.line),
      sizeGrowth: asNumber(leak.sizeGrowth),
      countGrowth: asNumber(leak.countGrowth),
      currentSize: asNumber(leak.currentSize),
      confidence: toLeakConfidence(asString(leak.confidence, "LOW")),
      reason: asString(leak.reason),
    })),
  };
}

/** Map an ingest snapshot result to the dashboard's snapshot shape. */
export function toDashboardSnapshot(result: MemoryIngestResult): MemoryDashboardSnapshot {
  return {
    memorySessionId: asString(result.memorySessionId),
    snapshotId: asString(result.snapshotId),
    currentMemory: asNumber(result.currentMemory),
    peakMemory: asNumber(result.peakMemory),
    gcObjects: asNumber(result.gcObjects),
    gcCounts: numberArrayField(result, "gcCounts"),
    topAllocations: recordArrayField(result, "topAllocations").map(toAllocation),
    timeline: [],
    heapProfilePath: asString(result.heapProfilePath),
  };
}

/** Map an ingest diff result to the dashboard's diff shape (lowercasing confidence). */
export function toDashboardDiff(result: MemoryIngestResult): MemoryDiffData {
  const leaks = recordArrayField(result, "suspectedLeaks");
  return {
    totalGrowth: asNumber(result.totalGrowth),
    totalFreed: asNumber(result.totalFreed),
    netGrowth: asNumber(result.netGrowth),
    grownAllocations: [],
    suspectedLeaks: leaks.map((leak) => {
      return {
        file: asString(leak.file),
        line: asNumber(leak.line),
        sizeGrowth: asNumber(leak.sizeGrowth),
        countGrowth: asNumber(leak.countGrowth),
        currentSize: asNumber(leak.currentSize),
        currentCount: asNumber(leak.currentCount),
        confidence: toConfidence(asString(leak.confidence, "low")),
        reason: asString(leak.reason),
      };
    }),
  };
}
