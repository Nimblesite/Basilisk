# Basilisk profiling {#LSPPROF}

Basilisk samples Python CPU stacks, exports profiles, publishes hotspot hints, and supports
debug-session memory inspection through injected `tracemalloc`/`gc` scripts. The LSP owns
sampling and aggregation; VS Code owns DAP injection and rich presentation.

## UI availability {#PROFILE-UI-GATE}

Profiling entry points are available without a feature-enable context key. Their visibility
is gated only by real server/debug/profiling state. Manifest tests reject the removed
`basilisk.profilingEnabled` gate.

## Sampling API {#PROFILE-API}

External CPU attach embeds the `py-spy` Rust crate. `ProfileSessionManager` allows one active
session per PID, a sampler thread feeds stack traces through a channel, `ProfileData`
aggregates them, and stop/snapshot produces diagnostics and export artifacts. No comparative
profiler or overhead claim is part of this contract.

## CPU protocol {#PROFILE-PROTOCOL}

### Start {#PROFILE-REQUESTS-START}

`basilisk.profiler.start` requires `pid` and accepts optional `sampleRate`, `includeNative`,
`duration`, or a named preset. It returns `sessionId`, `pid`, `pythonVersion`, and
`startedAt`. A missing PID is rejected; discovery is explicit.

### Active debuggee PID {#PROFILE-SAME-PROCESS}

The LSP has no DAP connection, so the VS Code DAP proxy captures debugpy's
`systemProcessId` and passes that concrete PID to the profiler. The server has no hidden
debug-session-to-PID lookup.

### Stop {#PROFILE-REQUESTS-STOP}

`basilisk.profiler.stop` requires `sessionId` and accepts `speedscope`, `flamegraph`, or
`summary`. It stops sampling, publishes hotspot diagnostics, and returns duration, sample
count, hot functions/lines, artifact paths, and any export error.

### Snapshot {#PROFILE-REQUESTS-SNAPSHOT}

`basilisk.profiler.snapshot` returns the same current aggregation/export shape without
ending the session.

### List {#PROFILE-REQUESTS-LIST}

`basilisk.profiler.list` returns active sessions with ID, PID, Python version, start time,
sample count, and duration.

### Notifications {#PROFILE-NOTIFICATIONS}

#### Profiling diagnostics {#PROFILE-NOTIFICATIONS-DIAG}

Stop/snapshot publishes source `basilisk-profiler` diagnostics at `Hint` severity for hot
source locations. These do not count as checker errors or warnings.

#### Live progress {#PROFILE-NOTIFICATIONS-PROGRESS}

While a session exists, `basilisk/profiler/progress` reports session ID, sample count,
duration, and current top function once per second. The editor binds the handler to each LSP
client instance so restarts do not leave stale status UI.

### Progress UX {#PROFILE-UX-PROGRESS}

Long operations use the shared progress wrapper and status/panel starting states. Process
empty/error messages depend on both server state and fetch state; a failed or unfinished
request must not say that no Python processes exist.

### Memory-action discoverability {#PROFILE-MEMORY-DISCOVERY}

During Basilisk debug memory tracking, snapshot/compare/stop are reachable from the debug
toolbar and Python Processes view. Dashboard prompts and notifications route to the same
registered commands rather than describing palette-only actions.

## Process discovery {#PROFILE-PROCESSES}

### `basilisk.profiler.processes` {#PROFILE-PROCESSES-LSP}

The command reads the OS process table through `sysinfo` and returns `{processes:
ProcessInfo[]}`. Enumeration does not require elevation. Attachable processes sort by CPU
usage; non-attachable rows sort last.

### Scope {#PROFILE-PROCESSES-SCOPE}

Every detected Python process is returned. Workspace membership and debugger machinery are
display attributes, not filters. Only aggregate counts are logged; argv and usernames are
not logged.

### Process model {#PROFILE-PROCESSES-MODEL}

`ProcessInfo` contains PID/parent PID, name, interpreter path, script, Python version, CPU,
resident memory, runtime, user, elevation hint, workspace membership, launcher, attachable
flag, and an optional non-attachable reason. Python detection examines process name,
executable, and argv. Version probing is cached and bounded per enumeration.

### Python Processes view {#PROFILE-PROCESSES-PANEL}

VS Code renders a row for each process plus current-file CPU/memory launch entries. The
store, not the tree provider, owns visibility-scoped polling at
`basilisk.profiler.processRefreshMs`; manual refresh uses the same fetch. Stable IDs are
derived from PID.

#### Display {#PROFILE-PROCESSES-DISPLAY}

Rows show interpreter/script, PID, version, CPU, memory, launcher/workspace/elevation hints,
and why a row cannot attach. The active session gets a profiling marker. Debugger machinery
is shown but greyed and non-actionable.

#### Reactive session state {#PROFILE-PROCESSES-REACTIVE}

Central store signals own process data and CPU/memory session state. While a start or session
is active, launch controls are replaced or hidden and stop/session chrome is shown.

### Launch actions {#PROFILE-PROCESSES-LAUNCH}

An attachable row offers CPU profile and memory tracking. Handlers resolve an explicit tree
item first, then current selection, and otherwise show a warning; they do not guess a PID.

#### Current-file launch {#PROFILE-PROCESSES-LAUNCH-FILE}

Run & Profile CPU launches the active Python file under `basilisk-debug` with
`profileOnLaunch`; Run & Track Memory uses `stopOnEntry` plus
`memoryTrackOnLaunch`. Both are available from title/empty/pinned/command surfaces according
to state.

##### Profiling runs do not stop interactively {#PROFILE-LAUNCH-NOSTOP}

For a profile-on-launch session, the DAP proxy suppresses breakpoint and exception-stop
requests while preserving the entry pause needed for sampler injection. Ordinary debug
sessions are forwarded unchanged.

## Cooperative sampling {#PROFILE-COOPERATIVE}

For debug-launched macOS sessions, the editor injects a Python daemon sampler at the entry
pause. It writes framed JSONL stack ticks to a minted temporary file; the LSP tails complete
records through `cooperativeAttach` and feeds the same aggregator as py-spy. Stop uses a
sentinel and removes temporary files. This path cannot include native frames. External macOS
processes use the elevated helper; Linux/Windows debug launches use PID attach.

## Aggregation {#PROFILE-AGGREGATION}

### Data {#PROFILE-AGGREGATION-STRUCTS}

`ProfileData` stores per-file line hits, per-file function totals/self counts, total and
per-thread counts, deduplicated frames, thread names, and per-thread stacks/weights for
export.

### Logic {#PROFILE-AGGREGATION-LOGIC}

Each retained thread trace increments a distinct line/function at most once per sample;
recursion therefore cannot inflate a percentage above 100. The leaf increments self time,
while full root-first stacks remain available for flame charts. Idle traces are optional.

### Runtime filtering {#PROFILE-AGGREGATION-SCAFFOLD}

Anchored basename/path-segment checks remove runpy/debugpy/pydevd and Basilisk sampler frames
at the common ingest point. Machinery-only threads are dropped, while user paths that merely
contain those strings remain.

### Thresholds {#PROFILE-AGGREGATION-THRESHOLD}

Defaults are 1% for hot lines, 2% for hot functions, and 20 diagnostics per file.

## Speedscope export {#PROFILE-SPEEDSCOPE}

### Mapping {#PROFILE-SPEEDSCOPE-MAPPING}

Frames are deduplicated by function/file/line, leaf-first sampler frames are reversed to
root-first samples, threads become profiles, and each sample receives `1 / sampleRate`
weight.

### Validation {#PROFILE-SPEEDSCOPE-VALIDATE}

Export refuses zero samples, non-finite/negative weights, out-of-range frame indexes, and
mismatched samples/weights. The same exportability guard protects speedscope, flamegraph, and
V8 CPU profile output.

### Viewer delivery {#PROFILE-VIEWER-DELIVERY}

Local files are served temporarily from a tokenized loopback URL for speedscope import;
registrations expire and use no-store headers. Reveal-file/manual import remains available
when loopback delivery fails.

## Flamegraph SVG {#PROFILE-FLAMEGRAPH}

`inferno` renders a self-contained SVG from collapsed stacks. The results panel can inline an
SVG up to its size limit and offers the original interactive file externally; missing or
oversized SVG does not break summary results.

## Shared webview host {#PROFILE-WEBVIEW-HOST}

CPU results, memory dashboard, and retention graph share singleton panel lifecycle, a
nonce-gated CSP, `<`-safe JSON embedding, source-navigation routing, and theme variables.
Panels must not assemble separate unsafe HTML shells or register duplicate message handlers.

## Native VS Code profiles {#PROFILE-NATIVE}

CPU exports `.cpuprofile`; memory snapshots export `.heapprofile`. They are optional actions
from the Basilisk result views rather than the sole landing surface.

### Fallback {#PROFILE-NATIVE-FALLBACK}

Every result remains usable when VS Code's built-in profile editor is absent or refuses a
file: show the self-contained result panel, surface export errors, and offer reveal/open
actions. A resolved `vscode.open` call alone is not proof the profile rendered.

### Short programs {#PROFILE-SHORT-PROGRAM}

If samples exist but no sample lands in user code, the UI explains that the program ran too
briefly for sampling. No deterministic cProfile mode is implemented or promised here.

## Visualization {#PROFILE-VIS}

### Palette {#PROFILE-VIS-PALETTE}

CPU heat uses orange/amber accents, memory uses purple, leaks use red, and freed memory uses
green over editor theme variables. This is the shared implemented palette; no donut,
sunburst, GIL gauge, or PNG-export contract exists.

### Inline heat map {#PROFILE-VIS-HEATMAP}

Source decorations grade CPU lines by percentage and memory lines by allocation/leak data.
They coexist on separate decoration tracks and are controlled by the shipped inline-heat-map
setting.

## Memory profiling {#PROFILE-MEMORY}

### Editor-as-courier flow {#PROFILE-MEMORY-HOWTO}

The LSP returns a Python script, the VS Code DAP connection evaluates it in the paused
debuggee, and the editor posts its output to `basilisk.memory.ingest`. The LSP never owns or
proxies the DAP connection.

#### Large payloads {#PROFILE-MEMORY-COURIER}

Snapshot/diff payloads use minted temporary files where debugpy stdout would truncate them.
The editor reads and deletes the file, then sends the content to ingest. Paths are encoded as
Python string literals rather than interpolated raw.

#### Final snapshot {#PROFILE-MEMORY-FINAL}

Start arms an exit/signal hook that writes one final snapshot file. The editor finalizes only
the tracked debug session, polls briefly for the file, ingests it, and reports when no final
snapshot was produced. Runtime/debugger allocation frames are filtered before heap-profile
construction.

### Autopilot {#PROFILE-MEMORY-AUTOPILOT}

Autopilot reuses the same snapshot-then-diff courier path and updates existing decorations
and dashboards without opening a new native profile for every capture.

#### Pause capture {#PROFILE-MEMORY-AUTOPILOT-PAUSE}

With `autoSnapshotOnPause` enabled, a genuine pause in the tracked session triggers one
capture. Reentrancy and in-flight-operation guards ignore courier pauses and unrelated debug
sessions.

#### Interval capture {#PROFILE-MEMORY-AUTOPILOT-INTERVAL}

With `autoSnapshot` enabled, captures repeat every configured interval (default 30 seconds)
while tracking. The timer follows tracking state and is disposed with the session.

#### Leak actions {#PROFILE-MEMORY-LEAK-ACTIONS}

The first high-confidence growth or definite cycle per session can offer reference-graph and
garbage-collection actions without notifying repeatedly.

### Reference-type picker {#PROFILE-MEMORY-REFGRAPH-PICKER}

The picker combines document class symbols with common container builtins and an explicit
free-text type option.

### Commands {#PROFILE-MEMORY-COMMANDS}

`memory.start`, `snapshot`, `diff`, `references`, `objectsByType`, and `gcCollect` return the
script for courier execution (plus session metadata where applicable). `memory.stop` ends
tracking. Command names are advertised from the shared registry.

#### Ingest {#PROFILE-MEMORY-INGEST}

`basilisk.memory.ingest` accepts a memory session ID and raw output. Marker dispatch returns
one of `ack`, `snapshot`, `diff`, `gc`, `refs`, or `objects`, updates leak history, and
publishes applicable memory diagnostics. Unknown sessions or markerless payloads are errors.

### Retention graph {#PROFILE-MEMORY-VIS-REFGRAPH}

The shipped webview draws one static force layout. Node radius is logarithmic in size;
targets, shallow retainers, cycles, and other objects use distinct colors; edges may show
reference labels. It does not implement hover expansion, context navigation, animation, or
alternate layouts.

### Leak confidence {#PROFILE-MEMORY-CONFIDENCE}

One growth is Low; two consecutive growths or a single growth over 10 MiB is Medium; three
consecutive growths is High. A cycle containing `__del__` is Definite. A non-growing diff
resets the site's streak, and freed allocations contribute to freed/net totals only.

### Diagnostic codes {#PROFILE-MEMORY-CODES}

The emitted memory families are `BSK-MEM-ALLOC`, `BSK-MEM-GROWTH`, `BSK-MEM-LEAK`, and
`BSK-MEM-CYCLE`. There is no separate `BSK-MEM-UNCOLLECTABLE` code.

## Permissions {#PROFILE-PERMISSIONS}

### macOS {#PROFILE-PERMISSIONS-MACOS}

Debug-launched sessions prefer cooperative sampling. External-process attach uses the
privileged `basilisk-profiler-helper` path because task-port access may require elevation.

### Helper protocol {#PROFILE-HELPER-PROTOCOL}

The LSP and helper exchange newline-delimited JSON over a Unix socket using shared types from
`basilisk-profiler-protocol`: attach, attached, sample batches, stop/stopped, and errors.

#### Attach errors {#PROFILE-HELPER-PROTOCOL-ERRORS}

The helper sends a structured process-not-found, not-Python, permission-denied, or generic
attach-failed frame before exit. EOF/send/handshake failures fall back to reaped exit status
and stderr, so a privilege prompt cancellation or crash is not reported as a bare EOF.

### Helper socket lifecycle {#PROFILE-HELPER-SOCKET}

The LSP binds before spawning the helper, launches it without waiting for process completion,
accepts the stream, and forwards sample batches into the normal sampler channel. Cleanup
removes the socket and helper process.

### Linux {#PROFILE-PERMISSIONS-LINUX}

Permission depends on Yama/ptrace policy and process relationship. The implementation tries
eligible attaches and turns real `EPERM` failures into remedies; kernel-enforced restrictive
scopes are rejected early.

### Windows {#PROFILE-PERMISSIONS-WINDOWS}

Same-user processes normally use `ReadProcessMemory` without elevation; OS attach errors are
still surfaced through the common error mapping.

## Diagnostic configuration {#PROFILE-CONFIG-CODES}

The shipped VS Code keys are `sampleRate`, `includeNative`, `lineThreshold`,
`functionThreshold`, `maxDiagnosticsPerFile`, `showInlineHeatMap`, `profileOnLaunch`,
`processRefreshMs`, `preset`, `autoSnapshotOnPause`, `autoSnapshot`, and
`autoSnapshotInterval` under `basilisk.profiler`. CPU diagnostics emitted are
`BSK-PROF-LINE` and `BSK-PROF-FUNC`; `BSK-PROF-GIL` is a reserved constant, not emitted.

## Errors {#PROFILE-ERRORS}

Profiler JSON-RPC codes are `-32001` process not found/missing required target, `-32002` not
Python, `-32003` permission denied, `-32004` already profiling, `-32005` generic attach
failure, and `-32000` other session/export failure. Memory ingest uses its own invalid-session
and payload errors.
