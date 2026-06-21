# LSP Profiler — Memory Autopilot Plan

Implements [PROFILE-MEMORY-AUTOPILOT], [PROFILE-MEMORY-AUTOPILOT-PAUSE],
[PROFILE-MEMORY-AUTOPILOT-INTERVAL], [PROFILE-MEMORY-LEAK-ACTIONS],
[PROFILE-MEMORY-REFGRAPH-PICKER].
See [docs/specs/LSP-PROFILING-SPEC.md](../specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-AUTOPILOT).

## Problem

The interactive memory-leak hunt was almost entirely manual. To watch leak
confidence climb the user had to, at **every** breakpoint pause: *Start Memory
Tracking*, then *Take Memory Snapshot*, then *Compare Memory Snapshots*, repeated
across three-plus passes. *Show Reference Graph* opened a blank input box
demanding a typed type name; *Force Garbage Collection* was yet another palette
trip. Too many clicks for what is one intent: "find the leak."

Two settings (`basilisk.profiler.autoSnapshot`, `…autoSnapshotInterval`) were
declared in `package.json` but **wired to nothing** — dead config.

## Solution — the autopilot

While memory tracking is active, the editor captures **for** the user.

1. **Snapshot+diff on every pause** ([PROFILE-MEMORY-AUTOPILOT-PAUSE]) — the
   money flow. Set a breakpoint in the leaking loop, start tracking (or use
   *Run & Track Memory (Current File)* with a breakpoint set), then just press
   Continue. Each pass auto-captures and escalates confidence Low→Medium→High.
   On by default (`basilisk.profiler.autoSnapshotOnPause`).
2. **Snapshot on an interval** ([PROFILE-MEMORY-AUTOPILOT-INTERVAL]) — wires the
   previously-dead `autoSnapshot`/`autoSnapshotInterval` settings for runs that
   never pause. Off by default.
3. **Proactive leak actions** ([PROFILE-MEMORY-LEAK-ACTIONS]) — the first
   High/Definite finding offers one-click *Show Reference Graph* / *Force GC*.
4. **Data-driven reference-graph picker** ([PROFILE-MEMORY-REFGRAPH-PICKER]) —
   replaces the free-text box with a Quick Pick of the file's own classes (from
   `textDocument/documentSymbol`) plus container builtins.

## Architecture

- **`memory-capture.ts`** (new) — the editor-as-courier round-trip and result
  presentation, extracted from the over-500-LOC `memory-profiler.ts`. Owns
  `runMemoryOperation`, `captureSnapshotAndDiff`, `presentSnapshot`/`presentDiff`,
  the rolling dashboard timeline, and the `isMemoryOperationInFlight` guard that
  keeps the autopilot from reacting to a capture's own transparent pause.
- **`memory-autopilot.ts`** (new) — the triggers. Subscribes to the store's
  tracking signal (interval-timer lifecycle), receives `stopped` events from the
  DAP tracker, enforces the re-entrancy + tracked-session guards, and offers leak
  actions. Exposes `recordedAutopilotCaptures()` / `recordedLeakOffers()` e2e
  seams (same pattern as `recordedOperations()` / `appliedMemoryDecorations()`).
- **`debug-adapter.ts`** — tracker factory gains an `onStopped` callback (sibling
  to the existing `onDebuggeeProcessId`).
- **`extension.ts`** — registers the autopilot and wires `onStopped`.
- **`memory-profiler.ts`** — slimmed; delegates capture to `memory-capture.ts`;
  `handleMemoryReferences` uses the new picker.

LSP-side: **no command-surface changes** — auto-captures reuse the existing
`basilisk.memory.snapshot`/`diff`/`ingest` round-trip, and the picker reuses
`textDocument/documentSymbol`. 100% of the engine stays shared across editors.

## Tests (real, no mocks — [CHKARCH-TESTING])

`memory-autopilot-e2e.test.ts` drives a real `basilisk-debug` session over the
`memory_autopilot_loop.py` fixture:

- **Money flow**: *Run & Track Memory* + a loop breakpoint → press Continue → the
  autopilot's recorded captures escalate to **HIGH**, attribute the real leak
  line, and paint the purple + leak decorations — with the test never invoking
  snapshot/diff itself.
- **Off switch**: `autoSnapshotOnPause=false` → no auto-capture on pause.
- **Interval mode**: `autoSnapshot=true` + tiny interval on a running program →
  ≥2 auto-captures.
- **Leak actions**: a High finding records exactly one leak-action offer.
- **Picker**: `gatherReferenceTypeCandidates` over the real document-symbol
  provider includes the fixture's classes + builtins (no free text).
- **User's example**: `examples/memory_demo.py` runs to completion under *Run &
  Track Memory* and finalises into a visible result.

## Status

Done in this branch. All five spec IDs implemented, referenced from code and
tests; dead settings now live; `memory-profiler.ts` back under the 500-LOC limit.
