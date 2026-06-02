---
layout: layouts/docs.njk
title: "Python Profiler — CPU Heatmaps and Memory Leak Detection"
description: Integrated Python profiling with Basilisk — CPU heatmaps, flamegraphs, memory leak detection, and reference graph visualization, all inside your editor.
keywords: basilisk, profiling, python, py-spy, flamegraph, heatmap, memory leak, tracemalloc, cpu profiler, vs code, cursor, windsurf, open vsx, zed, neovim
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Profiler
  order: 5
---

# Profiler

Basilisk includes a fully integrated Python profiler. CPU hotspot heatmaps appear inline on every line of your code, memory leaks are flagged as diagnostics, and flamegraphs open directly in your editor — without leaving your workspace.


## Overview

The Basilisk profiler combines two complementary engines:

- **CPU profiling** — powered by [py-spy](https://github.com/benfred/py-spy), a sampling profiler that attaches to a running Python process with no code changes required. Samples the call stack at a configurable rate (default: 100 Hz) and shows inline heat annotations on every hot line.
- **Memory profiling** — powered by Python's `tracemalloc` module, injected at runtime. Tracks allocations over time, diffs snapshots to find leaks, and walks reference graphs to identify retention chains.

Both engines are orchestrated through the Basilisk LSP server (Rust). The IDE extensions listen for LSP notifications and render results — no standalone CLI needed.


---

## CPU Profiling

### Starting a profiling session

**VS Code:**

1. Run the Python script or test you want to profile (it must be running)
2. Open the Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`)
3. Run **Basilisk: Start Profiling**
4. Select the target process from the list

**Zed:**

Use the slash command in the AI panel:

```
/profile
```

**Neovim:**

```vim
:BasiliskProfileStart
```


### Inline heatmap annotations

As samples accumulate, Basilisk annotates every hot line directly in the editor gutter. The heat palette uses four levels:

| Level | Threshold | Colour | What it means |
|-------|-----------|--------|---------------|
| Critical | ≥ 10% of CPU time | 🔴 `#e8500a` | Severe bottleneck — optimise first |
| Hot | ≥ 5% | 🟠 `#f97316` | Significant overhead |
| Warm | ≥ 2% | 🟡 `#fbbf24` | Moderate cost, worth reviewing |
| Cool | ≥ 1% | ⚫ `#4a5468` | Minor overhead |

Lines below 1% receive no annotation.


### Taking a snapshot

Capture a point-in-time profile while a session is running:

- **VS Code:** Command Palette → **Basilisk: Snapshot Profile**
- **Zed:** `/profsnapshot`
- **Neovim:** `:BasiliskProfileSnapshot`

Snapshots are saved to the `basilisk-profiles/` directory in your workspace as Speedscope-compatible JSON. You can open them in [Speedscope](https://www.speedscope.app) for deeper analysis.

### Stopping profiling

- **VS Code:** Command Palette → **Basilisk: Stop Profiling** (or click the status bar item)
- **Zed:** `/profstop`
- **Neovim:** `:BasiliskProfileStop`


### Flamegraph viewer

When you stop a session (or take a snapshot), Basilisk opens a flamegraph webview directly in VS Code.


The flamegraph viewer includes:

- **Summary cards** — total samples, duration, top function, peak thread count
- **Interactive flamegraph** — click a frame to zoom in; click the breadcrumb to zoom out
- **Top functions table** — sorted by CPU time, with file/line links that navigate to source
- **Export** — download the raw Speedscope JSON for external analysis

### Attaching to a debug session

You can profile a program that is already running under the Basilisk debugger:

- **VS Code:** Command Palette → **Basilisk: Attach Profiler to Debug Session**

This attaches py-spy to the debugpy-managed process, so you can set breakpoints and see profiling data simultaneously.

### Profile comparison (diff)

To compare two profiling sessions:

1. Take a snapshot of session A (save as baseline)
2. Make your change
3. Take a snapshot of session B
4. Run **Basilisk: Compare Profiles** and select both snapshots

The diff view highlights functions that got faster (green) or slower (red) between the two sessions.


---

## Memory Profiling

### Starting memory tracking

Memory tracking injects Python's `tracemalloc` module into the running process to record every allocation with its call stack.

- **VS Code:** Command Palette → **Basilisk: Start Memory Tracking**
- **Zed:** `/memleak`
- **Neovim:** `:BasiliskMemoryStart`


### Taking memory snapshots

Capture a snapshot of the current allocation state:

- **VS Code:** Command Palette → **Basilisk: Take Memory Snapshot**
- **Zed:** `/memsnap`

Snapshots are numbered sequentially (snapshot 1, snapshot 2, …). Take at least two snapshots to enable diff analysis.

### Finding memory leaks

Diff two snapshots to identify allocations that grew between them:

- **VS Code:** Command Palette → **Basilisk: Diff Memory Snapshots**
- **Zed:** `/memdiff`

Basilisk compares the snapshots and emits LSP diagnostics on the lines that allocated memory that was not freed. Diagnostics use the `BSK-PROF-MEM` code and appear as warnings in the Problems panel.


Leak confidence scoring:

| Badge | Score | Meaning |
|-------|-------|---------|
| **Definite** | 95–100% | Object is reachable only from a leaked root |
| **High** | 70–94% | Strong retention evidence |
| **Medium** | 40–69% | Possible leak, needs investigation |
| **Low** | < 40% | Suspicious growth but uncertain |

### Reference graph

The reference graph walks the Python object graph from a suspected leak root to show exactly what is retaining it in memory.

- **VS Code:** Command Palette → **Basilisk: Show Reference Graph**
- **Zed:** `/memrefs`


The graph is rendered as an interactive force-directed layout:

- **Node size** — proportional to the object's memory footprint
- **Node colour** — object type (dict, list, class instance, module, etc.)
- **Edge labels** — attribute name, index, or key that holds the reference
- **Cycles** — highlighted in red; cycles prevent garbage collection

Click any node to inspect its type, `repr()`, size, and outgoing references.

### Memory timeline

The memory dashboard shows allocation growth over time as a stacked area chart, broken down by object type.


### Stopping memory tracking

- **VS Code:** Command Palette → **Basilisk: Stop Memory Tracking**
- **Zed:** `/memstop`
- **Neovim:** `:BasiliskMemoryStop`

---

## Memory Dashboard

The memory dashboard aggregates all memory profiling views into a single panel:


Panels in the dashboard:

| Panel | Description |
|-------|-------------|
| **Timeline** | Heap growth over the session, stacked by object type |
| **Top allocators** | Functions responsible for the most live allocations |
| **Leak candidates** | Ranked list with confidence scores and allocation call stacks |
| **Reference graph** | Force-directed object graph for the selected leak candidate |

---

## Profiling Presets

Basilisk ships four profiling presets that trade coverage for overhead:

| Preset | Sample rate | Native frames | Memory | Overhead |
|--------|------------|---------------|--------|----------|
| `default` | 100 Hz | No | No | ~1% |
| `lightweight` | 25 Hz | No | No | <0.5% |
| `detailed` | 200 Hz | Yes | No | ~3% |
| `memory` | 50 Hz | No | Yes | ~2% |

Select a preset from the Basilisk settings panel or set it in `pyproject.toml`:

```toml
[tool.basilisk.profiler]
preset = "detailed"
```

---

## Configuration

All profiler settings live under `[tool.basilisk.profiler]` in `pyproject.toml`:

```toml
[tool.basilisk.profiler]
enabled = true
sample-rate = 100          # Hz — samples per second
include-native = false     # include C extension frames
line-threshold = 1.0       # minimum % to show line annotation
function-threshold = 2.0   # minimum % to show function in table
output-directory = "basilisk-profiles"
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enabled` | bool | `true` | Enable the profiler feature |
| `sample-rate` | int | `100` | Sampling frequency in Hz |
| `include-native` | bool | `false` | Include C extension and Rust frames |
| `line-threshold` | float | `1.0` | Minimum CPU % for line annotations |
| `function-threshold` | float | `2.0` | Minimum CPU % for table entries |
| `output-directory` | string | `"basilisk-profiles"` | Where snapshots are saved |

VS Code settings mirror these under `basilisk.profiler.*`:

```json
{
  "basilisk.profiler.sampleRate": 100,
  "basilisk.profiler.includeNative": false,
  "basilisk.profiler.lineThreshold": 1.0
}
```

---

## Platform requirements

### macOS

py-spy reads another process's memory via `task_for_pid`, which macOS restricts. What matters is **not** whether you own the process, but whether Basilisk launched it:

- **A process Basilisk launched** — for example a script started from a Basilisk debug session — is a child of the language server, so it can be profiled with **no elevation**.
- **Any other process** — including one you started yourself in a separate terminal under the same user account — is not a child, so macOS still requires elevation. (Unlike Windows, macOS has no "same user, no elevation" path.)

For that second case Basilisk ships a privileged helper binary (`basilisk-profiler-helper`). The first time you profile such a process, macOS prompts for your administrator password via a standard system dialog; the helper then runs as root, attaches to the target, and streams samples back to Basilisk over a local Unix socket. If you cancel the prompt, profiling stops with a clear error and no partial results.


### Linux

On Linux, `ptrace` access is governed by `/proc/sys/kernel/yama/ptrace_scope`:

| Scope | Meaning | Basilisk behaviour |
|-------|---------|-------------------|
| `0` | All processes | No elevation needed |
| `1` (default on many distros) | Parent/child only | Basilisk launches a child shim |
| `2` | Admin only | Requires `sudo` |
| `3` | Disabled | Profiling unavailable |

If you see a permission error on Linux, check:

```bash
cat /proc/sys/kernel/yama/ptrace_scope
```

To temporarily allow profiling:

```bash
sudo sysctl kernel.yama.ptrace_scope=0
```

### Windows

No elevation is required on Windows. The profiler attaches using standard Win32 APIs available to any process owner.

---

## Diagnostic codes

| Code | Severity | Description |
|------|----------|-------------|
| `BSK-PROF-LINE` | Warning | Line spends ≥ `line-threshold`% of CPU time |
| `BSK-PROF-FUNC` | Warning | Function spends ≥ `function-threshold`% of CPU time |
| `BSK-PROF-GIL` | Warning | Thread blocked on GIL ≥ 20% of the time |
| `BSK-PROF-MEM` | Warning | Memory allocation that was not freed between snapshots |

These diagnostics appear in the Problems panel alongside type errors. They are suppressed when no profiling session is active.

---

## Profiling in Zed

The Zed extension surfaces profiling via slash commands in the AI assistant panel. There is no webview in Zed; instead, flamegraph data is exported to the `basilisk-profiles/` directory and diagnostics appear inline in the editor.

| Command | Action |
|---------|--------|
| `/profile` | Start CPU profiling |
| `/profstop` | Stop CPU profiling |
| `/profsnapshot` | Take CPU snapshot |
| `/memleak` | Start memory tracking |
| `/memstop` | Stop memory tracking |
| `/memrefs` | Show reference graph for top leak |


---

## Profiling in Neovim

The Neovim plugin exposes profiling via user commands:

| Command | Action |
|---------|--------|
| `:BasiliskProfileStart` | Start CPU profiling |
| `:BasiliskProfileStop` | Stop CPU profiling |
| `:BasiliskProfileSnapshot` | Take CPU snapshot |
| `:BasiliskMemoryStart` | Start memory tracking |
| `:BasiliskMemoryStop` | Stop memory tracking |

Inline heatmap annotations appear as virtual text in the sign column.


---

## Architecture

```
┌──────────────┐    LSP (JSON-RPC)    ┌──────────────────────────────┐
│  VS Code /   │◄────────────────────►│  Basilisk LSP (Rust)         │
│  Zed / Nvim  │                      │  ┌──────────────────────────┐ │
└──────────────┘                      │  │  ProfileSessionManager   │ │
                                      │  │  Sampler thread (py-spy) │ │
                                      │  │  SampleAggregator        │ │
                                      │  │  SpeedscopeExporter      │ │
                                      │  │  MemorySessionManager    │ │
                                      │  │  SnapshotDiffer          │ │
                                      │  │  ReferenceGraphWalker    │ │
                                      │  └──────────────────────────┘ │
                                      └──────────────┬───────────────┘
                                                     │ attaches to
                                                     ▼
                                         ┌──────────────────┐
                                         │  Python process  │
                                         │  (your code)     │
                                         └──────────────────┘
```

The LSP server owns all profiling state. Editor extensions are pure UI — they send commands and render what the LSP sends back via notifications. This means profiling works identically in VS Code, Zed, and Neovim.

---

## Performance targets

The profiler is designed to be low-overhead by default:

| Operation | Target |
|-----------|--------|
| Sampling overhead | < 3% CPU |
| LSP memory for 10-min session | < 50 MB |
| Diagnostics generation | < 100 ms |
| Speedscope export | < 200 ms |
| Flamegraph SVG render | < 500 ms |

---

## Troubleshooting

### "Process not found"

The target process exited before the profiler could attach. Make sure your script is still running before starting a profiling session.

### "Not a Python process"

py-spy only works with CPython. PyPy and other interpreters are not supported.

### "Permission denied" (macOS)

Click **Allow** on the system dialog that appears, or run VS Code/Zed with administrator privileges. See [Platform requirements](#platform-requirements).

### "Permission denied" (Linux)

Check `ptrace_scope` — see [Linux section](#linux) above.

### "Already profiling"

Only one CPU profiling session can run at a time. Stop the current session first.

### Snapshots not appearing

Ensure `output-directory` is writable. By default this is `basilisk-profiles/` relative to your workspace root.

---

## Comparison with other profilers

| Feature | Basilisk | cProfile | Scalene | Memray | Austin |
|---------|----------|---------|---------|--------|--------|
| Zero code changes | Yes | No | No | No | Yes |
| Inline editor annotations | Yes | No | No | No | No |
| Memory leak detection | Yes | No | Yes | Yes | No |
| Reference graph | Yes | No | No | No | No |
| IDE integration (native) | Yes | No | No | No | No |
| Flamegraph viewer | Yes | No | Yes | Yes | No |
| Profile on attach | Yes | No | No | No | Yes |
| GIL contention | Yes | No | Yes | No | Yes |

---

## Next steps

- [Configuration reference](/docs/configuration/) — full `pyproject.toml` schema including profiler settings
- [Debugging](/docs/debugging/) — use the integrated debugger alongside the profiler
- [Installation](/docs/installation/) — platform setup including privilege requirements
