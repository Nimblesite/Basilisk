# Whole-Module & Cross-Module Analysis — Specification

> **Scope**: LSP analysis modes — from open-files-only to whole-workspace to cross-module
> **Plan**: [CHECKER-CROSS-MODULE-ANALYSIS-PLAN.md](../plans/CHECKER-CROSS-MODULE-ANALYSIS-PLAN.md)

---

## 1. Problem Statement

The Basilisk LSP server currently analyses files individually and in isolation. Diagnostics are only computed for files that are open in the editor, or scanned at startup. This means:

- A type error caused by a function exported from `utils.py` and consumed in `main.py` is only visible when the consumer file is open.
- Cross-file navigation (Go to Definition, Find References, Rename) silently falls back to single-file results.
- The `scan_workspace()` call at startup analyses all files once but does **not** build a shared symbol graph — each file is still resolved in isolation.
- There is no user-facing control over how aggressively Basilisk analyses the workspace.

This spec defines the **Analysis Mode** setting and the infrastructure required to support whole-module and eventually cross-module analysis.

---

## 2. Analysis Modes

Three modes govern how the LSP server decides *which files to analyse* and *how symbol graphs are shared* across them.

### 2.1 `openFilesOnly`

| Property | Value |
|----------|-------|
| **Scope** | Files currently open in the editor |
| **Trigger** | `didOpen`, `didChange`, `didSave` notifications |
| **Symbol graph** | Per-file, no cross-file sharing |
| **Startup scan** | None |
| **Performance cost** | Minimal — only active documents are analysed |

Diagnostics are published only for open documents. Workspace files are not touched. Suitable for large monorepos where full workspace analysis is too expensive.

### 2.2 `wholeModule` (default)

| Property | Value |
|----------|-------|
| **Scope** | All `.py` / `.pyi` files reachable from workspace roots, respecting `include`/`exclude` config |
| **Trigger** | Startup scan + `didOpen` / `didChange` / `didSave` / file-watcher events |
| **Symbol graph** | Per-file `ResolvedModule` cached in the workspace index; updated incrementally on change |
| **Startup scan** | Full workspace scan; diagnostics published for every file |
| **Performance cost** | Higher startup cost; incremental updates are fast |

This is the default mode. It corresponds to how Pyright's `basic` / `standard` mode works: the entire project is indexed and diagnostics are visible for **all** files, not just open ones.

### 2.3 `crossModule` (future)

| Property | Value |
|----------|-------|
| **Scope** | Same as `wholeModule`, plus import graph traversal across module boundaries |
| **Trigger** | Same as `wholeModule`; additionally triggered by changes to imported modules |
| **Symbol graph** | Shared, reference-counted `ResolvedModule` graph with explicit import edges |
| **Startup scan** | Full workspace scan + import graph construction |
| **Performance cost** | Highest; requires Salsa-backed incremental computation (Phase 7.4) |

`crossModule` enables features that depend on knowing what a symbol *is* across file boundaries: cross-file Go to Definition, cross-file Find References, cross-file Rename, and auto-import suggestions. This mode is **not implemented** yet — it is defined here so that `wholeModule` is architected to be a stepping stone toward it rather than a dead end.

---

## 3. Configuration

### 3.1 Basilisk Config (basilisk.json / pyproject.toml)

```json
{
  "analysisMode": "wholeModule"
}
```

| Value | Meaning |
|-------|---------|
| `"openFilesOnly"` | Analyse only open documents |
| `"wholeModule"` | Analyse all workspace files (default) |
| `"crossModule"` | Cross-file import graph analysis (future) |

Default: `"wholeModule"`

When the field is absent from the config file, `wholeModule` is assumed. This preserves the principle that Basilisk is **strict by default** — the user must explicitly opt down to `openFilesOnly`, not opt up to whole-workspace analysis.

### 3.2 VS Code Extension Setting

A VS Code workspace setting maps 1:1 to the config field:

```json
"basilisk.analysisMode": {
  "type": "string",
  "enum": ["openFilesOnly", "wholeModule", "crossModule"],
  "default": "wholeModule",
  "description": "Controls which files Basilisk analyses.\n\n- openFilesOnly: only files open in the editor\n- wholeModule: all workspace Python files (default)\n- crossModule: full cross-file import graph (future)"
}
```

The extension passes this value to the server via `InitializationOptions` so the server has the setting available before `initialized()` fires.

### 3.3 Config Priority

Config resolution order (highest wins):

1. VS Code workspace setting (`basilisk.analysisMode`)
2. `analysisMode` in `basilisk.json`
3. `analysisMode` in `[tool.basilisk]` section of `pyproject.toml`
4. Hard default: `wholeModule`

---

## 4. Workspace Index

`wholeModule` and `crossModule` modes both require a **workspace index** — a persistent, process-scoped data structure that holds the resolved state of every file in the workspace. The index is owned by `LspServer` and accessed from all request handlers.

### 4.1 WorkspaceIndex Structure

```
WorkspaceIndex {
    roots: Vec<PathBuf>,
    files: DashMap<PathBuf, FileEntry>,   // file path → resolved state
    config: BasiliskConfig,
}

FileEntry {
    source_hash: u64,           // FNV hash of source text; used for invalidation
    resolved: Arc<ResolvedModule>,
    diagnostics: Vec<Diagnostic>,
    version: u64,               // LSP document version (for open files)
    is_open: bool,              // true iff the editor has this file open
}
```

### 4.2 Invalidation

A `FileEntry` is invalidated when:

- Its on-disk content changes (file-watcher event) AND `source_hash` changes
- The editor sends a `didChange` notification for it
- Any of its direct importers are invalidated (cross-module mode only)

When invalidated: re-parse, re-resolve, re-check, update `source_hash` and `diagnostics`, and re-publish diagnostics for the file.

### 4.3 Open-File Priority

When a file is open in the editor, the in-memory text (from `didOpen`/`didChange`) is authoritative. The file-watcher event for the same path is silently ignored as long as `is_open == true`.

When the file is closed (`didClose`), the on-disk text is re-read and used to rebuild the `FileEntry`.

---

## 5. Startup Behaviour

### 5.1 `openFilesOnly`

No workspace scan at startup. The server waits passively for `didOpen` notifications.

### 5.2 `wholeModule`

On `initialized`: all `.py` / `.pyi` files under workspace roots are collected (respecting `include`/`exclude`), analysed in parallel, and their diagnostics published via `publishDiagnostics`. A `window/logMessage` reports the scan summary (file count, error count, elapsed ms). Progress is reported via `window/workDoneProgress` so the editor can show a spinner.

### 5.3 `crossModule` (future)

Same as `wholeModule`, with an additional pass after the initial scan: the import graph is built by walking `ImportInfo` from each `ResolvedModule`, topologically sorted, and inter-module symbol references are resolved. Any file whose diagnostics change as a result is re-checked and re-published.

---

## 6. Incremental Updates

### 6.1 On `didChange` (all modes)

Incremental text edits are applied to the in-memory buffer, then the parse → resolve → check pipeline runs for the changed file. The `WorkspaceIndex` entry and published diagnostics are updated. In `crossModule` mode, direct importers are queued for re-analysis if the exported symbol table changed.

### 6.2 On File-Watcher Event (wholeModule / crossModule)

If the file is open in the editor, the event is ignored (editor text is authoritative). Otherwise the file is read from disk; if `source_hash` is unchanged the entry is left as-is. If changed, the pipeline re-runs, the index is updated, and diagnostics are re-published.

### 6.3 Debouncing

File-watcher events MUST be debounced with a 150 ms delay to avoid thrashing during bulk saves (e.g. `git checkout`, `npm install`). `didChange` events are NOT debounced — latency matters for the open file.

---

## 7. Diagnostic Publishing Contract

| Mode | Which files get diagnostics published |
|------|--------------------------------------|
| `openFilesOnly` | Only currently open documents |
| `wholeModule` | All workspace files (open and closed) |
| `crossModule` | All workspace files + any file whose diagnostics changed due to cross-module re-analysis |

When a file is **deleted** from disk, publish empty diagnostics for its URI to clear the error panel.

When the user switches mode at runtime (via VS Code setting change), the server clears all currently published diagnostics, re-analyses according to the new mode, and re-publishes.

---

## 8. Interaction With Cross-Module Analysis

`wholeModule` is the **prerequisite** for `crossModule`. The design choices here are deliberately forward-compatible:

- `WorkspaceIndex` uses `DashMap<PathBuf, FileEntry>` — the same data structure that the cross-module import graph will augment with import edges.
- `FileEntry` stores `Arc<ResolvedModule>` — the `Arc` allows multiple importers to hold a shared reference without copying.
- Import invalidation in `crossModule` mode is an additive extension to the same invalidation mechanism used in `wholeModule`.
- The `analysisMode` enum has `crossModule` as a defined value even though it is not yet implemented, so clients can opt in when it ships without a breaking config change.

The key difference between `wholeModule` and `crossModule`:

| Concern | `wholeModule` | `crossModule` |
|---------|---------------|---------------|
| Import graph built? | No | Yes |
| Cross-file type info available? | No | Yes |
| Cross-file Go to Definition | Falls back to single-file | Full |
| Cross-file Find References | Falls back to single-file | Full |
| Salsa required? | No (but compatible) | Yes (Phase 7.4) |

---

## 9. LSP Capabilities

When `analysisMode` is `wholeModule` or `crossModule`, the server advertises:

```json
"workspace": {
  "fileOperations": {
    "didCreate": { "filters": [{ "pattern": { "glob": "**/*.py" } }] },
    "didDelete": { "filters": [{ "pattern": { "glob": "**/*.py" } }] },
    "didRename": { "filters": [{ "pattern": { "glob": "**/*.py" } }] }
  }
}
```

When `analysisMode` is `openFilesOnly`, these capabilities are omitted.

---

## 10. Performance Constraints

| Metric | Target |
|--------|--------|
| Startup scan (wholeModule, 10 K LOC) | < 2 s |
| Startup scan (wholeModule, 100 K LOC) | < 10 s |
| Single-file incremental update | < 50 ms |
| Diagnostic publish latency (open file) | < 100 ms after last keystroke |
| Memory per file in index | < 500 KB average |

Large workspaces (> 500 K LOC) MAY show a progress notification and allow the user to cancel. Analysis in `openFilesOnly` mode is always instantaneous on open.

---

## 11. Error Handling

- If a file cannot be read (permissions, encoding), log a `window/logMessage` warning and skip it. Do not crash.
- If the workspace root does not exist, skip silently.
- If the workspace scan exceeds 30 s, log a warning and continue in degraded mode (publish results for files analysed so far).

---

## 12. Non-Goals

- This spec does NOT define type inference across module boundaries (that is `crossModule` Phase 7+).
- This spec does NOT define stub resolution or typeshed integration.
- This spec does NOT define how `crossModule` resolves circular imports — that is deferred to the cross-module spec.
