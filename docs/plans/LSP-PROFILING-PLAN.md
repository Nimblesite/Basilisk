# Basilisk Profiling — Implementation Plan

See [LSP-PROFILING-SPEC.md](../specs/LSP-PROFILING-SPEC.md) for the full technical specification.

---

## Status

- Slash command constants defined in `basilisk-common` (`PROFILE`, `PROFSTOP`, `PROFSNAPSHOT`, `MEMLEAK`, `MEMSTOP`, `MEMREFS`)
- Zed extension has stub slash command handlers (return placeholder messages, no actual profiling)
- **No profiling engine exists yet** — no py-spy, no inferno, no profiler module in LSP

---

## TODO

### Phase 1: Core Engine

- [ ] Add `py-spy = "0.4"` and `inferno = "0.12"` to `crates/basilisk-lsp/Cargo.toml`
- [ ] Create `crates/basilisk-lsp/src/profiler/mod.rs` — `ProfileSessionManager`
- [ ] Create `crates/basilisk-lsp/src/profiler/sampler.rs` — py-spy wrapper, mpsc channel
- [ ] Create `crates/basilisk-lsp/src/profiler/aggregator.rs` — `ProfileData`, hit counting
- [ ] Create `crates/basilisk-lsp/src/profiler/export.rs` — speedscope JSON + flamegraph SVG
- [ ] Create `crates/basilisk-lsp/src/profiler/diagnostics.rs` — LSP diagnostics from profile data
- [ ] Create `crates/basilisk-lsp/src/profiler/commands.rs` — LSP command handlers
- [ ] Wire profiler commands into `server.rs` and register in `initialize` capabilities
- [ ] Tests: attach to Python process, verify samples; profile CPU-bound script; multi-threaded

### Phase 2: VS Code Extension — Profiling UI

- [ ] Add `basilisk.profileStart/Stop/Snapshot/AttachToDebug` commands to `package.json`
- [ ] Add profiler status bar item
- [ ] Create inline heat map decorations (`decorations.ts`)
- [ ] Create flamegraph webview (animated, with pie chart, timeline, sunburst views)
- [ ] Create profiler dashboard (summary cards, top functions, thread breakdown, GIL gauge, call tree, diff mode)
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
