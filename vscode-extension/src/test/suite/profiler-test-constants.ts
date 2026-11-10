// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Shared constants and type helpers for profiler test suites.
 *
 * Extracted to avoid duplication across profiler.test.ts,
 * profiler-decorations.test.ts, and profiler-memory-integration.test.ts.
 *
 * Manifest contributions live in ./extension-manifest — they are read by every
 * suite, not just the profiler ones.
 */

/** Profiler client-side commands (registered in profiler.ts). */
export const PROFILER_CLIENT_COMMANDS = [
    'basilisk.profileStart',
    'basilisk.profileStop',
    'basilisk.profileSnapshot',
    'basilisk.profileAttachToDebug',
    'basilisk.profileShowResults',
] as const;

/** Memory profiler client-side commands (registered in memory-profiler.ts). */
export const MEMORY_CLIENT_COMMANDS = [
    'basilisk.memoryStart',
    'basilisk.memorySnapshot',
    'basilisk.memoryStop',
    'basilisk.memoryReferences',
] as const;

/** Profiler server-side commands (advertised by LSP). */
export const PROFILER_SERVER_COMMANDS = [
    'basilisk.profiler.start',
    'basilisk.profiler.stop',
    'basilisk.profiler.snapshot',
    'basilisk.profiler.list',
    'basilisk.profiler.cooperativeScript',
    'basilisk.profiler.cooperativeAttach',
] as const;

/** Profiler configuration keys. */
export const PROFILER_SETTINGS = [
    'basilisk.profiler.sampleRate',
    'basilisk.profiler.includeNative',
    'basilisk.profiler.lineThreshold',
    'basilisk.profiler.functionThreshold',
    'basilisk.profiler.maxDiagnosticsPerFile',
    'basilisk.profiler.showInlineHeatMap',
] as const;

/** Additional profiler configuration keys. */
export const PROFILER_EXTRA_SETTINGS = [
    'basilisk.profiler.profileOnLaunch',
    'basilisk.profiler.preset',
] as const;
