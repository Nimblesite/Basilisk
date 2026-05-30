# Profiler Process Panel — Implementation Plan {#PROFPANEL-PLAN}

**Spec:** [LSP-PROFILING-SPEC.md](../specs/LSP-PROFILING-SPEC.md) `{#LSPPROF}` — this plan adds the
`{#PROFILE-PROCESSES}` family of spec sections (listed under [Spec changes](#spec-changes-required) below).
**Related:** [EXTENSION-ACTIVITY-PANEL-SPEC.md](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md) `{#EXTACT}`
(the activity-bar panel pattern we mirror), [LSP-ARCHITECTURE-SPEC.md](../specs/LSP-ARCHITECTURE-SPEC.md)
(custom command registration).

---

## Problem {#PROFPANEL-PROBLEM}

Starting a CPU/memory profile today is hostile:

1. **The only entry point is a command** — `basilisk.profileStart` from the Command Palette.
   There is no button anywhere in the Basilisk window. ([profiler.ts:151](../../vscode-extension/src/profiler.ts))
2. **It throws a raw PID text box** at the user — "Python process PID (leave empty to auto-detect)".
   ([profiler.ts:166](../../vscode-extension/src/profiler.ts))
3. **"Auto-detect" is a lie.** The LSP handler hard-rejects a missing `pid` with error `-32001`;
   no auto-detect was ever implemented. ([profiler_handlers.rs:30](../../crates/basilisk-lsp/src/server/profiler_handlers.rs))
4. **Nothing enumerates OS processes.** `basilisk.profiler.list` lists *active profiling sessions*,
   not attachable Python processes. So the extension *cannot* show a picker even if it wanted to —
   the user must alt-tab to a terminal, run `ps`, copy a PID, and paste it back.

The result: the user has a running script and the profiler swears "no process found", because nothing
is looking for one.

## Goal {#PROFPANEL-GOAL}

A **Python Processes** panel in the Basilisk activity bar that lists every attachable Python process
with rich detail, lets the user **sort and group** by any field, and lets them **launch CPU or memory
profiling with one click** — no Command Palette, no hand-typed PIDs.

Per the project's prime directive (*"The LSP drives the functionality, not the IDE extension"*),
process enumeration lives entirely in the **Rust LSP**, exposed as a new advertised command. The panel
is pure UI: it calls the command and renders the result. This keeps Zed/Neovim able to reuse the same
data later.

## Non-goals {#PROFPANEL-NON-GOALS}

- Killing/signalling processes from the panel (destructive; out of scope for v1).
- Zed/Neovim panels (the LSP command is shared and ready for them; their UI is a follow-up).
- Remote/container process attachment.

---

## Architecture {#PROFPANEL-ARCH}

```
┌─────────────────────────────┐   workspace/executeCommand    ┌──────────────────────────────────┐
│  VS Code — Python Processes  │  basilisk.profiler.processes  │  Basilisk LSP (Rust)             │
│  TreeDataProvider (UI only)  │ ────────────────────────────► │  ProcessEnumerator (sysinfo)     │
│  • sort / group / filter     │ ◄──────────────────────────── │  • list python procs             │
│  • inline ▶ Profile / 🧠 Mem │   ProcessInfo[]               │  • best-effort version resolve   │
│  • toolbar Start buttons     │                               │  • privilege/elevation flags     │
└─────────────────────────────┘   basilisk.profiler.start      └──────────────────────────────────┘
            │  click ▶ on a row → start with that pid (no input box)
            ▼
   existing ProfileSessionManager → py-spy attach
```

**New LSP command:** `basilisk.profiler.processes` (advertised in the server's `executeCommandProvider`
capabilities, per [LSP-ARCHITECTURE-SPEC.md]; the extension is forbidden from registering a UI action
for a capability the LSP does not advertise). Reuses the existing
[profiler_handlers.rs](../../crates/basilisk-lsp/src/server/profiler_handlers.rs) dispatch.

**New crate dependency:** `sysinfo` (cross-platform process enumeration: pid, ppid, name, exe, cmd,
cpu, memory, run time, user). Add to `crates/basilisk-lsp/Cargo.toml` **and** keep
`.github/workflows/ci.yml` + `.devcontainer/Dockerfile` in sync per CLAUDE.md.

### Data model {#PROFPANEL-MODEL}

`ProcessInfo` returned by `basilisk.profiler.processes` (spec `{#PROFILE-PROCESSES-MODEL}`):

| Field | Type | Notes |
|---|---|---|
| `pid` | u32 | Process id |
| `ppid` | u32 | Parent pid (for "group by parent") |
| `name` | string | Process name (e.g. `python3.12`) |
| `interpreterPath` | string? | Resolved exe path |
| `script` | string? | Best-effort target script (first non-flag cmd arg) |
| `pythonVersion` | string? | Best-effort, e.g. `3.12.13`; `null` ⇒ render `—` |
| `cpuPercent` | f32 | Instantaneous CPU% |
| `memoryBytes` | u64 | RSS |
| `runtimeSecs` | u64 | Elapsed since start |
| `user` | string? | Owner login |
| `requiresElevation` | bool | True if not owned by current user (macOS/Linux helper prompt) |
| `kind` | enum | `interpreter` \| `launcher` (uvicorn/gunicorn/pytest under python) |

**Detection:** a process is "Python" if its `name`/exe basename matches `python`, `python3`,
`pythonX.Y`, or its `cmd[0]` is such an interpreter. Common launchers (uvicorn/gunicorn/pytest/celery)
running on a Python interpreter are tagged `kind = launcher` so they still appear.

**Python version resolution (must not block enumeration):** derive from the interpreter path pattern
where possible (`.../python3.12` ⇒ `3.12.x`). For unknowns, resolve lazily: spawn the interpreter with
`--version` **once per unique exe path**, short timeout, cache keyed by `(exe, mtime)`. Enumeration
returns immediately with `pythonVersion: null` for unresolved entries; a follow-up
`basilisk/profiler/processesChanged` notification (spec `{#PROFILE-PROCESSES-NOTIFY}`) pushes versions
as they resolve. **Never log cmdline/user** (may contain secrets/PII) — log `count` and truncated
interpreter basenames only, per CLAUDE.md logging standards.

---

## The Panel {#PROFPANEL-PANEL}

A new tree view `basilisk.pythonProcesses` in the existing `basilisk-explorer` container
([package.json:57](../../vscode-extension/package.json)), implemented in a new
`vscode-extension/src/process-explorer.ts`, mirroring the sort logic of
[type-health.ts](../../vscode-extension/src/type-health.ts) and the group/filter logic of
[module-explorer.ts](../../vscode-extension/src/module-explorer.ts). Spec `{#PROFILE-PROCESSES-PANEL}`.

**Row label:** `python3.12 — app.py` · **description:** `PID 82875 · 3.12.13 · 12.4% · 88 MB`
· **tooltip:** full interpreter path, script, user, runtime, elevation note.
· **icon:** `$(snake)`-style python glyph; a `$(lock)` badge overlay when `requiresElevation`.

**Sort modes** (toolbar pick, spec `{#PROFILE-PROCESSES-PANEL-SORT}`): CPU% (default, desc),
Memory, PID, Name, Runtime, Python version.

**Group modes** (toolbar pick, spec `{#PROFILE-PROCESSES-PANEL-GROUP}`): None (flat),
Python version, Interpreter/venv, User, Parent process. Groups render as collapsible parent nodes
with a count badge.

**Filter:** a search box action filters by name/script/pid; a toggle hides `launcher` kinds.

**Refresh:** auto-poll while the view is visible (`onDidChangeVisibility` gates the timer;
interval from `basilisk.profiler.processRefreshMs`, default 2000); manual refresh button always present.

### Launch from the panel (no Command Palette) {#PROFPANEL-LAUNCH}

This is the headline fix — spec `{#PROFILE-PROCESSES-LAUNCH}`:

- **Per-row inline buttons** (`view/item/context`, group `inline`): **▶ Profile CPU**
  (`basilisk.profileProcess`) and **🧠 Track Memory** (`basilisk.memoryTrackProcess`). One click →
  calls the existing `basilisk.profiler.start` / memory-start with that row's `pid`. **No input box.**
- **Row context menu:** Profile CPU · Track Memory · Copy PID · Reveal Script in Editor.
- **Panel toolbar:** **Refresh** · **Sort…** · **Group…** · **Filter** ·
  **▶ Run & Profile Current File** (`basilisk.profileCurrentFile`) — launches the active `.py` under a
  child interpreter Basilisk owns (no elevation) and auto-attaches the profiler. The true one-click path.
- **Welcome view** (`viewsWelcome`) when no Python processes are running:
  *"No Python processes running. [Run & Profile Current File]"*.

`basilisk.profileStart` (the old palette command) is **kept** but rewritten: instead of an input box it
**focuses this panel** and shows a toast "Pick a process below, or Run & Profile Current File". The
lying "auto-detect" prompt is deleted.

### package.json contributions {#PROFPANEL-CONTRIB}

- `views.basilisk-explorer[]` += `{ id: "basilisk.pythonProcesses", name: "Python Processes", when: "basilisk.hasWorkspace" }`.
- New commands: `basilisk.refreshProcesses`, `basilisk.sortProcesses`, `basilisk.groupProcesses`,
  `basilisk.filterProcesses`, `basilisk.profileProcess`, `basilisk.memoryTrackProcess`,
  `basilisk.profileCurrentFile`, `basilisk.copyProcessPid`, `basilisk.revealProcessScript`.
- `menus.view/title` (toolbar) + `menus.view/item/context` (inline + context) wired with
  `when: view == basilisk.pythonProcesses`.
- `viewsWelcome` entry for the empty state.
- New setting `basilisk.profiler.processRefreshMs` (number, default 2000) and
  `basilisk.profiler.showLaunchers` (bool, default true).

---

## Testing {#PROFPANEL-TESTING}

Follows [LSP-PROFILING-SPEC.md#PROFILE-TESTING] and the CLAUDE.md "coarse e2e only" rule.

- **LSP e2e** (`crates/basilisk-lsp/tests/profiler_e2e_pyspy.rs` already spawns a real Python process):
  add a test that spawns a CPU-bound interpreter, calls `basilisk.profiler.processes`, and asserts the
  spawned PID appears with correct `name`, non-null `interpreterPath`, and eventually-resolved
  `pythonVersion`. Assert `requiresElevation == false` for our own child. Assert non-Python noise is
  excluded.
- **LSP e2e (sort/group):** assert the command returns stable, fully-populated records; sorting/grouping
  is UI-side, but verify every documented field is present and typed.
- **VSIX test:** per CLAUDE.md, **do not** call `vscode.commands.getCommands(true)` /
  `whenCommandReady`. Assert the panel through internal VSIX state — register the
  `TreeDataProvider`, feed it a stubbed `ProcessInfo[]`, and assert `getChildren()` yields the expected
  sorted/grouped tree items and that inline-button commands carry the right `pid` argument.
- **Coverage:** ratchet `coverage-thresholds.json` upward only; never down.

## Spec changes required {#spec-changes-required}

Add to [LSP-PROFILING-SPEC.md](../specs/LSP-PROFILING-SPEC.md), all under a new
`## Process Enumeration & Selection {#PROFILE-PROCESSES}` section beside `{#PROFILE-PROTOCOL}`:

- `{#PROFILE-PROCESSES-LSP}` — `basilisk.profiler.processes` request/response.
- `{#PROFILE-PROCESSES-MODEL}` — the `ProcessInfo` data model (table above).
- `{#PROFILE-PROCESSES-NOTIFY}` — `basilisk/profiler/processesChanged` lazy-version notification.
- `{#PROFILE-PROCESSES-PANEL}` / `-SORT` / `-GROUP` — the panel, sort modes, group modes.
- `{#PROFILE-PROCESSES-LAUNCH}` — launch-from-panel UX, replacing the input-box flow.
- Amend `{#PROFILE-REQUESTS-START}` to drop the false "auto-detect when omitted" claim (or implement it
  — see TODO Phase 5).
- Register this plan in [docs/INDEX.md](../INDEX.md) Plans table.

---

## TODO {#PROFPANEL-TODO}

### Phase 0 — Spec & scaffolding
- [ ] Add `{#PROFILE-PROCESSES}` section family to `LSP-PROFILING-SPEC.md` (all sub-IDs above).
- [ ] Amend `{#PROFILE-REQUESTS-START}` to stop claiming unimplemented auto-detect.
- [ ] Register this plan in `docs/INDEX.md` Plans table.
- [ ] Add `sysinfo` to `crates/basilisk-lsp/Cargo.toml`; mirror version into
      `.github/workflows/ci.yml` and `.devcontainer/Dockerfile`.

### Phase 1 — LSP process enumeration (Rust, the core)
- [ ] New `crates/basilisk-lsp/src/profiler/processes.rs`: `ProcessEnumerator` over `sysinfo`,
      producing `ProcessInfo` (`// Implements [PROFILE-PROCESSES-MODEL]`). Keep file < 500 LOC.
- [ ] Python detection: interpreter-name/exe match + `cmd[0]` interpreter + launcher tagging.
- [ ] Best-effort Python version: path-pattern fast path; lazy `--version` resolver cached by
      `(exe, mtime)`; never block enumeration.
- [ ] `requiresElevation`: compare process owner to current uid (reuse logic shape from
      [privilege.rs](../../crates/basilisk-lsp/src/profiler/privilege.rs)).
- [ ] Register `basilisk.profiler.processes` in `profiler_handlers.rs` + advertise it in the server's
      `executeCommandProvider` capabilities (`// Implements [PROFILE-PROCESSES-LSP]`).
- [ ] `basilisk/profiler/processesChanged` notification when lazy versions resolve
      (`// Implements [PROFILE-PROCESSES-NOTIFY]`).
- [ ] Structured logging only: log process **count** + truncated interpreter basenames, never cmdline/user.

### Phase 2 — Fix the broken start path
- [ ] Delete the lying input box in [profiler.ts:166](../../vscode-extension/src/profiler.ts); rewrite
      `basilisk.profileStart` to focus the new panel + toast.
- [ ] Decide: implement real LSP-side auto-detect OR remove the claim from spec & UI (track in Phase 5).

### Phase 3 — The panel (VS Code extension)
- [ ] New `vscode-extension/src/process-explorer.ts`: `TreeDataProvider<ProcessNode>` calling
      `basilisk.profiler.processes`; subscribe to `processesChanged`
      (`// Implements [PROFILE-PROCESSES-PANEL]`).
- [ ] Row rendering: label/description/tooltip/icon + `$(lock)` elevation badge.
- [ ] Sort modes (CPU/Memory/PID/Name/Runtime/Version) — reuse pattern from `type-health.ts`.
- [ ] Group modes (None/Version/Interpreter/User/Parent) — reuse pattern from `module-explorer.ts`.
- [ ] Filter action (search + hide-launchers toggle).
- [ ] Visibility-gated auto-refresh timer (`basilisk.profiler.processRefreshMs`); manual refresh button.
- [ ] Register the provider in [extension.ts](../../vscode-extension/src/extension.ts).

### Phase 4 — Launch UX (`{#PROFILE-PROCESSES-LAUNCH}`)
- [ ] `package.json`: add the `basilisk.pythonProcesses` view, all new commands, `view/title` toolbar
      menus, `view/item/context` inline + context menus, `viewsWelcome` empty state, new settings.
- [ ] `basilisk.profileProcess` / `basilisk.memoryTrackProcess`: start CPU/memory profiling with the
      row's `pid` — **no input box**.
- [ ] `basilisk.profileCurrentFile`: launch active `.py` under a Basilisk-owned child interpreter and
      auto-attach (no elevation) — the one-click path; reuse the debug-launch plumbing where possible.
- [ ] `basilisk.copyProcessPid`, `basilisk.revealProcessScript`.

### Phase 5 — Tests, polish, ratchet
- [ ] LSP e2e in `profiler_e2e_pyspy.rs`: spawn Python → `processes` lists it → assert fields +
      eventual version + `requiresElevation == false`; assert noise excluded.
- [ ] VSIX test: stub `ProcessInfo[]` → assert sorted/grouped `getChildren()` tree + inline command
      args carry correct `pid` (no `getCommands(true)` / `whenCommandReady`).
- [ ] `deslop:find-similar` before adding code, `deslop:top-offenders` after; merge any duplication.
- [ ] `make ci` green; ratchet `coverage-thresholds.json` up only.
- [ ] (Optional) auto-detect: pick the highest-CPU `interpreter`-kind process when start is invoked with
      no pid and exactly one strong candidate; otherwise focus the panel.

### Phase 6 — Cross-editor follow-up (not v1)
- [ ] Zed/Neovim: consume the shared `basilisk.profiler.processes` command; surface as a list/slash UI.
