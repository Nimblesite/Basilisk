---
layout: layouts/docs.njk
title: "Python Profiler — CPU Heatmaps and Memory Leak Detection"
description: Integrated Python profiling with Basilisk — inline CPU heatmaps, a flame-graph results panel, and debugger-integrated memory leak detection, all inside your editor.
keywords: basilisk, profiling, python, py-spy, flamegraph, heatmap, memory leak, tracemalloc, cpu profiler, vs code, cursor, windsurf, open vsx, zed, neovim
date: 2026-02-28
dateModified: 2026-07-08
author: The Basilisk Project
eleventyNavigation:
  key: Profiler
  order: 6
---

# Profiler

Basilisk includes a fully integrated Python profiler. CPU hotspot heatmaps appear inline on your code, memory leaks are flagged as diagnostics, and profile results open in a flame-graph panel — without leaving your workspace.


## Overview

The Basilisk profiler combines two complementary engines:

- **CPU profiling** — sampling-based, with no code changes required. External processes are sampled by [py-spy](https://github.com/benfred/py-spy) (embedded in Basilisk as a Rust library); programs you launch through Basilisk's debugger on macOS are sampled by a cooperative in-process sampler instead, because modern macOS blocks external memory reads even for root (see [Platform requirements](#platform-requirements)). Default rate: 100 Hz.
- **Memory profiling** — powered by Python's `tracemalloc` and `gc` modules, driven **through an active Basilisk debug session**: the editor injects the instrumentation via the debug adapter, so memory tracking works on programs you are debugging (or launch with "Run & Track Memory"), not on arbitrary external processes.

Both engines live in the Basilisk LSP server (Rust). Editors call LSP commands (`basilisk.profiler.*`, `basilisk.memory.*` via `workspace/executeCommand`) and render what the server sends back — no standalone CLI, no `pip install`.


---

## CPU Profiling

### Starting a session (VS Code)

The **Python Processes panel** (Basilisk icon in the activity bar) is the home of profiling:

- **Run & Profile CPU (Current File)** — launches the active Python file and profiles it from the first line. The easiest way to profile a script end-to-end.
- **Profile CPU** — inline button on any row of the panel, which lists every running Python process on your system. No PIDs to hand-type.
- **Basilisk: Profile Debug Session** — attaches the profiler to the process the debugger is currently running, so breakpoints and profiling data work together.

The palette command **Basilisk: Start Profiling** doesn't guess a target — it focuses the panel so you pick one explicitly.

While a session runs, the status bar shows a live sample counter; click it (or run **Basilisk: Stop Profiling**) to finish.

### Inline heatmap annotations

As samples accumulate and when a session ends, hot lines get colored annotations directly in the editor:

| Level | Threshold | Colour |
|-------|-----------|--------|
| Critical | ≥ 20% of CPU time | 🔴 `#e8500a` |
| Hot | ≥ 10% | 🟠 `#f97316` |
| Warm | ≥ 5% | 🟡 `#fbbf24` |
| Cool | ≥ 1% | ⚫ `#4a5468` |

Lines below 1% receive no annotation. Hot lines and functions also appear in the Problems panel as **hint**-severity diagnostics (`BSK-PROF-LINE`, `BSK-PROF-FUNC`) with the measured percentage, so the profile is navigable from the same place as type errors.

### Taking a snapshot

**Basilisk: Take Profile Snapshot** captures the hotspots collected so far while the session keeps running — decorations and diagnostics update immediately, and the toast's **View Results** button opens the results panel. Snapshots are in-memory; files are written when the session stops.

### Results panel

Stopping a session opens the **Basilisk Profiler** panel beside your source:

- **Summary cards** — total samples, duration, hot function and hot line counts
- **Flame graph** — with an **Open Interactive Flame Graph** button for the zoom-and-search SVG
- **Hot functions / hot lines tables** — click any row to jump to the source line
- **Open Trace in VS Code Viewer** — opens the raw trace in VS Code's built-in profile viewer (the same flame chart and bottom-up tables used for [Node.js profiling](https://code.visualstudio.com/docs/nodejs/profiling))
- **Open in Speedscope (external)** — reveals the trace file and opens [speedscope.app](https://www.speedscope.app) so you can drag it in

Closed the panel? **Basilisk: Show Profile Results** re-opens it for the most recent profile.

On stop, Basilisk writes three artifacts to your system temp directory: `basilisk-<session>.speedscope.json`, `basilisk-<session>.flamegraph.svg`, and `basilisk-<session>.cpuprofile`.

### Programs too short to sample

A sampling profiler takes one snapshot per 10 ms (at 100 Hz). If your script finishes its work faster than that, no samples land in your code — and Basilisk says exactly that instead of showing an empty flame graph. Profile a longer run, or a loop that exercises the hot path repeatedly.

### Profiling presets

Set `basilisk.profiler.preset` in your editor settings:

| Preset | Sample rate | Duration |
|--------|------------|----------|
| `default` | your `sampleRate` setting (100 Hz) | until stopped |
| `quick` | 100 Hz | 10 s |
| `detailed` | 200 Hz | 60 s |
| `longRunning` | 50 Hz | until stopped |

---

## Memory Profiling

Memory tracking rides the debugger: Basilisk injects `tracemalloc` into the debuggee through the debug adapter, so **an active Basilisk debug session is required** (or use the launch flow, which creates one for you).

### Starting

- **Run & Track Memory (Current File)** — panel button; launches the active file with tracking armed from the first line. If the program runs to completion, a final snapshot is captured automatically as it exits — you always get a result.
- **Basilisk: Start Memory Tracking** — starts tracking in the debug session you're already in. Also one click away on the memory status-bar item whenever a Basilisk debug session is active.

While tracking, **Snapshot / Compare / Stop buttons sit directly on the debug toolbar**, next to Continue and Step.

### Snapshots and leak detection

- **Basilisk: Take Memory Snapshot** — records the current allocation state, paints per-line allocation decorations, and opens the native `.heapprofile` view (falling back to the Basilisk memory dashboard).
- **Basilisk: Compare Memory Snapshots** — diffs the latest two snapshots and emits leak diagnostics on the allocating lines.
- **Autopilot** — with `basilisk.profiler.autoSnapshotOnPause` (on by default), every debugger pause quietly captures a snapshot, and the diff surfaces when there's something to show. `basilisk.profiler.autoSnapshot` adds interval-based capture for long runs.

Leak confidence is criteria-based, not a guess:

| Badge | Criteria |
|-------|----------|
| **Definite** | Object with `__del__` participating in a reference cycle — uncollectable |
| **High** | Same line kept growing across 3+ consecutive snapshot diffs |
| **Medium** | Growth across 2 consecutive diffs, or a single diff over 10 MB |
| **Low** | Growth observed once — worth watching |

### Memory diagnostics

| Code | Severity | Meaning |
|------|----------|---------|
| `BSK-MEM-ALLOC` | Hint | Line is a top allocation site |
| `BSK-MEM-GROWTH` | Warning | Allocations at this line grew between snapshots |
| `BSK-MEM-LEAK` | Warning (Error when Definite) | Suspected leak with confidence badge |
| `BSK-MEM-CYCLE` | Error | Reference cycle that blocks garbage collection |

### Reference graph and dashboard

- **Basilisk: Show Reference Graph** — walks the object graph from a leak candidate via `gc.get_referrers()` and renders what's retaining it; cycles are highlighted.
- The **memory dashboard** aggregates the session: heap timeline, top allocators, leak candidates — each row navigates to source.
- **Basilisk: Force Garbage Collection** — runs `gc.collect()` in the debuggee, so you can confirm whether "leaked" memory is actually reclaimable.

Stop tracking with **Basilisk: Stop Memory Tracking** (also on the debug toolbar and the status-bar menu).

---

## Configuration

The profiler is configured through **editor settings** (`basilisk.profiler.*` in VS Code) — it has no `pyproject.toml` section:

| Setting | Default | Description |
|---------|---------|-------------|
| `basilisk.profiler.sampleRate` | `100` | Sampling frequency in Hz (1–1000) |
| `basilisk.profiler.includeNative` | `false` | Include C-extension frames (py-spy attach only) |
| `basilisk.profiler.lineThreshold` | `1` | Minimum CPU % for a line annotation |
| `basilisk.profiler.functionThreshold` | `2` | Minimum CPU % for a function diagnostic |
| `basilisk.profiler.maxDiagnosticsPerFile` | `20` | Cap on profiler diagnostics per file |
| `basilisk.profiler.showInlineHeatMap` | `true` | Toggle the inline heat decorations |
| `basilisk.profiler.profileOnLaunch` | `false` | Auto-profile every Basilisk debug launch |
| `basilisk.profiler.processRefreshMs` | `2000` | Python Processes panel refresh interval |
| `basilisk.profiler.preset` | `"default"` | `default` / `quick` / `detailed` / `longRunning` |
| `basilisk.profiler.autoSnapshotOnPause` | `true` | Memory snapshot on every debugger pause |
| `basilisk.profiler.autoSnapshot` | `false` | Interval-based memory snapshots |
| `basilisk.profiler.autoSnapshotInterval` | `30` | Interval in seconds for the above |

---

## Platform requirements

### macOS

Two paths, chosen automatically:

- **Programs you launch through Basilisk** ("Run & Profile CPU", debug sessions) are profiled by the **cooperative in-process sampler** — no elevation, no prompts. Modern macOS refuses external memory reads even for root, so for launched programs Basilisk samples from inside the debuggee instead. Trade-off: C-extension frames are not visible on this path.
- **Attaching to an external process** (one you started in a terminal — even under your own user) requires elevation: Basilisk ships a privileged helper (`basilisk-profiler-helper`), and macOS shows a standard administrator-password prompt the first time. The helper attaches as root and streams samples back over a local Unix socket. Cancelling the prompt stops profiling with a clear error.

### Linux

`ptrace` access is governed by `/proc/sys/kernel/yama/ptrace_scope`:

| Scope | Meaning | Basilisk behaviour |
|-------|---------|-------------------|
| `0` | Any same-user process | Attach works directly |
| `1` (default on many distros) | Ancestors only | Programs launched through Basilisk attach fine (the LSP is an ancestor); attaching to unrelated processes fails with remedies |
| `2` | Admin only | Requires elevated capabilities |
| `3` | Disabled | External attach unavailable |

If an attach is denied, Basilisk's error explains the options: `sudo sysctl kernel.yama.ptrace_scope=0`, granting the binary `cap_sys_ptrace`, or profiling via a Basilisk debug session instead.

### Windows

Processes owned by your user attach with no elevation (standard Win32 APIs). Profiling another user's processes is not supported.

---

## CPU diagnostic codes

| Code | Severity | Description |
|------|----------|-------------|
| `BSK-PROF-LINE` | Hint | Line spends ≥ `lineThreshold`% of CPU time |
| `BSK-PROF-FUNC` | Hint | Function spends ≥ `functionThreshold`% of CPU time |

Profiler diagnostics appear only while a session has produced data, and clear with the session. Memory codes are listed [above](#memory-diagnostics).

---

## Profiling in Zed

Zed's extension API has no webviews or custom panels, so the Zed extension surfaces profiling through **LSP diagnostics** (hot lines appear inline as hints) and **informational slash commands** in the assistant panel — each command explains the matching LSP command (`basilisk.profiler.*` / `basilisk.memory.*`) and how to drive it, rather than executing it directly:

| Command | Topic |
|---------|-------|
| `/profile` | Starting CPU profiling |
| `/profstop` | Stopping and getting results |
| `/profsnapshot` | Mid-session snapshots |
| `/memleak` | Starting memory tracking |
| `/memstop` | Stopping memory tracking |
| `/memrefs` | Reference-graph walks |

Flame graphs open via the exported SVG or by dragging the speedscope JSON into [speedscope.app](https://www.speedscope.app).

---

## Profiling in Neovim

The `basilisk.nvim` plugin exposes LSP-backed user commands:

| Command | Action |
|---------|--------|
| `:BasiliskProfile [pid]` | Start CPU profiling (bare form prompts via process list) |
| `:BasiliskProfileStop` | Stop CPU profiling |
| `:BasiliskProfileSnapshot` | Take a CPU snapshot |
| `:BasiliskMemLeak` | Start memory tracking |
| `:BasiliskMemStop` | Stop memory tracking |
| `:BasiliskMemRefs {type}` | Reference graph for a type (with completion) |

Default keymaps: `<prefix>p` starts profiling, `<prefix>P` stops it. Heat annotations render as virtual text.

---

## Architecture

```
┌──────────────┐  workspace/executeCommand   ┌──────────────────────────────┐
│  VS Code /   │────────────────────────────►│  Basilisk LSP (Rust)          │
│  Zed / Nvim  │◄────────────────────────────│  ┌──────────────────────────┐ │
└──────────────┘  diagnostics + progress     │  │  ProfileSessionManager   │ │
                  notifications              │  │  Sampler (py-spy /       │ │
                                             │  │    cooperative)          │ │
                                             │  │  SampleAggregator        │ │
                                             │  │  Speedscope / SVG /      │ │
                                             │  │    .cpuprofile exporters │ │
                                             │  │  MemorySessionManager    │ │
                                             │  │  SnapshotDiffer          │ │
                                             │  │  LeakTracker             │ │
                                             │  └──────────────────────────┘ │
                                             └──────────────┬───────────────┘
                                                            │ samples
                                                            ▼
                                                ┌──────────────────┐
                                                │  Python process  │
                                                │  (your code)     │
                                                └──────────────────┘
```

The LSP server owns all profiling state. Editors invoke `basilisk.profiler.*` / `basilisk.memory.*` commands and render results delivered through diagnostics and the `basilisk/profiler/progress` and `basilisk/memory/timeline` notifications. That's why the same engine drives VS Code, Zed, and Neovim.

---

## Troubleshooting

### "Process not found"

The target process exited before the profiler could attach. Make sure your script is still running, or use **Run & Profile CPU (Current File)** to launch and profile in one step.

### "Not a Python process"

The profiler works with CPython. PyPy and other interpreters are not supported.

### "Permission denied" (macOS)

Enter your password on the administrator prompt when attaching to an external process — or launch the program through Basilisk instead, which needs no elevation at all.

### "Permission denied" (Linux)

Check `cat /proc/sys/kernel/yama/ptrace_scope` — see [Linux requirements](#linux). Programs launched through Basilisk are unaffected.

### "Already profiling"

Each process supports one CPU profiling session at a time. Stop the current session first.

### "Captured N samples, but none landed in your code"

The program finished its work faster than the sampling interval — see [Programs too short to sample](#programs-too-short-to-sample).

---

## How the engine compares

Basilisk embeds py-spy because it is the only Python profiler available as a Rust library crate, which is what makes a zero-dependency, in-LSP profiler possible:

| Property | py-spy | Scalene | Memray | Austin |
|---|---|---|---|---|
| Embeddable as a Rust crate | **Yes** | No | No | No |
| Attaches to running processes | **Yes** | No | No | Yes |
| Modifies the target program | **No** | Yes | Yes | No |

Comparison drawn from each project's own documentation — [py-spy](https://github.com/benfred/py-spy), [Scalene](https://github.com/plasma-umass/scalene), [Memray](https://github.com/bloomberg/memray), [Austin](https://github.com/P403n1x87/austin).

---

## Next steps

- [Debugging](/docs/debugging/) — the integrated debugger that memory tracking (and macOS launch profiling) builds on
- [Installation](/docs/installation/) — editor setup
- [Configuration reference](/docs/configuration/) — checker and LSP configuration (profiler settings are editor settings, listed [above](#configuration))
