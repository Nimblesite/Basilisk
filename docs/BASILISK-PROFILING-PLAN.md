# Basilisk Profiling — Implementation Plan

See [BASILISK-PROFILING-SPEC.md](BASILISK-PROFILING-SPEC.md) for the full technical specification.

## Implementation Plan

### Phase 1: Core Engine (Rust LSP)

Add py-spy as a Cargo dependency and build the profiler module inside `basilisk-lsp`.

- Add `py-spy = "0.4"` and `inferno = "0.12"` to `crates/basilisk-lsp/Cargo.toml`
- Create `crates/basilisk-lsp/src/profiler/mod.rs` — module root, `ProfileSessionManager`
- Create `crates/basilisk-lsp/src/profiler/sampler.rs` — wrapper around py-spy `Sampler`, mpsc channel
- Create `crates/basilisk-lsp/src/profiler/aggregator.rs` — `ProfileData` struct, hit counting, function stats
- Create `crates/basilisk-lsp/src/profiler/export.rs` — speedscope JSON and flamegraph SVG generation
- Create `crates/basilisk-lsp/src/profiler/diagnostics.rs` — convert `ProfileData` to LSP diagnostics
- Create `crates/basilisk-lsp/src/profiler/commands.rs` — LSP command handlers (start/stop/snapshot/list)
- Wire profiler commands into `server.rs` `execute_command` handler
- Register profiler commands in `initialize` capabilities
- Add session cleanup on LSP `shutdown`
- Test: send raw `basilisk/profiler/start` with a known Python PID, verify samples collected

### Phase 2: VS Code Extension — Profiling UI

Build the rich profiling experience in the VS Code extension.

#### Commands & Activation
- Add `basilisk.profileStart` command to `package.json`
- Add `basilisk.profileStop` command
- Add `basilisk.profileSnapshot` command
- Add `basilisk.profileAttachToDebug` command
- Add keyboard shortcuts (Ctrl+Shift+P → "Basilisk: Start Profiling")
- Add profiler status bar item

#### Inline Heat Map
- Create `vscode-extension/src/profiler/decorations.ts` — `TextEditorDecorationType` per heat level
- Define Basilisk-branded color palette for heat levels (red/orange/yellow/dim, matching design system)
- Listen for `basilisk/profiler/diagnostics` and apply decorations
- Update decorations on active editor change
- Clear decorations on profile clear

#### Flamegraph Webview — THE BEAUTIFUL ONE
- Create `vscode-extension/src/profiler/flamegraph-panel.ts` — WebviewPanel
- Build flamegraph renderer using Basilisk design system colors, fonts, and styling
- **Animated flamegraph** — smooth transitions on zoom/filter, frame highlight animations
- **Pie chart breakdown** — donut chart showing top-N functions by CPU%, animated on load
- **Timeline graph** — line chart showing CPU% over time per function, smooth curves, hover tooltips
- **Sunburst chart** — alternative to flamegraph, animated radial layout with drill-down
- Click any frame/slice → navigate to source file:line
- Search/filter bar with live highlighting
- Toggle between flamegraph, sunburst, timeline, and table views
- Dark/light theme support using Basilisk design tokens
- Export as PNG/SVG from the panel
- Responsive layout — adapts to panel size

#### Profiler Dashboard
- Create `vscode-extension/src/profiler/dashboard-panel.ts` — secondary WebviewPanel
- **Summary cards** — total time, sample count, hot function count (animated counters)
- **Top functions table** — sortable by total%, self%, samples, with sparkline mini-charts
- **Thread breakdown** — stacked bar chart showing per-thread CPU distribution
- **GIL contention indicator** — animated gauge showing GIL wait percentage
- **Call tree** — collapsible tree view with percentage bars
- **Diff mode** — compare two profiles side-by-side with delta highlighting

### Phase 3: Zed Extension — Profiling

Build the Zed profiling integration using available APIs.

- Implement `/profile` slash command handler in `basilisk-zed/src/lib.rs`
- Implement `/profstop` slash command handler
- Slash command output: formatted markdown with hot functions, hot lines, file references
- Profiling diagnostics flow through standard LSP `publishDiagnostics` (hint severity)
- Speedscope JSON written to temp file, path included in slash command output
- Auto-open speedscope.app in browser with local file (when Zed gains this capability)

### Phase 4: macOS Privilege Escalation

Handle the `vm_read` permission requirement on macOS.

- Create `crates/basilisk-profiler-helper/` — small Rust binary that attaches to a PID and streams samples
- Helper communicates with LSP over Unix domain socket
- LSP spawns helper via `osascript -e 'do shell script "..." with administrator privileges'`
- User sees one macOS password prompt per profiling session
- Skip elevation entirely when profiling a child process (debug session)
- Add Linux `ptrace_scope` detection and user-friendly error messages
- Test on all three platforms

### Phase 5: Memory Profiling & Leak Detection

Build the comprehensive memory analysis tool on top of the debug adapter.

#### 5A: tracemalloc Integration (Allocation Tracking)
- Add `basilisk/memory/start` command — inject `tracemalloc.start(25)` and `gc.set_debug(gc.DEBUG_SAVEALL)` via DAP evaluate
- Add `basilisk/memory/snapshot` command — inject `tracemalloc.take_snapshot()`, parse results into `MemorySnapshot` struct
- Create `crates/basilisk-lsp/src/profiler/memory/mod.rs` — `MemorySessionManager`
- Create `crates/basilisk-lsp/src/profiler/memory/scripts.rs` — Python injection scripts as const strings
- Parse tracemalloc output (per-line size, count, full tracebacks up to 25 frames)
- Generate per-line allocation diagnostics (`BSK-MEM-ALLOC` hint severity)
- Generate allocation flamegraph using same flamegraph component (purple palette instead of orange)
- Memory heat map inline decorations (purple gradient in VS Code)

#### 5B: Snapshot Diffing (Leak Detection)
- Add `basilisk/memory/diff` command — compare two snapshots
- Create `crates/basilisk-lsp/src/profiler/memory/diff.rs` — snapshot comparison logic
- Create `crates/basilisk-lsp/src/profiler/memory/leaks.rs` — leak confidence scoring
- Track growing allocations across consecutive diffs
- Score suspected leaks: Definite (uncollectable cycle), High (3+ consistent diffs), Medium (2 diffs), Low (single diff)
- Generate leak diagnostics (`BSK-MEM-GROWTH` warning, `BSK-MEM-LEAK` warning)
- Auto-snapshot mode: periodic snapshots at configurable interval for trend analysis
- Memory timeline chart: stacked area chart showing memory by type over time

#### 5C: Reference Graph Walking (THE BIG ONE)
- Add `basilisk/memory/references` command — walk `gc.get_referrers()` / `gc.get_referents()`
- Create `crates/basilisk-lsp/src/profiler/memory/refgraph.rs` — reference graph builder
- Inject `walk_references()` script via DAP evaluate (see spec for full script)
- Parse graph response: nodes (id, type, size, repr) and edges (from, to, label)
- Detect reference cycles using DFS
- Build human-readable retention paths ("module:src.cache .cache_instance → LRUCache → dict → DataFrame (248 MB)")
- Add `basilisk/memory/objectsByType` command — list all objects of a type with sizes and refcounts
- Add `basilisk/memory/gcCollect` command — force gc, report collected + uncollectable objects
- Flag uncollectable objects (`__del__` in cycle) as `BSK-MEM-CYCLE` errors

#### 5D: Reference Graph Visualization (VS Code)
- Create `vscode-extension/src/profiler/refgraph/` directory
- Build force-directed graph renderer using Canvas 2D
- Node sizing proportional to `log(size)`
- Node coloring: purple for targets, blue for root retainers, gray for containers, pulsing red for cycles
- Edge labels showing reference type (`.attribute`, `['key']`, `[index]`)
- Click node to expand referrers/referents (lazy-load via LSP)
- Right-click "Navigate to Creation Site" (tracemalloc traceback)
- Cycle detection banner with explanation
- Layout modes: force-directed, tree (hierarchical), radial
- Filter by type, minimum size, search by repr
- Retention path summary at bottom of panel

#### 5E: Memory Dashboard Integration
- Add memory tab to profiler dashboard
- Animated memory summary cards (current, peak, gc objects, gc counts)
- Memory timeline (stacked area, smooth bezier curves)
- Top allocations table with sparklines
- Leak confidence badges next to suspected leak entries
- Dual heat map mode: CPU (orange, left gutter) + memory (purple, right gutter) simultaneously
- "Hot and Heavy" filter: functions that are both CPU-intensive and memory-intensive

### Phase 6: Polish & Performance

- Benchmark profiler overhead (target: <3% CPU on target process)
- Benchmark diagnostic generation (target: <100ms for 60K samples)
- Add profile comparison (diff two profiles, highlight regressions)
- Add profiling presets: "Quick (10s)", "Standard (60s)", "Long-running"
- Add "Profile on launch" debug configuration option
- Documentation and demo videos

---

## TODO List

### Phase 1: Core Engine

#### Cargo & Module Structure
- [ ] Add `py-spy = "0.4"` to `crates/basilisk-lsp/Cargo.toml`
- [ ] Add `inferno = "0.12"` to `crates/basilisk-lsp/Cargo.toml`
- [ ] Add `serde_json` (if not already present) for speedscope export
- [ ] Create `crates/basilisk-lsp/src/profiler/mod.rs`
- [ ] Create `crates/basilisk-lsp/src/profiler/sampler.rs`
- [ ] Create `crates/basilisk-lsp/src/profiler/aggregator.rs`
- [ ] Create `crates/basilisk-lsp/src/profiler/export.rs`
- [ ] Create `crates/basilisk-lsp/src/profiler/diagnostics.rs`
- [ ] Create `crates/basilisk-lsp/src/profiler/commands.rs`

#### ProfileSessionManager (`mod.rs`)
- [ ] Define `ProfileSession` struct (session_id, pid, start_time, sampler handle, aggregator)
- [ ] Define `ProfileError` enum (ProcessNotFound, NotPython, PermissionDenied, AlreadyProfiling, UnsupportedVersion)
- [ ] Implement `start_session(pid, config) -> Result<ProfileSession>`
- [ ] Implement `stop_session(session_id) -> Result<ProfileData>`
- [ ] Implement `snapshot(session_id) -> Result<ProfileData>`
- [ ] Implement `list_sessions() -> Vec<SessionInfo>`
- [ ] Implement `cleanup_all()` for LSP shutdown
- [ ] Handle process exit during profiling (detect and auto-stop)

#### Sampler (`sampler.rs`)
- [ ] Wrap `py_spy::PythonSpy::new()` with error mapping to `ProfileError`
- [ ] Spawn sampling thread with `py_spy::sampler::Sampler`
- [ ] Send `Vec<StackTrace>` through `mpsc::Sender` to aggregator
- [ ] Implement graceful shutdown via `AtomicBool` flag
- [ ] Handle sampling errors (process exit, permission revoked) without panic

#### Aggregator (`aggregator.rs`)
- [ ] Define `ProfileData` struct (line_hits, function_stats, frame_index, thread_stacks)
- [ ] Define `FunctionStats` struct (name, file, line, total_samples, self_samples)
- [ ] Implement `ingest(traces: Vec<StackTrace>)` — accumulate hits
- [ ] Implement `hot_lines(threshold: f64) -> Vec<HotLine>`
- [ ] Implement `hot_functions(threshold: f64) -> Vec<HotFunction>`
- [ ] Implement frame deduplication by (name, filename, line)
- [ ] Thread-safe: `Mutex<ProfileData>` or channel-based

#### Export (`export.rs`)
- [ ] Define speedscope JSON structs (serde `Serialize`)
- [ ] Implement `to_speedscope(data: &ProfileData) -> serde_json::Value`
- [ ] Reverse stack order (py-spy leaf-first → speedscope root-first)
- [ ] Write to temp file, return path
- [ ] Implement `to_flamegraph_svg(data: &ProfileData) -> String` using `inferno`
- [ ] Convert aggregated stacks to collapsed format for inferno

#### Diagnostics (`diagnostics.rs`)
- [ ] Implement `to_diagnostics(data: &ProfileData, config: &ProfilerConfig) -> HashMap<Url, Vec<Diagnostic>>`
- [ ] Hot line diagnostics: severity Hint, source "basilisk-profiler", code "BSK-PROF-LINE"
- [ ] Hot function diagnostics: severity Hint, code "BSK-PROF-FUNC"
- [ ] GIL contention diagnostics: severity Info, code "BSK-PROF-GIL"
- [ ] Respect threshold and max-per-file config
- [ ] Format percentages to 1 decimal place

#### LSP Integration (`commands.rs` + `server.rs`)
- [ ] Handle `basilisk/profiler/start` in `execute_command`
- [ ] Handle `basilisk/profiler/stop` in `execute_command`
- [ ] Handle `basilisk/profiler/snapshot` in `execute_command`
- [ ] Handle `basilisk/profiler/list` in `execute_command`
- [ ] Register all four commands in `initialize` capabilities
- [ ] Publish profiling diagnostics via `textDocument/publishDiagnostics`
- [ ] Send `basilisk/profiler/progress` notifications during active profiling
- [ ] Auto-detect PID from active debug session when pid is omitted
- [ ] Clean up profiling sessions on LSP shutdown

#### Tests — Core Engine
- [ ] Unit: aggregator correctly counts line hits for known stacks
- [ ] Unit: aggregator correctly computes self vs total samples
- [ ] Unit: hot_lines respects threshold
- [ ] Unit: hot_functions respects threshold and max count
- [ ] Unit: speedscope export matches schema (frame dedup, stack reversal, weights)
- [ ] Unit: flamegraph SVG is valid SVG
- [ ] Unit: diagnostics have correct severity, source, code, message format
- [ ] Integration: attach to a sleeping Python process, verify non-zero samples
- [ ] Integration: profile a CPU-bound Python script, verify hot function matches bottleneck
- [ ] Integration: profile a multi-threaded script, verify per-thread data

### Phase 2: VS Code Extension — Profiling UI

#### Commands & Configuration
- [ ] Add `basilisk.profileStart` to `package.json` commands
- [ ] Add `basilisk.profileStop` to `package.json` commands
- [ ] Add `basilisk.profileSnapshot` to `package.json` commands
- [ ] Add `basilisk.profileAttachToDebug` to `package.json` commands
- [ ] Add `basilisk.profiler.*` settings to `package.json` contributes.configuration
- [ ] Add keyboard shortcuts for profiling commands
- [ ] Implement PID picker (QuickPick with running Python processes)
- [ ] Add profiler status bar item (state, duration, sample count)

#### Inline Heat Map
- [ ] Create `vscode-extension/src/profiler/decorations.ts`
- [ ] Define 4 heat level decoration types with Basilisk brand colors
- [ ] Subscribe to profiling diagnostics from LSP
- [ ] Apply decorations to active editors based on hot-line data
- [ ] Update decorations on editor switch
- [ ] Clear decorations on profile clear or new profile
- [ ] Animate decoration appearance (fade in)

#### Flamegraph Webview
- [ ] Create `vscode-extension/src/profiler/flamegraph-panel.ts`
- [ ] Design flamegraph HTML/CSS/JS using Basilisk design system
- [ ] Implement animated flamegraph renderer (d3-flamegraph or custom canvas)
- [ ] Implement animated donut/pie chart for top-N function breakdown
- [ ] Implement timeline line chart (CPU% over time per function, smooth bezier curves)
- [ ] Implement sunburst chart as alternative view
- [ ] Add smooth CSS transitions and animations on load, zoom, filter
- [ ] Implement click-to-source navigation (postMessage to extension, `vscode.window.showTextDocument`)
- [ ] Add search/filter bar with live highlighting
- [ ] Add view toggle (flamegraph / sunburst / timeline / table)
- [ ] Support dark and light VS Code themes with Basilisk design tokens
- [ ] Add export buttons (PNG, SVG)

#### Profiler Dashboard
- [ ] Create `vscode-extension/src/profiler/dashboard-panel.ts`
- [ ] Animated summary counters (total time, samples, functions profiled)
- [ ] Top functions table with inline sparklines
- [ ] Thread breakdown stacked bar chart
- [ ] GIL contention gauge (animated needle/arc)
- [ ] Collapsible call tree with percentage bars
- [ ] Profile diff mode (select two profiles, show deltas with green/red coloring)

#### Tests — VS Code Profiling
- [ ] E2E: Start profiling via command palette, verify status bar updates
- [ ] E2E: Stop profiling, verify flamegraph webview opens
- [ ] E2E: Verify inline decorations appear for hot lines
- [ ] E2E: Profile during debug session, verify auto-PID detection
- [ ] E2E: Click flamegraph frame, verify navigation to source

### Phase 3: Zed Extension

#### Extension Scaffolding
- [ ] Create `basilisk-zed/` directory with extension.toml, Cargo.toml, src/lib.rs
- [ ] Implement `language_server_command()` — resolve and return basilisk binary
- [ ] Implement `language_server_initialization_options()` — workspace root
- [ ] Implement `language_server_workspace_configuration()` — read Zed settings
- [ ] Implement binary resolution (PATH check, ~/.cargo/bin, GitHub release download)
- [ ] Set up tree-sitter-python grammar reference in extension.toml
- [ ] Add Python language config (config.toml, highlights.scm, outline.scm, etc.)

#### Debugging
- [ ] Implement `get_dap_binary()` — return basilisk binary with debug-adapter args
- [ ] Create `debug_adapter_schemas/basilisk-debug.json` with launch/attach schema
- [ ] Test: debug session launches via Zed's debug UI

#### Profiling via Slash Commands
- [ ] Register `/profile` slash command in extension.toml
- [ ] Register `/profstop` slash command
- [ ] Implement `run_slash_command` for `/profile` — send LSP `basilisk/profiler/start`
- [ ] Implement `run_slash_command` for `/profstop` — send LSP `basilisk/profiler/stop`, format output
- [ ] Format slash command output as markdown (hot functions table, hot lines, speedscope path)
- [ ] Implement argument completion for `/profile` (suggest PIDs of running Python processes)

#### Tests — Zed
- [ ] Slash command `/profile` returns session ID
- [ ] Slash command `/profstop` returns formatted hot functions
- [ ] LSP diagnostics appear as hints for hot lines
- [ ] Speedscope JSON file is written and valid

### Phase 4: macOS Privilege Escalation
- [ ] Create `crates/basilisk-profiler-helper/Cargo.toml`
- [ ] Implement helper binary: accept PID via CLI arg, attach via py-spy, stream samples over Unix socket
- [ ] Implement LSP-side Unix socket client to receive samples from helper
- [ ] Spawn helper via `osascript` for privilege escalation
- [ ] Skip escalation when PID is a child of the LSP process
- [ ] Add Linux `ptrace_scope` detection and error message
- [ ] Test: macOS external process profiling with privilege prompt
- [ ] Test: macOS debug-session profiling without prompt
- [ ] Test: Linux with ptrace_scope=0 and ptrace_scope=1

### Phase 5: Memory Profiling & Leak Detection

#### 5A: tracemalloc Integration
- [ ] Create `crates/basilisk-lsp/src/profiler/memory/mod.rs` — `MemorySessionManager`
- [ ] Create `crates/basilisk-lsp/src/profiler/memory/scripts.rs` — Python injection scripts
- [ ] Implement `basilisk/memory/start` — inject `tracemalloc.start(25)` via DAP evaluate
- [ ] Implement `basilisk/memory/snapshot` — inject `tracemalloc.take_snapshot()`, parse JSON output
- [ ] Define `MemorySnapshot` struct (per-line allocations, tracebacks, totals)
- [ ] Generate `BSK-MEM-ALLOC` diagnostics for top allocation sites
- [ ] Generate allocation flamegraph (purple palette, bytes on X-axis)
- [ ] Memory heat map decorations (VS Code, purple gradient)
- [ ] Wire memory commands into `server.rs` `execute_command`
- [ ] Test: inject tracemalloc into debug session, verify allocations captured

#### 5B: Snapshot Diffing & Leak Detection
- [ ] Create `crates/basilisk-lsp/src/profiler/memory/diff.rs` — snapshot comparison
- [ ] Create `crates/basilisk-lsp/src/profiler/memory/leaks.rs` — leak confidence scoring
- [ ] Implement `basilisk/memory/diff` — compare two snapshots by line
- [ ] Track growth across 3+ consecutive diffs for high-confidence leak detection
- [ ] Score leaks: Definite / High / Medium / Low
- [ ] Generate `BSK-MEM-GROWTH` and `BSK-MEM-LEAK` diagnostics with confidence badges
- [ ] Auto-snapshot mode: configurable periodic snapshots
- [ ] Memory timeline data generation (per-snapshot totals by type)
- [ ] Test: allocate in a loop across snapshots, verify growth detected
- [ ] Test: free memory between snapshots, verify freed allocations reported
- [ ] Test: uncollectable cycle with `__del__` flagged as Definite

#### 5C: Reference Graph Walking
- [ ] Create `crates/basilisk-lsp/src/profiler/memory/refgraph.rs` — reference graph builder
- [ ] Define `ReferenceGraph` struct (nodes, edges, cycles, retention paths)
- [ ] Define `GraphNode` struct (id, type, size, repr, depth, is_target, retained_size)
- [ ] Define `GraphEdge` struct (from, to, label)
- [ ] Build `walk_references()` Python script with gc.get_referrers/get_referents
- [ ] Build `_find_ref_label()` — determine dict key, list index, or attribute name for each edge
- [ ] Build `_detect_cycles()` — DFS cycle detection on reference graph
- [ ] Implement `basilisk/memory/references` command — inject script, parse graph JSON
- [ ] Build human-readable retention path strings
- [ ] Implement `basilisk/memory/objectsByType` — list objects by type with sizes
- [ ] Implement `basilisk/memory/gcCollect` — force gc, report collected + uncollectable
- [ ] Flag uncollectable objects as `BSK-MEM-CYCLE` errors
- [ ] Handle module.__dict__ detection (identify module-level references)
- [ ] Limit fan-out (max_nodes, max_depth) to prevent explosion
- [ ] Test: create known retention chain, verify graph correctly represents it
- [ ] Test: create reference cycle, verify cycle detected
- [ ] Test: create cycle with `__del__`, verify flagged as uncollectable
- [ ] Test: large graph (1000+ objects), verify max_nodes limits output

#### 5D: Reference Graph Visualization (VS Code)
- [ ] Create `vscode-extension/src/profiler/refgraph/` directory
- [ ] Build Canvas 2D force-directed graph renderer
- [ ] Implement physics simulation (repulsion, attraction, damping, settling)
- [ ] Node sizing: proportional to `log(size)`, minimum 20px, maximum 80px
- [ ] Node coloring: purple targets, blue roots, gray containers, red cycles
- [ ] Edge rendering: curved arrows with labels
- [ ] Cycle highlighting: thick red edges, pulsing animation (CSS keyframes via Canvas)
- [ ] Tooltip on hover: type, size, repr, refcount, creation site
- [ ] Click node: lazy-load expand (send `basilisk/memory/references` for that node)
- [ ] Right-click "Navigate to Creation Site" (open file at tracemalloc traceback line)
- [ ] Retention path summary panel at bottom
- [ ] Layout toggle: force-directed / tree / radial
- [ ] Filter bar: type filter, minimum size, repr search
- [ ] Zoom and pan controls
- [ ] Export graph as PNG/SVG
- [ ] Test: render known 5-node graph, verify layout and labels
- [ ] Test: click expand, verify new nodes appear with animation

#### 5E: Memory Dashboard Integration
- [ ] Add "Memory" tab to profiler dashboard webview
- [ ] Animated summary cards: current memory, peak, gc objects, gc generation counts
- [ ] Memory timeline: stacked area chart (by type, smooth bezier curves)
- [ ] Top allocations table with sparklines (size over snapshots)
- [ ] Leak confidence badges (color-coded by Definite/High/Medium/Low)
- [ ] Dual heat map mode in editor: CPU orange (left) + memory purple (right)
- [ ] "Hot and Heavy" combined filter
- [ ] Type breakdown donut chart (top types by total size)
- [ ] "Force GC" button (calls `basilisk/memory/gcCollect`, animates freed count)
- [ ] Test: dashboard renders with mock memory data
- [ ] Test: timeline updates with new snapshot data

### Phase 6: Polish
- [ ] Benchmark: profiler overhead <3% CPU on target
- [ ] Benchmark: diagnostic generation <100ms for 60K samples
- [ ] Benchmark: speedscope export <200ms for 60K samples
- [ ] Add profile diff comparison
- [ ] Add profiling presets (Quick/Standard/Long-running)
- [ ] Add "Profile on Launch" debug configuration option
- [ ] Documentation and README updates
- [ ] Demo scripts and screenshots
