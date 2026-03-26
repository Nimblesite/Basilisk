# Basilisk Profiling — Implementation Plan

See [LSP-PROFILING-SPEC.md](../specs/LSP-PROFILING-SPEC.md) for the full technical specification.

---

## Status

- Slash command constants defined in `basilisk-common` (`PROFILE`, `PROFSTOP`, `PROFSNAPSHOT`, `MEMLEAK`, `MEMSTOP`, `MEMREFS`)
- Profiler command constants defined in `basilisk-common` (`PROFILER_START`, `PROFILER_STOP`, `PROFILER_SNAPSHOT`, `PROFILER_LIST`)
- Profiler diagnostic codes defined in `basilisk-common` (`BSK-PROF-LINE`, `BSK-PROF-FUNC`, `BSK-PROF-GIL`)
- Zed extension has stub slash command handlers (return placeholder messages, no actual profiling)
- **Phase 1 core engine COMPLETE** — py-spy, inferno, profiler module with all 6 files, fully wired into server
- **Phase 2 VS Code extension COMPLETE** — commands, settings, keybindings, status bar, heat map decorations, flamegraph webview, extension wiring
- **Server wiring COMPLETE** — profiler_handlers.rs, server/mod.rs field, shutdown hook, command dispatch all done

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

- [ ] Wire `/profile` slash command to `basilisk/profiler/start` LSP command
- [ ] Wire `/profstop` to `basilisk/profiler/stop`, format real hot functions/lines as markdown
- [ ] Wire `/profsnapshot`, `/memleak`, `/memstop`, `/memrefs` to real LSP commands
- [ ] Profiling diagnostics via standard `publishDiagnostics`

### Phase 4: macOS Privilege Escalation

- [ ] Create `crates/basilisk-profiler-helper/` — small binary for `vm_read` elevation
- [ ] Helper communicates with LSP over Unix domain socket
- [ ] LSP spawns helper via `osascript` for privilege escalation
- [ ] Skip elevation for child processes (debug sessions)
- [ ] Linux `ptrace_scope` detection and error messages
- [ ] Cross-platform tests

### Phase 5: Memory Profiling & Leak Detection

#### 5A: tracemalloc Integration
- [ ] Create `crates/basilisk-lsp/src/profiler/memory/mod.rs` — `MemorySessionManager`
- [ ] Create `crates/basilisk-lsp/src/profiler/memory/scripts.rs` — Python injection scripts
- [ ] Implement `basilisk/memory/start` and `basilisk/memory/snapshot` commands
- [ ] Generate `BSK-MEM-ALLOC` diagnostics + allocation flamegraph (purple palette)
- [ ] Memory heat map decorations (VS Code)

#### 5B: Snapshot Diffing & Leak Detection
- [ ] Create `memory/diff.rs` and `memory/leaks.rs`
- [ ] Implement `basilisk/memory/diff` — compare snapshots, track growth
- [ ] Leak scoring: Definite / High / Medium / Low
- [ ] Generate `BSK-MEM-GROWTH` and `BSK-MEM-LEAK` diagnostics
- [ ] Auto-snapshot mode + memory timeline data

#### 5C: Reference Graph Walking
- [ ] Create `memory/refgraph.rs` — reference graph builder
- [ ] Implement `basilisk/memory/references`, `objectsByType`, `gcCollect` commands
- [ ] DFS cycle detection, retention path strings
- [ ] Flag uncollectable objects as `BSK-MEM-CYCLE` errors

#### 5D: Reference Graph Visualization (VS Code)
- [ ] Build Canvas 2D force-directed graph renderer
- [ ] Node sizing/coloring, edge labels, cycle highlighting
- [ ] Click-to-expand, navigate to creation site, filter/search
- [ ] Layout modes: force-directed / tree / radial

#### 5E: Memory Dashboard Integration
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