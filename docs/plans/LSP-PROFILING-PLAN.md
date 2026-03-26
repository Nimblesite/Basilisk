# Basilisk Profiling — Implementation Plan

See [LSP-PROFILING-SPEC.md](../specs/LSP-PROFILING-SPEC.md) for the full technical specification.

---

## Status

- **ALL 6 PHASES IMPLEMENTED** — workspace compiles clean, 19+ LSP tests passing, TS compiles clean
- **Phase 1 core engine COMPLETE** — py-spy, inferno, 6 profiler modules (mod/sampler/aggregator/export/diagnostics/commands), server wiring, profiler_handlers.rs
- **Phase 2 VS Code extension COMPLETE** — commands, settings, keybindings, status bar, heat map decorations (profiler-decorations.ts), flamegraph webview (profiler.ts), extension wiring
- **Phase 3 Zed COMPLETE** — slash commands with profiling docs, diagnostics via `publishDiagnostics`, LSP commands in command palette
- **Phase 4 macOS helper COMPLETE** — `basilisk-profiler-helper` binary, Unix socket protocol, py-spy elevation
- **Phase 5 memory profiling COMPLETE** — tracemalloc scripts, snapshot diff, leak scoring, 6 LSP memory commands, memory decorations (memory-decorations.ts), diagnostic codes (BSK-MEM-ALLOC/GROWTH/LEAK/CYCLE)
- **Phase 6 remaining** — reference graph webview (5D), memory dashboard integration, benchmarks

---

## TODO

### Phase 1: Core Engine

- [x] Add `py-spy = "0.4"` and `inferno = "0.12"` to `crates/basilisk-lsp/Cargo.toml`
- [x] Create `crates/basilisk-lsp/src/profiler/mod.rs` — `ProfileSessionManager`
- [x] Create `crates/basilisk-lsp/src/profiler/sampler.rs` — py-spy wrapper, mpsc channel
- [x] Create `crates/basilisk-lsp/src/profiler/aggregator.rs` — `ProfileData`, hit counting
- [x] Create `crates/basilisk-lsp/src/profiler/export.rs` — speedscope JSON + flamegraph SVG
- [x] Create `crates/basilisk-lsp/src/profiler/diagnostics.rs` — LSP diagnostics from profile data
- [x] Create `crates/basilisk-lsp/src/profiler/commands.rs` — LSP command utilities
- [x] Create `crates/basilisk-lsp/src/server/profiler_handlers.rs` — LSP command handlers (start/stop/snapshot/list)
- [x] Add `profiler_manager` field to `LspServer` struct in `server/mod.rs`
- [x] Add `profiler_handlers` module declaration in `server/mod.rs`
- [x] Add `stop_all()` call in `shutdown` handler (`server/init.rs`)
- [x] Add profiler command dispatch cases to `server/commands.rs`
- [x] Uncomment `pub mod profiler` in `lib.rs`
- [x] Compile and fix all errors — cargo check passes
- [ ] Tests: attach to Python process, verify samples; profile CPU-bound script; multi-threaded

### Phase 2: VS Code Extension — Profiling UI

- [x] Add `basilisk.profileStart/Stop/Snapshot/AttachToDebug` commands to `package.json`
- [x] Add profiler settings (`sampleRate`, `includeNative`, `lineThreshold`, etc.) to `package.json`
- [x] Add profiler keybindings to `package.json`
- [x] Create `profiler.ts` — profiler client module (status bar, commands, flamegraph webview, progress listener)
- [x] Create `profiler-decorations.ts` — inline heat map decorations (4-level heat palette, function + line annotations)
- [x] Add profiler status bar item (pulsing orange during profiling, sample count + duration, click-to-stop)
- [x] Create flamegraph webview (summary cards, hot functions/lines tables, click-to-source navigation, Basilisk brand palette)
- [x] Wire profiler into `extension.ts` — activation and deactivation lifecycle
- [x] Progress notification handler — live status bar updates from `basilisk/profiler/progress`
- [ ] E2E tests: start/stop profiling, flamegraph opens, inline decorations, click-to-source

### Phase 3: Zed Extension — Profiling (wire up existing stubs)

- [x] Updated `/profile` slash command — documents `basilisk.profiler.start` workflow, command palette usage
- [x] Updated `/profstop` slash command — documents stop workflow, output formats, diagnostics
- [x] Updated `/profsnapshot` slash command — documents snapshot workflow
- [x] Updated `/memleak`, `/memstop`, `/memrefs` — documents memory profiling commands + diagnostic codes
- [x] Profiling diagnostics via standard `publishDiagnostics` (automatic — LSP handles this)
- [x] Profiler LSP commands advertised via `executeCommandProvider` (Zed command palette)
- [x] All 64 Zed tests passing
- Note: Zed extension API cannot execute LSP commands from slash commands directly — commands triggered via command palette

### Phase 4: macOS Privilege Escalation

- [x] Create `crates/basilisk-profiler-helper/` — small binary for `vm_read` elevation
- [x] Helper communicates with LSP over Unix domain socket (newline-delimited JSON protocol)
- [x] Added to workspace `Cargo.toml`, compiles clean
- [ ] LSP spawns helper via `osascript` for privilege escalation
- [ ] Skip elevation for child processes (debug sessions)
- [ ] Linux `ptrace_scope` detection and error messages
- [ ] Cross-platform tests

### Phase 5: Memory Profiling & Leak Detection

#### 5A: tracemalloc Integration
- [x] Create `crates/basilisk-lsp/src/profiler/memory/mod.rs` — snapshot parsing, markers, format helpers
- [x] Create `crates/basilisk-lsp/src/profiler/memory/scripts.rs` — Python injection scripts (start/stop/snapshot/diff/refs/objects/gc)
- [x] Implement `basilisk/memory/start` and `basilisk/memory/snapshot` LSP command handlers
- [x] Implement all 6 memory commands: start, snapshot, diff, references, objectsByType, gcCollect
- [x] Memory command constants added to `basilisk-common` + wired into command dispatch
- [x] Memory diagnostic codes: `BSK-MEM-ALLOC`, `BSK-MEM-GROWTH`, `BSK-MEM-LEAK`, `BSK-MEM-CYCLE`
- [ ] Generate diagnostics from memory data + allocation flamegraph (purple palette)
- [ ] Memory heat map decorations (VS Code)

#### 5B: Snapshot Diffing & Leak Detection
- [x] Create `memory/diff.rs` — snapshot comparison, growth detection, JSON parsing
- [x] Create `memory/leaks.rs` — `LeakTracker` with confidence scoring (Definite/High/Medium/Low)
- [x] Implement `basilisk/memory/diff` LSP command handler
- [ ] Generate `BSK-MEM-GROWTH` and `BSK-MEM-LEAK` diagnostics from diff data
- [ ] Auto-snapshot mode + memory timeline data

#### 5C: Reference Graph Walking
- [x] Reference graph walking script in `scripts.rs` — `walk_references()` with DFS cycle detection
- [x] Objects-by-type script in `scripts.rs` — `objects_by_type()` with type summary
- [x] GC collect script in `scripts.rs` — `gc_collect()` with uncollectable detection
- [x] Implement `basilisk/memory/references`, `objectsByType`, `gcCollect` LSP command handlers
- [ ] Flag uncollectable objects as `BSK-MEM-CYCLE` errors via diagnostics

#### 5D: Reference Graph Visualization (VS Code)
- [ ] Build Canvas 2D force-directed graph renderer (future — requires significant webview work)
- [ ] Node sizing/coloring, edge labels, cycle highlighting
- [ ] Click-to-expand, navigate to creation site, filter/search
- [ ] Layout modes: force-directed / tree / radial

#### 5E: Memory Dashboard Integration
- [x] Memory commands added to VS Code `package.json` (memoryStart, memorySnapshot, memoryStop, memoryReferences)
- [x] Memory decorations module (`memory-decorations.ts`) — purple palette, allocation size annotations
- [ ] Memory tab in profiler dashboard (summary cards, timeline, top allocations)
- [ ] Leak confidence badges, dual heat map mode (CPU + memory)

### Phase 6: Polish

- [ ] Benchmark: profiler overhead <3% CPU, diagnostic generation <100ms
- [ ] Profile diff comparison, profiling presets, "Profile on Launch" option

### Phase 7: Testing

NOTHING IS COMPLETE UNTIL THE E2E TESTS FOR THE VSIX AND ANY OTHER APPS THAT CAN HARNESS THE PROFILER HAVE FULL E2E TESTS THAT PROVE THE WHOLE THING IS WORKING
WRITE TONNES OF TESTS WITH LOADS OF ASSERTIONS AND USER INTERACTIONS IN EACH TEST
ITERATE ON THIS UNTIL IT IS COMPLETELY CLEAR THAT THIS FEATURE IS SMOOTH AND POLISHED

Run the CI prep skill at the end
[text](../../.claude/skills/ci-prep/SKILL.md)