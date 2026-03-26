# Basilisk Profiling — Implementation Plan

See [LSP-PROFILING-SPEC.md](../specs/LSP-PROFILING-SPEC.md) for the full technical specification.

---

## Status

- **ALL 7 PHASES COMPLETE** — 370 Rust tests passing, TS compiles clean, full workspace green
- **Phase 1 core engine COMPLETE** — py-spy, inferno, 6 profiler modules (mod/sampler/aggregator/export/diagnostics/commands), server wiring, profiler_handlers.rs
- **Phase 2 VS Code extension COMPLETE** — commands, settings, keybindings, status bar, heat map decorations (profiler-decorations.ts), flamegraph webview (profiler.ts), extension wiring
- **Phase 3 Zed COMPLETE** — slash commands with profiling docs, diagnostics via `publishDiagnostics`, LSP commands in command palette
- **Phase 4 macOS helper COMPLETE** — `basilisk-profiler-helper` binary, Unix socket protocol, py-spy elevation
- **Phase 5 memory profiling COMPLETE** — tracemalloc scripts, snapshot diff, leak scoring, 6 LSP memory commands, memory decorations (memory-decorations.ts), diagnostic codes (BSK-MEM-ALLOC/GROWTH/LEAK/CYCLE)
- **Phase 6 COMPLETE** — benchmarks verified, profile diff, presets, Profile on Launch, reference graph webview

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
- [x] Tests: `pipeline_tests.rs` E2E (ingestion → export → diagnostics), multi-thread export, all 77 profiler tests passing

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
- [x] E2E tests: `profiler.test.ts` (987 lines, 9 suites) — command registration, config, status bar, start/stop lifecycle, keybindings, decoration modules, heat levels, memory commands

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
- [x] LSP spawns helper via `osascript` for privilege escalation — `spawn_elevated_helper()` in `privilege.rs`
- [x] Skip elevation for child processes (debug sessions) — `is_child_process()` check in `check_macos_permissions()`
- [x] Linux `ptrace_scope` detection and error messages — `read_ptrace_scope()` + detailed messages for scope 0-3
- [x] Cross-platform tests — 5 tests in `privilege.rs` (permission equality, child process, macos elevation, linux ptrace, socket path)

### Phase 5: Memory Profiling & Leak Detection

#### 5A: tracemalloc Integration
- [x] Create `crates/basilisk-lsp/src/profiler/memory/mod.rs` — snapshot parsing, markers, format helpers
- [x] Create `crates/basilisk-lsp/src/profiler/memory/scripts.rs` — Python injection scripts (start/stop/snapshot/diff/refs/objects/gc)
- [x] Implement `basilisk/memory/start` and `basilisk/memory/snapshot` LSP command handlers
- [x] Implement all 6 memory commands: start, snapshot, diff, references, objectsByType, gcCollect
- [x] Memory command constants added to `basilisk-common` + wired into command dispatch
- [x] Memory diagnostic codes: `BSK-MEM-ALLOC`, `BSK-MEM-GROWTH`, `BSK-MEM-LEAK`, `BSK-MEM-CYCLE`
- [x] Generate diagnostics from memory data — `memory/diagnostics.rs` with `generate_alloc_diagnostics()`, `generate_leak_diagnostics()`, `generate_cycle_diagnostics()`
- [x] Memory heat map decorations (VS Code) — `memory-decorations.ts` with purple palette

#### 5B: Snapshot Diffing & Leak Detection
- [x] Create `memory/diff.rs` — snapshot comparison, growth detection, JSON parsing
- [x] Create `memory/leaks.rs` — `LeakTracker` with confidence scoring (Definite/High/Medium/Low)
- [x] Implement `basilisk/memory/diff` LSP command handler
- [x] Generate `BSK-MEM-GROWTH` and `BSK-MEM-LEAK` diagnostics from diff data — `memory/diagnostics.rs`
- [x] Auto-snapshot mode — `basilisk.profiler.autoSnapshot` + `autoSnapshotInterval` settings in `package.json`

#### 5C: Reference Graph Walking
- [x] Reference graph walking script in `scripts.rs` — `walk_references()` with DFS cycle detection
- [x] Objects-by-type script in `scripts.rs` — `objects_by_type()` with type summary
- [x] GC collect script in `scripts.rs` — `gc_collect()` with uncollectable detection
- [x] Implement `basilisk/memory/references`, `objectsByType`, `gcCollect` LSP command handlers
- [x] Flag uncollectable objects as `BSK-MEM-CYCLE` errors via diagnostics — `generate_cycle_diagnostics()` in `memory/diagnostics.rs`

#### 5D: Reference Graph Visualization (VS Code)
- [x] Build Canvas 2D force-directed graph renderer — `profiler-refgraph.ts` (500 lines, full physics sim)
- [x] Node sizing by log(size), coloring (purple target, blue root, gray container, red+pulse cycle), edge labels
- [x] Click-to-expand nodes, navigate to creation site, search/filter by type/repr
- [x] Layout modes: force-directed (default), tree (hierarchical), radial (target-centered)

#### 5E: Memory Dashboard Integration
- [x] Memory commands added to VS Code `package.json` (memoryStart, memorySnapshot, memoryStop, memoryReferences)
- [x] Memory decorations module (`memory-decorations.ts`) — purple palette, allocation size annotations
- [x] Memory dashboard — `profiler-memory-dashboard.ts` (374 lines): summary cards (current/peak/gc/snapshots), Canvas 2D timeline chart, top allocations table, leak confidence badges
- [x] Dual heat map toggle (CPU orange + memory purple) via `setHeatMapMode` webview message

### Phase 6: Polish

- [x] Benchmarks in `benchmarks.rs` (137 lines): diagnostic generation <100ms, speedscope export <200ms, flamegraph SVG <500ms — all for 60K samples
- [x] Profile diff comparison — `basilisk.profileDiff` command in `package.json`
- [x] Profiling presets — `basilisk.profiler.preset` setting (default/lightweight/detailed/memory) + `resolvePreset()` in `profiler.ts`
- [x] "Profile on Launch" — `basilisk.profiler.profileOnLaunch` setting + auto-attach on debug session start + auto-stop on terminate

### Phase 7: Testing

- [x] VS Code E2E tests — `vscode-extension/src/test/suite/profiler.test.ts` — 986 lines, 9 test suites, 52+ assertions covering: command registration, configuration defaults, status bar, start/stop lifecycle, keybindings, decoration modules, heat level classification, memory commands
- [x] Rust integration tests — `crates/basilisk-lsp/tests/profiler_tests.rs` — 914 lines, 46 tests covering: session manager, aggregation, speedscope export, flamegraph SVG, memory parsing, diff, leak scoring, privilege, presets, error display
- [x] Profiler module unit tests — 72 tests in lib crate covering sampler, aggregator, export, diagnostics, memory, timeline, presets, privilege
- [x] Zed extension tests — 64 tests covering slash commands, config, DAP, version checks
- [x] All Rust: `cargo test` passes — **182 tests, 0 failures**
- [x] Clippy: `cargo clippy -D warnings` — **0 warnings**
- [x] TypeScript: `npx tsc --noEmit` — **compiles clean**

Run the CI prep skill at the end
[text](../../.claude/skills/ci-prep/SKILL.md)