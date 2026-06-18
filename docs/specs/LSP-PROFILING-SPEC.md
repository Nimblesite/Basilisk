# Basilisk Profiling — Specification {#LSPPROF}

## Goal {#PROFILE-GOAL}

Embed a state-of-the-art Python profiler directly into the Basilisk LSP. No `pip install`. No separate tool. One binary does type checking, debugging, and profiling. The profiler attaches to running Python processes, samples call stacks, and surfaces hotspots inline in the editor — VS Code and Zed.

## UI Availability Gate {#PROFILE-UI-GATE}

The profiling UI has **shipped**: now that the run→profile→view flow is reliable (#145 — the `.cpuprofile` no longer dead-ends, "Run & Profile" runs to completion, and short-program runs are surfaced honestly), its VS Code surfaces are enabled in every session.

A single switch, `isProfilingUiEnabled(context)` (`vscode-extension/src/profiling-ui.ts`), returns `true` unconditionally. It is kept (rather than deleted) as the one place the gate can be re-narrowed if ever needed: `extension.ts` mirrors it into the `basilisk.profilingEnabled` context key that every profiling `when` clause keys off, and `memory-profiler.ts` reads it for the one surface no `when` clause can reach (the memory status-bar item). All commands stay advertised ([PROFILE-REQUESTS]) and registered.

## Why py-spy {#PROFILE-PYSPY}

py-spy is a **Rust crate on crates.io**. Basilisk is Rust. This is the only Python profiler that can be embedded as a library dependency in a Rust project.

| Property | py-spy | Scalene | Memray | Austin |
|---|---|---|---|---|
| Language | **Rust** | Python/C++ | C++ | C |
| Embeddable as Rust crate | **Yes** | No | No | No |
| Attaches externally | **Yes** | No | No | Yes |
| Modifies target | **No** | Yes | Yes | No |
| Overhead | **~2%** | ~5-30% | High | ~2% |
| CPU / Memory profiling | **Yes** / No | Yes / Yes | No / Yes | Yes / No |

py-spy reads the target process's memory directly via OS calls (`vm_read` on macOS, `process_vm_readv` on Linux, `ReadProcessMemory` on Windows). It resolves the CPython interpreter state and walks `PyFrameObject` chains to build stack traces. Zero injection, zero instrumentation, zero overhead on the target.

## Architecture {#PROFILE-ARCH}

```mermaid
graph TB
    subgraph "Editor (VS Code / Zed)"
        UI[Editor — inline heat map, diagnostics]
        FLAMEGRAPH[Flamegraph Viewer — webview or browser]
        CMD[Commands — Start/Stop/Snapshot]
    end

    subgraph "basilisk lsp (Rust)"
        LSP_CORE[Language Server Core]
        PROF_MGR[ProfileSessionManager]
        SAMPLER[Sampler Thread — py-spy]
        AGGREGATOR[Sample Aggregator]
        EXPORT[Speedscope / Flamegraph Exporter]
        DIAG[Profiling Diagnostics Generator]
    end

    subgraph "Target"
        PYTHON[Python Process — unmodified]
    end

    CMD -->|"basilisk/profiler/start"| PROF_MGR
    CMD -->|"basilisk/profiler/stop"| PROF_MGR
    CMD -->|"basilisk/profiler/snapshot"| PROF_MGR
    PROF_MGR -->|"Spawns"| SAMPLER
    SAMPLER -->|"get_stack_traces()"| PYTHON
    SAMPLER -->|"Samples"| AGGREGATOR
    PROF_MGR -->|"On stop/snapshot"| EXPORT
    PROF_MGR -->|"On stop/snapshot"| DIAG
    EXPORT -->|"speedscope JSON"| FLAMEGRAPH
    DIAG -->|"publishDiagnostics"| UI
```

### Core Components {#PROFILE-COMPONENTS}

**`ProfileSessionManager`** — Owns active profiling sessions. One session per PID. Handles start/stop/snapshot lifecycle. Lives in the LSP server alongside `DebugSessionManager`.

**`Sampler` thread** — A dedicated OS thread per profiling session. Calls `py_spy::PythonSpy::get_stack_traces()` in a loop at the configured sample rate. Sends samples to the aggregator via `mpsc` channel.

**`SampleAggregator`** — Accumulates stack traces into a per-file, per-line hit count map. Tracks total samples, per-function samples, and per-line samples. Thread-safe (receives from channel, queried from LSP thread).

**`SpeedscopeExporter`** — Converts aggregated samples into speedscope JSON format.

**`ProfilingDiagnosticsGenerator`** — Converts aggregated samples into LSP diagnostics. Each hot line becomes a `Diagnostic` with severity `Hint` and a message like `"38.2% CPU (412 samples)"`. Publishes via `textDocument/publishDiagnostics`.

## py-spy Rust API {#PROFILE-API}

The profiler uses `py-spy` (crate version 0.4) for stack sampling. See [py-spy docs](https://github.com/benfred/py-spy) for the Rust API.

### Platform Permissions {#PROFILE-PERMS}

| Platform | Requirement | Impact |
|---|---|---|
| macOS | **Root required** (`vm_read` needs task port) | Must spawn privileged helper or use `sudo` |
| Linux | Root, or `ptrace_scope=0`, or profiling own child | Works without root if `ptrace_scope` is relaxed |
| Windows | No elevation for processes you own | Works out of the box |

**macOS mitigation**: For a process Basilisk launched (a child), the LSP already holds the child PID and traces it directly (parent traces child, no root). For any other process — *including a same-user process started in another terminal* — the LSP spawns a small helper binary (`basilisk-profiler-helper`) via `osascript` to get elevated privileges; the helper runs as root and streams samples back over a Unix socket. See [#PROFILE-PERMISSIONS-MACOS], [#PROFILE-HELPER-PROTOCOL], and [#PROFILE-HELPER-SOCKET].

## LSP Protocol {#PROFILE-PROTOCOL}

### Custom Requests {#PROFILE-REQUESTS}

#### basilisk/profiler/start {#PROFILE-REQUESTS-START}

Start profiling a Python process.

| Field | Type | Required | Description |
|---|---|---|---|
| `pid` | `number` | **Yes** | Target PID. The editor obtains it from [`basilisk.profiler.processes`](#PROFILE-PROCESSES-LSP) (the Python Processes panel) or, for the active debug session, from the captured debuggee PID (see [#PROFILE-SAME-PROCESS]). There is **no** silent auto-detect. |
| `sampleRate` | `number` | No | Samples per second (default: 100) |
| `includeNative` | `boolean` | No | Include C extension frames (default: false) |
| `duration` | `number` | No | Auto-stop after N seconds (default: null = manual stop) |

A missing `pid` is rejected with `-32001` — earlier revisions of this spec
claimed an "auto-detect when omitted", but none was ever implemented (#62). PID
discovery is now an explicit, user-visible step.

#### Profiling the debug session's process {#PROFILE-SAME-PROCESS}

The profiler and debugger **use the same process**. Because the LSP holds no DAP
connection, it never learns the debuggee's OS PID directly (it spawns
`debugpy.adapter`; debugpy spawns the debuggee later). Instead the editor captures
it: the DAP proxy (`vscode-extension/src/dap-proxy.ts`) intercepts debugpy's
`process` event (`body.systemProcessId`) and stores `sessionId → pid` in the
extension store; "Profile Debug Session" (`basilisk.profileAttachToDebug`) then
calls `basilisk.profiler.start` with that concrete `pid`. The LSP profiler stays
PID-based — **no server-side `debugSession`→PID resolution** — and the existing
privilege layer ([#PROFILE-PERMISSIONS]) routes the attach: child/same-user →
in-process py-spy (Linux/Windows), external/grandchild → elevated helper (macOS).

**Response fields:** `sessionId`, `pid`, `pythonVersion`, `startedAt`.

**Error codes:** `-32001` (process not found), `-32002` (not Python), `-32003` (permission denied), `-32004` (already profiling).

#### basilisk/profiler/stop {#PROFILE-REQUESTS-STOP}

Stop profiling and return results.

| Field | Type | Required | Description |
|---|---|---|---|
| `sessionId` | `string` | Yes | Session to stop |
| `format` | `string` | No | `"speedscope"` (default), `"flamegraph"` (SVG), `"summary"` (text) |

**Response fields:** `sessionId`, `duration`, `totalSamples`, `outputFile`, `flamegraphPath`, `cpuProfilePath`, `exportError`, `hotFunctions[]` (name, file, line, samples, percentage, selfPercentage), `hotLines[]` (file, line, samples, percentage).

- `flamegraphPath` — the local self-contained flamegraph SVG, always attempted
  regardless of `format` so every editor has a viewer that needs no network
  access ([PROFILE-FLAMEGRAPH]).
- `exportError` — set when any export was refused or failed
  ([PROFILE-SPEEDSCOPE-VALIDATE]); a failed export is never silent. Editors
  must surface it to the user.

#### basilisk/profiler/snapshot {#PROFILE-REQUESTS-SNAPSHOT}

Take a point-in-time snapshot without stopping the session. Same response as `stop`, but profiling continues.

#### basilisk/profiler/list {#PROFILE-REQUESTS-LIST}

List active profiling sessions. Returns `sessions[]` with `sessionId`, `pid`, `startedAt`, `sampleCount`, `duration`.

### LSP Notifications {#PROFILE-NOTIFICATIONS}

#### basilisk/profiler/diagnostics {#PROFILE-NOTIFICATIONS-DIAG}

After `stop` or `snapshot`, the LSP publishes profiling diagnostics for every file in the samples via `textDocument/publishDiagnostics`. Profiling diagnostics use severity `Hint` (4) and source `"basilisk-profiler"` so they don't pollute error/warning counts.

#### basilisk/profiler/progress {#PROFILE-NOTIFICATIONS-PROGRESS}

Periodic notification during active profiling with `sessionId`, `sampleCount`, `duration`, `topFunction`. Editors display this in a status indicator.

### Editor loading & progress states {#PROFILE-UX-PROGRESS}

No profiling action may be silent while it works. Every multi-second flow
shows a progress surface from click to outcome:

- **One notification per user action**, with live stage messages
  ("Waiting for the program to pause…", "Injecting the in-process sampler…",
  "Attaching…", "Collecting results…"). Covered flows: one-click CPU launch
  (both the cooperative and py-spy legs), per-row py-spy attach, profiler
  stop, and every memory operation (start, snapshot, compare, GC,
  reference graph). All progress goes through a single shared wrapper
  (`progress-ops.ts` in the VS Code extension) so styling, structured logs,
  and the e2e seam stay uniform — VS Code's progress UI is not readable via
  the public API, so the wrapper records a begin/step/end **operation log**
  that tests assert against.
- **Status bar "starting" state**: between the click and the first sample
  batch the profiler item shows `$(loading~spin)` with an explanatory
  tooltip, then flips to the live flame counter
  ([#PROFILE-NOTIFICATIONS-PROGRESS]). It never sits hidden while a start is
  in flight.
- **Panel loading states** ([#PROFILE-PROCESSES-PANEL]): manual Refresh runs
  under the view's progress bar; the auto-refresh poll stays silent. The
  empty state is gated on the `basilisk.serverState` context key: while the
  language server is starting it reads "Connecting to the Basilisk language
  server…" — the "No Python processes running" message (with its launch
  buttons) is only shown when the server is actually running and the list is
  truly empty.
- **Reactive session chrome** ([#PROFILE-PROCESSES-REACTIVE]): once a profile
  is starting or running, the Python Processes panel itself reflects it — a
  live message + badge, the launch buttons swapped for Stop, and the profiled
  row marked — so the panel is never offering "Run & Profile" mid-session.

## Process Enumeration & Selection {#PROFILE-PROCESSES}

Starting a profile must never require the user to hand-type a PID (#62). The LSP
owns process **discovery**; editors only render it. This section defines the
enumeration command, its data model, and the panel/launch UX that replaces the
old raw PID input box. Design + phased TODO: [LSP-PROFILER-PROCESS-PANEL-PLAN.md](../plans/LSP-PROFILER-PROCESS-PANEL-PLAN.md) `{#PROFPANEL-PLAN}`.

### basilisk.profiler.processes {#PROFILE-PROCESSES-LSP}

A `workspace/executeCommand` request that returns every attachable Python
process. It takes no required arguments and responds with
`{ "processes": ProcessInfo[] }`, sorted by CPU usage descending.

Enumeration **only reads the OS process table** and therefore never requires
elevation — discovery works without `sudo`, which is the whole point. It is
implemented in [`processes.rs`](../../crates/basilisk-lsp/src/profiler/processes.rs)
over the `sysinfo` crate and is advertised in `executeCommandProvider` like every
other Basilisk command (editors must not pre-register it — see
[LSP-ARCHITECTURE-SPEC.md] command registration rule).

### ProcessInfo {#PROFILE-PROCESSES-MODEL}

Each entry in the `processes[]` response:

| Field (JSON) | Type | Notes |
|---|---|---|
| `pid` | number | Process id |
| `ppid` | number | Parent pid (`0` if unknown) — enables "group by parent" |
| `name` | string | Process name, e.g. `python3.12` |
| `interpreterPath` | string \| null | Resolved interpreter executable path |
| `script` | string \| null | Best-effort target script (first positional arg) |
| `pythonVersion` | string \| null | e.g. `3.12.13`; `null` ⇒ render `—` |
| `cpuPercent` | number | Instantaneous CPU% (may exceed 100 across cores) |
| `memoryBytes` | number | Resident memory in bytes |
| `runtimeSecs` | number | Seconds since process start |
| `user` | string \| null | Owner login name |
| `requiresElevation` | boolean | `true` if not owned by the current user |
| `kind` | `"interpreter"` \| `"launcher"` | Bare interpreter vs. launcher |

**Detection:** a process is "Python" when its name, interpreter exe basename, or
`argv[0]` basename matches `python`, `python3`, `pythonX.Y`, or `pypy`. Known
launchers (uvicorn, gunicorn, pytest, celery, flask, hypercorn, daphne, uwsgi,
sanic) running on a Python interpreter are included and tagged `kind = "launcher"`
so they are still offered for profiling rather than hidden.

**Exclusions:** debugger machinery is never offered as a target —
`python -m debugpy.adapter`/`pydevd` and scripts inside `debugpy`/`pydevd`
package directories are filtered out. Profiling the adapter instead of the
debuggee is always a mistake, and adapters orphaned by a hard-killed editor
would otherwise linger in the panel as phantom rows.

**macOS argv:** sysinfo cannot read other processes' argv on macOS, so the
enumerator takes one batched `ps -axo pid=,args=` snapshot per enumeration as
a best-effort fallback — this is what powers script labels, launcher
detection, and the exclusions there.

**Version resolution:** `pythonVersion` is resolved server-side — an exact
version from `<exe> --version` (cached per interpreter, bounded per enumeration),
falling back to the `pythonX.Y` path pattern, then `null`.

**`requiresElevation`** is a *hint* for the panel's lock badge; the authoritative
permission check still happens at attach time (see [#PROFILE-PERMISSIONS]).

**Logging:** only the process *count* is logged. Command lines and user names may
contain secrets/PII and are never logged (CLAUDE.md logging standards).

### basilisk/profiler/processesChanged {#PROFILE-PROCESSES-NOTIFY}

Reserved notification for pushing lazily-resolved interpreter versions to the
editor after an enumeration returned them as `null`. v1 resolves versions inline
within the request's resolution budget, so this notification is currently
optional; editors treat its absence as "versions are already final".

### Python Processes panel {#PROFILE-PROCESSES-PANEL}

VS Code contributes a `basilisk.pythonProcesses` tree view in the
`basilisk-explorer` activity-bar container, implemented in
[`process-explorer.ts`](../../vscode-extension/src/process-explorer.ts). It calls
`basilisk.profiler.processes` and renders one row per process:

- **label** `python3.12 — app.py` · **description** `PID 82875 · 3.12.13 · 12.4% · 88 MB`
- **tooltip** interpreter path, script, user, runtime, elevation note
- **icon** a Python glyph, with a `$(lock)` badge when `requiresElevation`

Auto-refresh is gated on view visibility (interval from
`basilisk.profiler.processRefreshMs`, default 2000); a manual refresh button is
always present. Process rows carry a stable `TreeItem.id` (`pythonProcess:<pid>`)
so VS Code can map inline-button clicks back to elements across refreshes
(issue #79). An empty state (`viewsWelcome`) offers the two metric-explicit
launches described in [#PROFILE-PROCESSES-LAUNCH-FILE].

#### Sort modes {#PROFILE-PROCESSES-PANEL-SORT}

CPU% (default, descending), Memory, PID, Name, Runtime, Python version.

#### Group modes {#PROFILE-PROCESSES-PANEL-GROUP}

None (flat), Python version, Interpreter, User, Parent process. Groups render as
collapsible parent nodes with a count badge.

#### Reactive session state {#PROFILE-PROCESSES-REACTIVE}

The panel is **reactive to the profiling session**, not a static list of
launchers. CPU and memory session state is the single reactive `profiler`
signal owned by the store (`profiler-state.ts`); the status bar, the panel
chrome, and the gating context keys all derive from it, so nothing on screen
goes stale (CLAUDE.md: "All state that can change uses Signals for reactivity").
The state machine per metric is `idle → starting → active → idle`.

One `effect` over the signal (`process-reactivity.ts`) drives the panel:

- **Live chrome.** The view shows a message above the tree and a badge dot
  while busy: `⏳ Starting CPU profiler…`, then `🔥 Profiling PID 1234 ·
  12.3K samples (4s) · hot_function` updated live from
  [#PROFILE-NOTIFICATIONS-PROGRESS]; `⏳ Starting memory tracking…` /
  `🗄️ Tracking memory allocations…` for the memory leg. The sample-count tick
  repaints only the message (cheap); the tree rebuilds only on a *gating*
  transition.
- **Button gating.** Four context keys flow from the effect:
  `basilisk.profilerBusy` (any activity starting or running),
  `basilisk.profiling` (CPU active), `basilisk.memoryTracking` (memory active),
  and `basilisk.profilerStarting`. While `profilerBusy`, the title-bar
  "Run & Profile CPU" / "Run & Track Memory" launches and the per-row
  Profile/Track actions are **hidden** — a session can no longer be started on
  top of a running one. In their place the title bar shows **Stop Profiling**
  (when `profiling`) or **Stop Memory Tracking** (when `memoryTracking`).
- **Active-row marker.** The row whose PID is being CPU-profiled renders with a
  flame icon, a "· profiling" suffix, and `contextValue = pythonProcessProfiling`,
  which swaps its inline Profile button for an inline **Stop**.

The launch commands also guard imperatively (`profileCurrentFile`,
`startProfilingForPid`, `handleProfileAttachToDebug`, `handleMemoryStart`): even
if invoked from the palette while busy, they decline with a "stop the current
session first" message instead of spawning a second session. The e2e seams are
the pure `panelMessage`/`panelBadge` builders plus `pythonProcessesViewState()`
(reads the live view chrome) and `profilerStatusText()` (reads the status bar).

### Launch from the panel {#PROFILE-PROCESSES-LAUNCH}

This is the headline fix for #62. Per-row inline buttons act on that row's
`pid` in one click, **with no input box**:

- **▶ Profile CPU** (`basilisk.profileProcess`) starts a CPU sampling session
  for the row's PID.
- **🧠 Track Memory** (`basilisk.memoryTrackProcess`) routes honestly: memory
  tracking rides the DAP-`evaluate` courier ([#PROFILE-MEMORY-HOWTO]), so it
  can only target the **live Basilisk debuggee**. When the row's PID is the
  active `basilisk-debug` debuggee it runs `basilisk.memoryStart`; for any
  other process it must **never** fall back to a CPU session — it explains
  the constraint and offers the "Run & Track Memory (Current File)" launch
  ([#PROFILE-PROCESSES-LAUNCH-FILE]). The routing decision is
  `memoryTrackRoute` in `process-launch.ts`.

The row context menu adds Copy PID and Reveal Script. The old palette command
`basilisk.profileStart` is **kept but rewritten**: instead of prompting for a PID
it focuses this panel and shows a toast ("Pick a process below"). The lying
"auto-detect" prompt is deleted.

VS Code can invoke an inline tree command with **no argument** (issue #79 —
e.g. a click racing the panel's auto-refresh), so the handlers resolve their
target as: explicit item → the panel's current selection → only then a warning
(`createProcessRowActions` in `process-launch.ts`). Both row commands share
this resolution.

#### Run & profile the current file {#PROFILE-PROCESSES-LAUNCH-FILE}

The view-title entry point must state **what it tracks** (issue #82 — the old
single "Run & Profile Current File" `$(run-all)` button named no metric). Two
metric-explicit buttons mirror the per-row actions, same labels and icons:

- **🔥 Run & Profile CPU (Current File)** (`basilisk.profileCurrentFileCpu`) —
  launches the active `.py` under `basilisk-debug` with `profileOnLaunch: true`;
  profiler.ts honours that launch-config flag (or the global
  `basilisk.profiler.profileOnLaunch` setting) and attaches the CPU profiler to
  the captured debuggee PID ([#PROFILE-SAME-PROCESS]).
- **🗄️ Run & Track Memory (Current File)** (`basilisk.trackMemoryCurrentFile`) —
  launches with `stopOnEntry: true` + `memoryTrackOnLaunch: true`; tracemalloc
  needs a paused debuggee ([#PROFILE-MEMORY-HOWTO]), so memory-profiler.ts
  starts tracking at the entry pause and then resumes the program. Because this
  run has no breakpoint, the start script also arms an at-exit snapshot so the
  run finalises into a visible result on session end rather than dead-ending
  ([#PROFILE-MEMORY-FINAL]).

Both appear in the title bar, the panel's empty state, and (gated on
[#PROFILE-UI-GATE]) the command palette.

##### Profiling runs complete; they do not stop interactively {#PROFILE-LAUNCH-NOSTOP}

A "Run & Profile" launch is a *profiling run*, not a debug session: it must run
to completion and surface a profile, never halt at the user's breakpoints or
exception stops (#145). It **cannot** simply set the DAP `noDebug` flag —
debugpy then runs the program with no adapter at all, so `stopOnEntry` never
fires and the macOS cooperative sampler ([#PROFILE-COOPERATIVE]) loses the entry
pause it injects at.

Instead the DAP proxy (`dap-proxy.ts`) neutralises breakpoints for the session:
on the `launch` request it records `profileOnLaunch`, and for that session
rewrites every `setBreakpoints` / `setFunctionBreakpoints` to an empty
`breakpoints` array and every `setExceptionBreakpoints` to empty
`filters`/`filterOptions` before forwarding to debugpy. `stopOnEntry` is a launch
argument, not a breakpoint, so the entry pause (and the cooperative injection) is
preserved. Normal debug sessions (`profileOnLaunch` unset) forward untouched. The
pure transformation is `suppressBreakpointsForProfiling`; the DAP order
guarantees `launch` reaches the proxy before `setBreakpoints`, so the flag is
known in time.

Both triggers of [#PROFILE-PROCESSES-LAUNCH-FILE] reach this guard: the explicit
"Run & Profile CPU" entry point sets `profileOnLaunch` directly, and when the
global `basilisk.profiler.profileOnLaunch` setting is on, the config resolver
(`applyDebugConfigDefaults` in `debug-adapter.ts`) stamps `profileOnLaunch: true`
onto every resolved `basilisk-debug` launch — so the proxy sees the flag in the
launch arguments for both. This keeps the proxy's suppression predicate in lock
step with `shouldProfileOnLaunch`'s two equivalent triggers.

## Cooperative In-Process Sampling {#PROFILE-COOPERATIVE}

The out-of-the-box CPU path for **debug-launched** sessions. Modern macOS
gates task ports behind signed, debugger-entitled callers — even root +
py-spy gets `EPERM` — so for sessions the editor launches under debugpy,
Basilisk samples **from inside the debuggee** instead of reading foreign
memory:

1. The launch config sets `stopOnEntry` (macOS only; see
   [#PROFILE-PROCESSES-LAUNCH-FILE]).
2. `basilisk.profiler.cooperativeScript` (leg 1) mints a sample-file path and
   returns a Python script; the editor evaluates it at the entry pause via
   the same courier as memory profiling ([#PROFILE-MEMORY-HOWTO]), then
   resumes the program. The script starts a **daemon thread** that walks
   `sys._current_frames()` at the configured rate and appends one JSONL tick
   record per sample to the file (header first:
   `{"header":{"python":…,"pid":…}}`, then
   `{"ticks":[[threadId,active,frames…]]}` with leaf-first frames, matching
   py-spy). Leading debugpy/pydevd frames (the `sys.settrace` callbacks that
   sit on top of the traced user frame) are stripped so tracer overhead is
   attributed to the user line being traced; threads whose remaining leaf
   sits in stdlib wait modules are marked idle.
3. `basilisk.profiler.cooperativeAttach` (leg 2) tails the file as a standard
   `SamplerHandle` (`cooperative.rs`) — handshaking on the header exactly like
   the elevated helper does on `attached` — and registers a normal session,
   so aggregation, hotspots, exports, diagnostics, and live progress
   ([#PROFILE-NOTIFICATIONS-PROGRESS]) are reused unchanged. The response
   matches `profiler.start`.
4. Stop writes a `<file>.stop` sentinel; the injected thread exits, the
   tailer drains what was flushed (0.5 s flush cadence) and removes both
   files.

Platform routing: macOS launch flow → cooperative; Linux/Windows launch flow
→ py-spy attach to the captured debuggee PID ([#PROFILE-SAME-PROCESS]);
external-process attach on macOS still requires the elevated helper
([#PROFILE-PERMISSIONS-MACOS]). Trade-off: no native (C-extension) frames
and GIL ownership is not observed — acceptable for the launch flow, where
zero-setup beats fidelity.

## Sample Aggregation {#PROFILE-AGGREGATION}

### Data Structures {#PROFILE-AGGREGATION-STRUCTS}

```rust
/// Accumulated profiling data for a single session
struct ProfileData {
    /// file path -> line number -> sample count
    line_hits: HashMap<String, HashMap<i32, u64>>,
    /// file path -> function name -> FunctionStats
    function_stats: HashMap<String, HashMap<String, FunctionStats>>,
    total_samples: u64,
    thread_samples: HashMap<u64, u64>,
    /// Raw frame list for speedscope export (frame index dedup)
    frame_index: HashMap<(String, String, i32), usize>,
    frames: Vec<SpeedscopeFrame>,
    /// Per-thread sample stacks (indices into frames)
    thread_stacks: HashMap<u64, Vec<Vec<usize>>>,
    thread_weights: HashMap<u64, Vec<f64>>,
}

struct FunctionStats {
    name: String,
    file: String,
    line: i32,
    /// Samples where this function appears anywhere in the stack
    total_samples: u64,
    /// Samples where this function is the leaf (top of stack)
    self_samples: u64,
}
```

### Aggregation Logic {#PROFILE-AGGREGATION-LOGIC}

For each `get_stack_traces()` call:

1. For each thread's `StackTrace`:
   - Skip if `!trace.active && !config.include_idle`
   - For each `Frame` in the stack: increment `line_hits` and `function_stats.total_samples`
   - The leaf frame (index 0) also gets `self_samples` incremented
   - Record the stack as frame indices for speedscope export
2. Increment `total_samples`

### Hotspot Threshold {#PROFILE-AGGREGATION-THRESHOLD}

Only lines/functions above a configurable threshold generate diagnostics:

- **Line threshold**: 1% of total samples (default)
- **Function threshold**: 2% of total samples (default)
- **Maximum diagnostics per file**: 20 (to avoid flooding)

## Speedscope Export {#PROFILE-SPEEDSCOPE}

### Mapping {#PROFILE-SPEEDSCOPE-MAPPING}

Output conforms to the [speedscope file format schema](https://www.speedscope.app/file-format-schema.json).

| py-spy | Speedscope |
|---|---|
| `Frame { name, filename, line }` | `shared.frames[i] { name, file, line }` |
| Each `get_stack_traces()` call | One entry in `samples` per thread |
| `1.0 / sampling_rate` | Each entry in `weights` |
| `StackTrace.thread_name` | `profiles[i].name` |
| Frames deduplicated by `(name, filename, line)` | Index into `shared.frames` |

Stacks in speedscope are root-first (callers before callees). py-spy returns leaf-first. Reverse the frame order when building `samples` entries.

### Export Validation {#PROFILE-SPEEDSCOPE-VALIDATE}

speedscope.app's importer indexes `shared.frames` by every sample entry, walks
parallel `samples`/`weights` arrays, and reads `profiles[activeProfileIndex]`.
A file violating any of those invariants loads as "Something went wrong" in
the browser. The exporter therefore **refuses to write** (returns an error
instead) when:

- the session captured **zero samples** (`profiles: []` with
  `activeProfileIndex: 0` is unloadable);
- any weight is **non-finite or negative** (serde serializes NaN/∞ as `null`,
  which the importer rejects);
- any sample's **frame index is out of bounds** for `shared.frames`;
- a thread's `samples` and `weights` **lengths differ**.

The same validation guards the flamegraph SVG export **and the V8 `.cpuprofile`
export** (`export_cpuprofile` calls `validate_exportable`). The `.cpuprofile`
case matters because VS Code's built-in viewer crashes on a **zero-sample**
profile: its `buildModel` guard `if (!timeDeltas || !samples)` misfires (an empty
`samples` array is truthy in JS), then it reads `samples[timeDeltas.length - 1]`
— i.e. `samples[-1]` = `undefined` — and indexes a node that was never created,
throwing `Cannot read properties of undefined (reading 'selfTime')` and leaving
the user on the "could not be opened" error (#145). The exporter therefore
refuses to write a zero-sample `.cpuprofile`; with no `cpuProfilePath` the editor
falls back to the self-contained flamegraph ([PROFILE-NATIVE-FALLBACK]).

Tests assert the full invariant set on every exported file, not just key presence
(`profiler_tests.rs::assert_speedscope_loadable`,
`cpuprofile.rs::export_refuses_a_zero_sample_profile`).

### Viewer Delivery {#PROFILE-VIEWER-DELIVERY}

`https://www.speedscope.app/#profileURL=<url>` only works for http(s) URLs the
browser may fetch from that origin. An https page can **never** read
`file://` URLs, so editors must never construct a speedscope.app link to a
local file — it always fails with "Something went wrong". Until profiles are
served over localhost HTTP with CORS, editors open the local flamegraph SVG
(`flamegraphPath`) directly and tell the user where the speedscope JSON lives
for manual import (drag-and-drop at speedscope.app).

## Flamegraph SVG Export {#PROFILE-FLAMEGRAPH}

For direct SVG flamegraph output, use the `inferno` crate (Rust port of Brendan Gregg's FlameGraph). Convert aggregated stacks to collapsed format and pipe through `inferno::flamegraph::from_lines()`.

## Native VS Code profile files {#PROFILE-NATIVE}

Both profilers also emit **V8 profile files** that VS Code's built-in profile
viewer opens natively (flame chart + bottom-up/left-heavy tables) — the same UI
as Node.js profiling (see <https://code.visualstudio.com/docs/nodejs/profiling>).
The editor opens them with `vscode.open`; the custom flamegraph/dashboard
webviews remain as fallbacks.

### Never dead-end the user {#PROFILE-NATIVE-FALLBACK}

The built-in `.cpuprofile`/`.heapprofile` viewer is best-effort, not a
guarantee: it can be **unavailable** in the host (e.g. a dev/extension host with
the bundled `vscode-js-profile-*` viewer disabled) or **refuse to render** a
given profile, surfacing VS Code's "The editor could not be opened due to an
unexpected error" inside the tab. Critically, opening a custom editor via
`vscode.open` **resolves even when that editor later fails to render** — the
failure is contained in the tab and does not reject the command — so a completed
profile must never depend on the built-in viewer to be viewable (#145).

Therefore, on profile stop the editor:

- opens the native `.cpuprofile` beside the source when one was produced, and
  catches any `vscode.open` rejection, falling back to the self-contained
  flamegraph webview ([PROFILE-FLAMEGRAPH]); and
- **always** raises a completion notification that offers an **"Open Flame
  Chart"** action (the self-contained webview, which needs no external viewer)
  and a **"Reveal Trace File"** action (reveals the `.cpuprofile`, else the
  speedscope JSON). The "Profile complete — N samples" toast must carry these
  affordances, never announce a result the user has no way to reach.

`presentProfileResult` in `profiler-flamegraph-html.ts` owns this routing.

### Programs too short to sample {#PROFILE-SHORT-PROGRAM}

A sampling profiler takes one snapshot per `1/rate` seconds — at the default
100 Hz, one every 10 ms. A program that runs for a few milliseconds (e.g.
`examples/debug_demo.py` ≈ 1 ms over ~600 calls) therefore yields ~0 useful
samples, and **raising the rate cannot fix it**: the in-process sampler is a
pure-Python GIL-bound daemon, and `ingest_traces` stamps a fixed
`weight = 1/sample_rate`, so a sub-tick run is structurally un-sampleable and a
higher rate only distorts the measurement (#145).

**Phase 1 (current) — honest detection.** The signal is **attribution, not raw
sample count**: a sub-tick program finishes before its work can be sampled, yet
the session keeps sampling the idle/exiting interpreter, so a result can carry
dozens of samples (observed: 48) that resolve to **zero** hot functions and zero
hot lines. When a completed profile has no hot functions and no hot lines, the
editor does not open the empty flame chart/heat map; it shows a clear "captured
N samples, but none landed in your code — ran too briefly to profile by
sampling" message, never an action that promises a higher rate.
`profileHasNoUsableData` in `profiler-flamegraph-html.ts` gates this.

**Phase 2 (planned) — deterministic profiling.** The real fix for short programs
is a launch-only deterministic mode: inject `cProfile` at the `stopOnEntry`
pause via the existing courier ([PROFILE-COOPERATIVE]), dump `pstats` at exit via
the file courier ([PROFILE-MEMORY-HOWTO]), and ingest into `ProfileData`
(`ingest_pstats`, sibling of `ingest_traces`). `cProfile` counts every call
regardless of duration, so "too few samples" vanishes by construction.
Attach-to-PID stays sampling (no injection seam into a foreign process). This is
tracked separately and not yet implemented.

- **CPU → `.cpuprofile`** (`Profiler.Profile` schema):
  [`cpuprofile.rs`](../../crates/basilisk-lsp/src/profiler/cpuprofile.rs) merges the
  per-thread py-spy stacks into one call tree (`nodes` + `samples` + integer-µs
  `timeDeltas`, derived from the sample rate). Written on `profiler.stop`;
  the path is returned as `cpuProfilePath`.
- **Memory → `.heapprofile`** (`HeapProfiler.SamplingHeapProfile` schema):
  [`heapprofile.rs`](../../crates/basilisk-lsp/src/profiler/memory/heapprofile.rs)
  maps each `tracemalloc` site to a `head`-tree node with `selfSize`. Written on
  a snapshot ingest; the path is returned as `heapProfilePath`.

Line numbers are 0-based in V8; `url` is the source file path so the viewer can
navigate. `.heapsnapshot` is intentionally not produced (the built-in editor
doesn't render it).

## Visualization {#PROFILE-VIS}

### Brand Palette for Profiling {#PROFILE-VIS-PALETTE}

| Token | Hex | Usage |
|---|---|---|
| `--prof-critical` | `#e8500a` | >20% CPU — Basilisk orange |
| `--prof-hot` | `#f97316` | 10-20% CPU |
| `--prof-warm` | `#fbbf24` | 5-10% CPU |
| `--prof-cool` | `#4a5468` | 1-5% CPU |
| `--prof-idle` | `#1a1f2e` | <1% — background blend |
| `--prof-mem-critical` | `#c084fc` | Memory hotspot — purple |
| `--prof-mem-leak` | `#f87171` | Memory leak detected — red |
| `--prof-success` | `#34d399` | Freed / resolved — green |
| `--prof-bg` | `#0a0c12` | Panel background |
| `--prof-surface` | `#141820` | Card/chart background |
| `--prof-text` | `#f0f2f7` | Primary text |
| `--prof-text-secondary` | `#8892a4` | Secondary text |

### Typography {#PROFILE-VIS-TYPOGRAPHY}

- **Headings**: Space Grotesk 600
- **Labels / Data**: Space Grotesk 500
- **Code / Filenames**: JetBrains Mono 400
- **Numbers / Percentages**: JetBrains Mono 500

### Animation Principles {#PROFILE-VIS-ANIMATION}

- Entry: 200ms ease-out, charts fade in at 95%-100% scale, numbers count up from 0
- Transitions: 120ms ease for hover, 200ms ease for view switches
- Live updates: smooth interpolation, no jarring jumps
- Loading: pulsing Basilisk orange glow, no spinners

### Chart Components {#PROFILE-VIS-CHARTS}

All charts rendered in Canvas 2D (no heavy dependencies like d3).

**Flamegraph**: Frames colored by self-time percentage. Hover for tooltip, click to navigate to source, zoom to subtrees with breadcrumb trail, search to highlight matching frames.

**Donut Chart**: Top 5 functions by CPU %, center shows total sample count. Click a slice to filter flamegraph.

**Timeline**: Smooth bezier curves per function over time. Hover for crosshair, click+drag to zoom time range. Live mode extends rightward during active profiling.

**Sunburst Chart**: Radial layout with root at center. Arc width proportional to total time, color by self-time.

**Memory Leak Retention Graph**: Interactive force-directed graph of object references. Nodes sized by retained memory, cycles highlighted in red with pulsing animation.

**GIL Contention Gauge**: Animated arc gauge. Green (<10%), amber (10-30%), red (>30%). Real-time updates during live profiling.

### Inline Heat Map {#PROFILE-VIS-HEATMAP}

Hot lines get colored decorations in the editor gutter:

| Level | Color | Threshold |
|---|---|---|
| Critical | `#e8500a` Basilisk Orange | >20% |
| Hot | `#f97316` Light Orange | 10-20% |
| Warm | `#fbbf24` Amber | 5-10% |
| Cool | `#4a5468` Muted | 1-5% |

Memory profiling uses the purple palette for a separate decoration track showing allocation sizes and leak warnings.

### Profiler Dashboard {#PROFILE-VIS-DASHBOARD}

Full dashboard with summary cards (samples, duration, threads), donut chart, timeline, flamegraph, and hot functions table. All charts are interactive and cross-linked. Updates live during active profiling.

## Editor Integration {#PROFILE-EDITOR}

### VS Code {#PROFILE-EDITOR-VSCODE}

See [VSIX-SPEC.md](VSIX-SPEC.md) for VS Code-specific profiling UX.

**Commands:** `basilisk.profileStart`, `basilisk.profileStop`, `basilisk.profileSnapshot`, `basilisk.profileAttachToDebug`.

**Flamegraph Webview:** Full dashboard with all chart types, Basilisk design system, source navigation, export as PNG/SVG.

**Status Bar:** Shows profiling state with pulsing orange dot. Click to stop.

### Zed {#PROFILE-EDITOR-ZED}

See [ZED-SPEC.md](ZED-SPEC.md) for Zed-specific profiling UX.

Zed's limited extension API means profiling works through LSP diagnostics (hot lines as `Hint`) and slash commands (`/profile`, `/profstop`). Flamegraph via speedscope in browser.

### Shared Code {#PROFILE-EDITOR-SHARED}

| Component | Code Location | Used By |
|---|---|---|
| py-spy sampling | `basilisk-lsp/src/profiler/sampler.rs` | Both |
| Privilege check / sampler selection | `basilisk-lsp/src/profiler/privilege.rs` | Both |
| Elevated helper socket client | `basilisk-lsp/src/profiler/helper_client.rs` | Both |
| Elevated helper binary | `basilisk-profiler-helper/src/main.rs` | Both |
| Helper wire protocol | `basilisk-profiler-protocol/src/lib.rs` | Both |
| Sample aggregation | `basilisk-lsp/src/profiler/aggregator.rs` | Both |
| Speedscope export | `basilisk-lsp/src/profiler/export.rs` | Both |
| Flamegraph SVG | `basilisk-lsp/src/profiler/flamegraph.rs` | Both |
| LSP commands | `basilisk-lsp/src/profiler/commands.rs` | Both |
| Diagnostic generation | `basilisk-lsp/src/profiler/diagnostics.rs` | Both |
| Webview flamegraph | `vscode-extension/src/flamegraph/` | VS Code only |
| Slash command handler | `basilisk-zed/src/lib.rs` | Zed only |

100% of the profiler engine is shared. Editors differ only in visualization.

## Memory Profiling & Leak Detection {#PROFILE-MEMORY}

### Overview {#PROFILE-MEMORY-OVERVIEW}

Built on two engines:

1. **tracemalloc** (Python stdlib) — per-line allocation tracking, allocation flamegraphs, growth-over-time analysis
2. **gc + objgraph introspection** (Python stdlib + DAP evaluate) — reference graph walking, cycle detection, retention chain analysis, leak identification

Together they answer: **what allocated the memory, how much, and what's holding on to it.**

### Architecture {#PROFILE-MEMORY-ARCH}

```mermaid
graph TB
    subgraph "Editor"
        MEM_UI[Memory Dashboard]
        MEM_INLINE[Inline Decorations]
        MEM_GRAPH[Reference Graph]
    end

    subgraph "basilisk lsp (Rust)"
        MEM_MGR[MemorySessionManager]
        MEM_CMD[Memory LSP Commands]
        SNAPSHOT[Snapshot Differ]
        GRAPH_BUILDER[Reference Graph Builder]
        MEM_DIAG[Memory Diagnostics Generator]
    end

    subgraph "Python Process (via DAP evaluate)"
        TRACEMALLOC[tracemalloc]
        GC_MOD[gc module]
        OBJGRAPH_SCRIPT[Injected introspection script]
    end

    MEM_CMD -->|"Inject tracemalloc.start()"| TRACEMALLOC
    MEM_CMD -->|"Inject gc.get_referrers()"| GC_MOD
    MEM_CMD -->|"Inject walk_references()"| OBJGRAPH_SCRIPT
    TRACEMALLOC -->|"Snapshot data"| SNAPSHOT
    GC_MOD -->|"Reference chains"| GRAPH_BUILDER
    SNAPSHOT -->|"Deltas"| MEM_DIAG
    GRAPH_BUILDER -->|"Retention graph JSON"| MEM_UI
    MEM_DIAG -->|"publishDiagnostics"| MEM_INLINE
```

### How It Works — Editor-as-Courier Round-Trip {#PROFILE-MEMORY-HOWTO}

Memory profiling requires an active **debug session** (debugpy). Crucially, **the
LSP holds no DAP connection — the editor does** (the editor connects directly to
debugpy; see [LSP-DEBUG-INTEGRATION-SPEC]). So the LSP cannot inject Python
itself. Instead, memory analysis is a **two-leg round-trip with the editor as
courier**, and debugpy can only `evaluate` against a **stopped** frame. The
editor satisfies that invariant itself: when the program is running, memory
operations **transparently pause → evaluate → resume**
(`acquireStoppedFrame` in `dap-evaluate.ts`) — IDE-grade snapshots never
demand a manual breakpoint. A pause the *user* created (a real breakpoint
stop) is left untouched after the evaluation.

1. **Leg 1 — LSP → editor (get script):** A `basilisk.memory.*` command returns a
   Python injection script (e.g. `tracemalloc.take_snapshot()` printing a
   `__BASILISK_MEM__`-prefixed JSON payload). The LSP performs no DAP I/O.
2. **Editor runs the script** in the paused debuggee via a DAP `evaluate` request
   (`vscode-extension/src/dap-evaluate.ts`), capturing the printed marker output.
   **Pause detection must be event-tracked, never probed:** debugpy answers
   `stackTrace` for a *running* thread with a sampled frame whose id is not
   evaluable (`evaluate` then fails with "Unable to find thread for
   evaluation"), so `currentStoppedFrameId` only mints a frame id for threads
   the DAP tracker has seen `stopped` (and not since `continued`) — exactly how
   VS Code's own debug UI tracks pause state (`dap-output.ts`). Anything else
   returns null and surfaces the honest "Pause the debugger at a breakpoint"
   guidance.
3. **Leg 2 — editor → LSP (ingest):** The editor posts the raw output back via
   [`basilisk.memory.ingest`](#PROFILE-MEMORY-INGEST). The LSP marker-dispatches it
   to the matching parser, updates per-session state (the
   [`MemorySessionManager`](../../crates/basilisk-lsp/src/profiler/memory/session.rs)
   holds the cross-diff [`LeakTracker`] and timeline), **publishes memory
   diagnostics** via `textDocument/publishDiagnostics`, and returns the structured,
   `kind`-tagged result the editor renders (decorations, dashboard, reference graph).

The operations: **start tracking** (`tracemalloc.start(25)` + `gc.set_debug`),
**snapshots** (`__BASILISK_MEM__`), **diffs** (`__BASILISK_MEM_DIFF__`; lines that
grow across ≥3 consecutive diffs escalate to High confidence), **gc collect**
(`__BASILISK_MEM_GC__`), and **reference-graph walks** (`__BASILISK_MEM_REFS__`,
via `gc.get_referrers()` with cycle detection). The diff script self-seeds its
baseline (`tracemalloc._basilisk_prev_snapshot`) inside the debuggee, so
cross-snapshot baseline state lives in Python; the LSP keeps only leak-confidence
history and diagnostics.

This is identical for both editors — 100% of the engine is shared. Zed reaches the
same flow through `workspace/executeCommand`; only the script-running leg is
editor-specific.

#### Large payloads ride a temp file, not stdout {#PROFILE-MEMORY-COURIER}

debugpy truncates a single `print()` at ~20 KB, and a real snapshot easily
exceeds that (100 stats × depth-25 tracebacks of absolute paths ≈ 200 KB),
silently corrupting the JSON ("invalid snapshot JSON: expected `,` or `}`").
So every JSON-emitting script (`take_snapshot`, `diff_snapshot`,
`walk_references`, `objects_by_type`, `gc_collect`) does **not** print its
payload. It writes `marker + json` to a temp file
(`emit_via_file_helper` in [`scripts.rs`](../../crates/basilisk-lsp/src/profiler/memory/scripts.rs))
and prints only `__BASILISK_MEM_FILE__<path>` — a short, never-truncated line.
The editor's `resolveMarkerFilePayload` ([`dap-evaluate.ts`](../../vscode-extension/src/dap-evaluate.ts))
reads that file back (deleting it), yielding the full payload it posts to
`ingest` unchanged — so the LSP marker-dispatch (leg 3) is untouched. Local
debugging only: the editor and debuggee share a filesystem, exactly as the
cooperative CPU sampler ([#PROFILE-COOPERATIVE]) already assumes. Small acks
(`__BASILISK_MEM_OK__`, the CPU ack) still go straight over stdout.

#### Final snapshot on session end {#PROFILE-MEMORY-FINAL}

The "Run & Track Memory (Current File)" flow ([#PROFILE-PROCESSES-LAUNCH-FILE])
runs the program to completion with **no breakpoint**. Every other memory
operation needs a *paused* debuggee to `evaluate` against
([#PROFILE-MEMORY-HOWTO]); a run that finishes therefore leaves no frame to
snapshot from, and the old flow dead-ended — tracking started, the program
exited, and nothing (chart, trace, report) was ever shown (#146).

The fix mirrors the cooperative CPU sampler ([#PROFILE-COOPERATIVE]): capture to
a **file during the run, read it at the end**. When tracking starts,
`basilisk.memory.start` mints a per-session `finalSnapshotFile` path and returns
it alongside the script; the start script
([`start_tracemalloc`](../../crates/basilisk-lsp/src/profiler/memory/scripts.rs))
registers a Python `atexit` hook that takes one `tracemalloc` snapshot **as the
program exits** and writes it directly to that file. The payload is byte-identical
to an evaluate-path snapshot — both embed the shared `snapshot_payload_fn` — so it
ingests through the same [`basilisk.memory.ingest`](#PROFILE-MEMORY-INGEST) path
with no new parser. A direct in-process write (not the `_basilisk_emit` print
path) is used because at process exit there is no DAP `evaluate` round-trip
listening, and writing in-process sidesteps debugpy's print truncation entirely
([#PROFILE-MEMORY-COURIER]).

When the debug session terminates, `memory-profiler.ts`'s
`onDidTerminateDebugSession` listener finalises **only the session it is
tracking** — the store records the tracked `memoryDebugSessionId` at start, and
the listener matches the terminating session against it, so an unrelated debug
session ending in the same window never tears down live tracking. For the tracked
session it calls `finalizeMemorySessionOnEnd`: it settles the now-stale tracking
state ([#PROFILE-PROCESSES-REACTIVE]), reads the file (briefly polling for the
terminate-event/flush race, then deleting it), posts it to `ingest`, and presents
the snapshot exactly like a manual one — the purple allocation track plus the V8
`.heapprofile` in the built-in viewer ([#PROFILE-NATIVE]). The launch toast states
this up front ("a final snapshot opens automatically when the program finishes")
instead of pointing at a manual snapshot the user can never reach. **Stopping
never silently produces nothing:** if the `atexit` hook didn't run (a crash,
`os._exit`, or no live allocations), the editor says so explicitly rather than
clearing state in silence, and the manual `basilisk.memory.stop` likewise reports
whether a snapshot was captured. Stopping *mid-run* leaves the hook armed, so the
file it will still write at exit is scheduled for deletion on that session's
termination — a manual stop never orphans a temp file.

The injected path is embedded as a JSON-encoded Python string literal (the same
cross-platform-safe pattern as the cooperative sampler, [#PROFILE-COOPERATIVE]),
so a Windows backslash or a quote in `TMPDIR` cannot break the script.

Covered end-to-end by the "Run & Track Memory (Current File): the run finalises
into a visible memory result on session end (#146)" test (asserts the at-exit
snapshot paints the allocation track after a breakpoint-free run exits) and the
"an unrelated debug session ending does not tear down live memory tracking (#146)"
regression test, both in
[`memory-e2e.test.ts`](../../vscode-extension/src/test/suite/memory-e2e.test.ts).

### LSP Commands {#PROFILE-MEMORY-COMMANDS}

The `start`/`snapshot`/`diff`/`references`/`objectsByType`/`gcCollect` commands are
**leg 1** — they return `{ memorySessionId?, script }`. The editor runs the script
and posts the output to [`basilisk.memory.ingest`](#PROFILE-MEMORY-INGEST) (leg 2).

| Command | Request Fields | Leg-1 Response |
|---|---|---|
| `basilisk.memory.start` | `tracebackDepth` (default 25) | `memorySessionId`, `tracingStarted`, `script`, `finalSnapshotFile` ([#PROFILE-MEMORY-FINAL]) |
| `basilisk.memory.snapshot` | `memorySessionId` | `memorySessionId`, `script` |
| `basilisk.memory.diff` | `memorySessionId` | `memorySessionId`, `script` |
| `basilisk.memory.references` | `memorySessionId`, `targetType`, `targetReprContains`, `maxDepth`, `maxNodes` | `script` |
| `basilisk.memory.objectsByType` | `memorySessionId`, `typeName`, `limit` | `script` |
| `basilisk.memory.gcCollect` | `memorySessionId` | `script` |

#### basilisk.memory.ingest {#PROFILE-MEMORY-INGEST}

Leg 2 of the round-trip. Request: `{ memorySessionId, output }` where `output` is
the raw stdout of a script run in the debuggee. The
[`MemorySessionManager`](../../crates/basilisk-lsp/src/profiler/memory/session.rs)
detects the `__BASILISK_MEM*__` marker, parses with the existing parsers, scores
leaks via the per-session `LeakTracker`, publishes diagnostics, and returns a
`kind`-tagged object:

- `kind: "snapshot"` → `snapshotId`, `currentMemory`, `peakMemory`, `gcObjects`, `gcCounts`, `topAllocations[]`
- `kind: "diff"` → `totalGrowth`, `totalFreed`, `netGrowth`, `suspectedLeaks[]` (with `confidence`)
- `kind: "gc"` → `collected`, `uncollectable`, `memoryFreed`, `uncollectableObjects[]`
- `kind: "refs"` → `graph` with `nodes[]`, `edges[]`, `cycles[]`
- `kind: "objects"` → `objects` (`objects[]`, `totalCount`, `totalSize`, `typeSummary`)
- `kind: "ack"` → bare acknowledgment (start/stop scripts)

An unknown session or a marker-less payload is rejected with `-32010`.

### Reference Graph Visualization {#PROFILE-MEMORY-VIS-REFGRAPH}

The reference graph answers "what is holding on to this?" Force-directed layout with physics simulation:

- **Node sizing**: proportional to `log(size)`
- **Node coloring**: target objects in purple, root retainers in blue, intermediate containers in gray, cyclic objects in red with pulsing animation
- **Edge labels**: show reference type (`.attribute`, `['key']`, `[index]`)
- **Interactions**: hover for tooltip, click to expand referrers/referents, right-click to navigate to creation site
- **Cycle highlighting**: thick red edges, pulsing animation, banner explaining `__del__` implications
- **Layout modes**: force-directed (default), tree, radial

### Leak Confidence Scoring {#PROFILE-MEMORY-CONFIDENCE}

| Confidence | Criteria | Color |
|---|---|---|
| **Definite** | Object has `__del__` and is in a reference cycle (uncollectable) | Red, solid |
| **High** | Consistent growth across 3+ consecutive snapshot diffs | Red, dashed |
| **Medium** | Growth in 2 consecutive diffs, or >10 MB single-diff growth | Amber |
| **Low** | Single-diff growth, small size, possible cache warmup | Gray |

### Diagnostic Codes {#PROFILE-MEMORY-CODES}

| Code | Severity | Meaning |
|---|---|---|
| `BSK-MEM-ALLOC` | Hint | Top allocation site (above threshold) |
| `BSK-MEM-GROWTH` | Warning | Memory growth between snapshots |
| `BSK-MEM-LEAK` | Warning | Suspected memory leak (high confidence) |
| `BSK-MEM-CYCLE` | Error | Reference cycle with `__del__` — definite leak |
| `BSK-MEM-UNCOLLECTABLE` | Error | gc reports uncollectable object |

### CPU+Memory Integration {#PROFILE-MEMORY-CPU-INTEGRATION}

CPU and memory profiling can run simultaneously. Dashboard shows dual heat maps (orange CPU, purple memory), correlated flamegraphs, and a "Hot and Heavy" filter for functions that are both CPU-intensive and memory-intensive.

### Shared Code {#PROFILE-MEMORY-SHARED}

| Component | Code Location |
|---|---|
| tracemalloc / gc injection scripts (incl. reference-graph walker) | `basilisk-lsp/src/profiler/memory/scripts.rs` |
| Snapshot/diff/refs/objects parsers | `basilisk-lsp/src/profiler/memory/{mod,diff}.rs` |
| Leak confidence scoring | `basilisk-lsp/src/profiler/memory/leaks.rs` |
| Memory diagnostics | `basilisk-lsp/src/profiler/memory/diagnostics.rs` |
| Session state + marker-dispatched ingest | `basilisk-lsp/src/profiler/memory/session.rs` |
| LSP memory command handlers (incl. `ingest`) | `basilisk-lsp/src/server/memory_handlers.rs` |
| Editor DAP `evaluate` courier bridge | `vscode-extension/src/dap-evaluate.ts` (VS Code only) |
| Memory UI (decorations, dashboard, reference graph webview) | `vscode-extension/src/memory-profiler.ts`, `memory-decorations.ts` (VS Code only) |

## Permissions Model {#PROFILE-PERMISSIONS}

### macOS {#PROFILE-PERMISSIONS-MACOS}

`vm_read` (via `task_for_pid`) requires root, a child-process relationship with a non-hardened target, the `com.apple.security.get-task-allow` entitlement, or SIP disabled.

1. **Child-process profiling (no elevation):** A process Basilisk launched itself (e.g. a debug session) can be traced by its parent without elevation. This is the primary UX.
2. **External-process profiling (elevation required):** Any process Basilisk did **not** launch — **including a same-user process started in another terminal** — is not a child, so macOS still requires elevation. There is no "same-user, no-elevation" shortcut on macOS the way there is on Windows; do not message users as if there were (issue #61, Defect 4). The LSP spawns `basilisk-profiler-helper` via `osascript` with administrator privileges; the helper runs as root and streams samples back over a Unix domain socket.

The split is decided by `check_profiling_permissions` in `basilisk-lsp/src/profiler/privilege.rs`: a child PID yields `Allowed` (in-process py-spy), an external PID yields `ElevationRequired` (helper socket path), and a missing PID yields `Denied`.

### Helper Socket Protocol {#PROFILE-HELPER-PROTOCOL}

When elevation is required, the elevated helper and the LSP talk over a Unix domain socket using **newline-delimited JSON**. The message shapes and framing live in the shared `basilisk-profiler-protocol` crate so the two binaries can never drift; both `basilisk-lsp` and `basilisk-profiler-helper` depend on it.

```text
LSP    -> {"cmd":"attach","pid":12345,"rate":100,"native":false}
helper -> {"type":"attached","pid":12345,"python":"3.12.0"}
helper -> {"type":"samples","traces":[...]}        (repeating)
LSP    -> {"cmd":"stop"}
helper -> {"type":"stopped"}
```

On an attach failure the helper MUST report the cause over the socket before exiting (issue #81 — exiting silently leaves the LSP with an undiagnosable EOF):

```text
helper -> {"type":"attachfailed","pid":12345,"reason":"py-spy attach failed: ..."}
```

The LSP classifies the reason into an actionable error — target process exited, permission denied (elevation required), or the verbatim py-spy error — and, when an old helper still EOFs without reporting, harvests the helper's exit status into the error message (`helper_client::describe_helper_eof`).

`traces` carry the minimal per-thread / per-frame fields py-spy produces; the LSP converts them back into py-spy shapes and feeds the same aggregator the in-process sampler uses.

#### Attach-failure reporting {#PROFILE-HELPER-PROTOCOL-ERRORS}

A failed attach must never surface as a bare EOF (issue #81). Two layers guarantee a diagnosable cause:

1. **Structured error frame.** When py-spy attach fails, the helper sends
   `{"type":"error","kind":"<kind>","message":"<py-spy cause>"}` before exiting.
   `kind` is one of `process-not-found`, `not-python`, `permission-denied`,
   `attach-failed` (`AttachErrorKind` in `basilisk-profiler-protocol`), shared
   with the in-process sampler via `classify_attach_error` so both attach paths
   report identical failure modes. The helper refines py-spy's ambiguous
   "cannot open process" with a liveness probe: target alive ⇒
   `permission-denied`, target gone ⇒ `process-not-found`.
2. **Exit diagnosis fallback.** If the socket still EOFs (or the handshake/accept
   times out) before `attached`, the LSP reaps the helper (its stderr is piped at
   spawn) and appends its exit status plus trailing stderr to the error — this
   also surfaces `osascript` elevation failures such as the user cancelling the
   privilege prompt.

### Helper Socket Sampling {#PROFILE-HELPER-SOCKET}

The LSP side (`basilisk-lsp/src/profiler/helper_client.rs`) owns the socket lifecycle. The ordering is load-bearing — getting it wrong was the entirety of issue #61:

1. **Bind the `UnixListener` first**, before spawning the helper. (The original bug: nothing ever bound the socket, so the helper's `connect()` always failed with `No such file or directory (os error 2)`.)
2. **Spawn the helper detached** — `osascript`-elevated in production, or directly for tests — and never block on its exit (`.output().await` is wrong for a long-lived streamer).
3. **Guard the elevated command's working directory** with `cd /` so `do shell script ... with administrator privileges` cannot fail with `getcwd: cannot access parent directories`.
4. Accept the connection, drive `attach`/`samples`/`stop`, and forward batches into a `SamplerHandle` channel — identical to the in-process path from there on.

### Linux {#PROFILE-PERMISSIONS-LINUX}

Works without root if `ptrace_scope=0`. Under the default `ptrace_scope=1`
(restricted) the precheck **attempts the attach instead of denying upfront**:
Yama still grants *ancestors* (a debug session's debuggee is the LSP's
grandchild via `debugpy.adapter`) and targets that opted in via
`PR_SET_PTRACER`, neither of which the precheck can observe. A real `EPERM`
from py-spy is surfaced as a classified permission error
([#PROFILE-HELPER-PROTOCOL-ERRORS]) with the remedies appended:
`sudo sysctl kernel.yama.ptrace_scope=0`, `setcap cap_sys_ptrace+ep`, or
profiling via a debug session. Scopes `2`/`3` are kernel-enforced regardless
of process relationships and stay denied upfront with the matching remedy.

### Windows {#PROFILE-PERMISSIONS-WINDOWS}

`ReadProcessMemory` works without elevation for same-user processes.

## Configuration {#PROFILE-CONFIG}

```json
{
    "basilisk": {
        "profiler": {
            "enabled": true,
            "sampleRate": 100,
            "includeNative": false,
            "includeIdle": false,
            "hotLineThreshold": 1.0,
            "hotFunctionThreshold": 2.0,
            "maxDiagnosticsPerFile": 20,
            "outputDirectory": "/tmp",
            "autoOpenFlamegraph": true
        }
    }
}
```

### Diagnostic Codes {#PROFILE-CONFIG-CODES}

| Code | Severity | Meaning |
|---|---|---|
| `BSK-PROF-LINE` | Hint | Hot line (above threshold) |
| `BSK-PROF-FUNC` | Hint | Hot function (above threshold) |
| `BSK-PROF-GIL` | Info | GIL contention detected |

## Error Handling {#PROFILE-ERRORS}

| Scenario | Code | Recovery |
|---|---|---|
| PID not found | -32001 | User re-enters PID |
| Not a Python process | -32002 | User checks PID |
| Permission denied | -32003 | Elevation or debug mode |
| Already profiling | -32004 | Stop first, or snapshot |
| Process exited during profiling | N/A | Auto-stops, partial results returned |
| Unsupported Python version | -32005 | Upgrade to 3.3+ |

## Performance Targets {#PROFILE-PERF}

| Metric | Target |
|---|---|
| Sampling overhead on target | <3% CPU |
| LSP memory for 10-minute session at 100Hz | <50 MB |
| Time to generate diagnostics from 60K samples | <100ms |
| Speedscope export for 60K samples | <200ms |
| Flamegraph SVG for 60K samples | <500ms |

## Testing Strategy {#PROFILE-TESTING}

### Unit Tests {#PROFILE-TESTING-UNIT}

- `aggregator.rs`: Verify hit counts, function stats, threshold filtering
- `export.rs`: Verify speedscope JSON matches schema, frame deduplication, stack reversal
- `diagnostics.rs`: Verify diagnostic message format, severity, threshold filtering

### Integration Tests {#PROFILE-TESTING-INTEGRATION}

- Start a known Python script, attach profiler, verify hot function matches expected bottleneck
- Profile a debug session, verifying the debuggee PID captured from the DAP `process` event ([#PROFILE-SAME-PROCESS])
- Verify speedscope output opens correctly in speedscope.app
- Verify diagnostics appear for hot lines and disappear after clearing

### E2E Tests {#PROFILE-TESTING-E2E}

- **VS Code**: Command palette profile attach, debug session profiling, inline decorations
- **Zed**: `/profile` and `/profstop` slash commands, hint diagnostics

### Platform Tests {#PROFILE-TESTING-PLATFORM}

- macOS: privilege escalation prompt for external process, debug-session profiling without elevation
- Linux: ptrace_scope handling
- Windows: no-elevation profiling
