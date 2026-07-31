# LSP Analysis Modes — Specification {#LSPMODES}

> **Scope**: How the LSP server decides which files to analyse and how symbol graphs are shared
> **Related**: [LSP-ARCHITECTURE-SPEC.md §LSPARCH-FEATURES](LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES) — LSP features and protocol

---

## Analysis Modes {#ANALYSIS-MODES}

Three modes govern which files are analysed and how symbol information flows.

### openFilesOnly {#ANALYSIS-OPEN}

| Property | Value |
|----------|-------|
| **Scope** | Files currently open in the editor |
| **Trigger** | `didOpen`, `didChange`, `didSave` notifications |
| **Symbol graph** | Per-file, no cross-file sharing |
| **Startup scan** | None |
| **Performance cost** | Minimal — only active documents are analysed |

Diagnostics published only for open documents. For large monorepos where full workspace analysis is too expensive.

### wholeModule {#ANALYSIS-WHOLE}

| Property | Value |
|----------|-------|
| **Scope** | All `.py` / `.pyi` files reachable from workspace roots, respecting `include`/`exclude` config |
| **Trigger** | Startup scan + `didOpen` / `didChange` / `didSave` / file-watcher events |
| **Symbol graph** | Per-file `ResolvedModule` cached in the workspace index; updated incrementally on change |
| **Startup scan** | Full workspace scan; diagnostics published for every file |
| **Performance cost** | Higher startup cost; incremental updates are fast |

Default mode. Equivalent to Pyright's [`diagnosticMode: workspace`](https://microsoft.github.io/pyright/#/configuration): the entire project is indexed and diagnostics are visible for all files, not just open ones.

### crossModule {#ANALYSIS-CROSS}

| Property | Value |
|----------|-------|
| **Scope** | Same as `wholeModule`, plus import graph traversal across module boundaries |
| **Trigger** | Same as `wholeModule`; additionally triggered by changes to imported modules |
| **Symbol graph** | Shared, reference-counted `ResolvedModule` graph with explicit import edges |
| **Startup scan** | Full workspace scan + import graph construction |
| **Performance cost** | Highest |

`crossModule` enables features that need cross-boundary symbol identity: cross-file Go to Definition, Find References, Rename, and auto-import suggestions.

| Concern | `wholeModule` | `crossModule` |
|---------|---------------|---------------|
| Import graph built? | No | Yes |
| Cross-file type info available? | No | Yes |
| Cross-file Go to Definition | Falls back to single-file | Full |
| Cross-file Find References | Falls back to single-file | Full |
| Cross-file Rename | Not supported | Full |
| Auto-import suggestions | Not supported | Full |

---

## Configuration {#ANALYSIS-CONFIG}

### Config Sources {#ANALYSIS-CONFIG-SRC}

```json
{ "analysisMode": "wholeModule" }
```

| Value | Meaning |
|-------|---------|
| `"openFilesOnly"` | Analyse only open documents |
| `"wholeModule"` | Analyse all workspace files (default) |
| `"crossModule"` | Cross-file import graph analysis |

Default: `"wholeModule"`. The user must explicitly opt down to `openFilesOnly`.

### Config Priority {#ANALYSIS-CONFIG-PRI}

Resolution order (highest wins):

1. Editor workspace setting (`basilisk.analysisMode`) — delivered as `initializationOptions.analysisMode` at startup and re-applied at runtime via `workspace/didChangeConfiguration` (top-level `analysisMode` or nested `basilisk.analysisMode`). Editors that always forward a value (the VS Code extension and basilisk.nvim both send their `wholeModule` default) pin the mode from the editor side; clients that send no value (e.g. the Zed extension) fall through to the file tier.
2. The first parseable config file in the first workspace root, checked in this order: `pyrightconfig.json` (pyright compatibility), then `pyproject.toml` — `[tool.basilisk]` or, failing that, `[tool.pyright]`. The winning file supplies the entire workspace config: precedence is first-file-wins, NOT per-field merging, so a `pyrightconfig.json` that omits `analysisMode` resolves to the default even if `pyproject.toml` sets one (mirroring [pyright's own whole-file precedence](https://microsoft.github.io/pyright/#/configuration) of `pyrightconfig.json` over `pyproject.toml`). These pyright files are **compatibility inputs to this analysis tier only**, parsed into the one shared model — rule and tag entries live solely in `pyproject.toml` `[tool.basilisk]` ([CHKARCH-CONFIG-FILE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-FILE)), which the server watches live ([LSPARCH-CONFIG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG)); `pyrightconfig.json` is read at startup and registry rebuilds, and is not watched.
3. Hard default: `wholeModule`.

Tier 1 and the fallback are resolved by `resolve_analysis_mode` (`crates/basilisk-lsp/src/workspace_analysis.rs`); the file tier is `load_config` (`crates/basilisk-lsp/src/config.rs`), which the CLI shares.

To be explicit about liveness ([LSPARCH-CONFIG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG)): the analysis **mode** is resolved when the index is (re)built — at `initialize`, on a `didChangeConfiguration` that supplies a mode, and on workspace-folder changes — it is a session-scoped structural choice, not hot-swapped per keystroke. **Rule** configuration, include/exclude, and the import environment are fully live: the server's own watcher refreshes them through the shared refresh tail without any client watcher support and without a restart.

This tiering governs the **analysis-level** workspace config (mode, formatter,
etc.) only. **Rule** config is resolved per file by walking ancestor
directories, and it is deliberately *not* a cumulative merge: every ancestor
`pyproject.toml` carrying a `[tool.basilisk]` table contributes to a
nearest-first chain, and the nearest table that decides a rule wins that rule
outright. Non-rule scalar fields do merge additively, nearest directory winning
per key. There are no per-module or per-path override tables and no glob-keyed
sections — the model is exactly two flat maps, `[tool.basilisk.rules]` and
`[tool.basilisk.rule-tags]` ([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)).
Scoping a rule to part of the tree is done by placing a `pyproject.toml` in
that folder. See
[CHKARCH-CONFIG-DISCOVERY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-DISCOVERY)
(GitHub #311) — identically in the CLI and the LSP.

---

## Workspace Index {#ANALYSIS-INDEX}

`wholeModule` and `crossModule` both require a **workspace index** — a process-scoped structure holding the resolved state of every workspace file.

### Structure {#ANALYSIS-INDEX-STRUCT}

```
WorkspaceIndex {
    roots: Vec<PathBuf>,
    files: DashMap<PathBuf, FileEntry>,
    config: BasiliskConfig,
    import_graph: Option<ImportGraph>,  // populated in crossModule
}

FileEntry {
    source_hash: u64,
    resolved: Arc<ResolvedModule>,
    diagnostics: Vec<Diagnostic>,
    version: u64,
    is_open: bool,
}
```

### Invalidation {#ANALYSIS-INDEX-INVAL}

A `FileEntry` is invalidated when:

- Its on-disk content changes (file-watcher event) AND `source_hash` changes
- The editor sends a `didChange` notification for it
- A file it depends on changes in a way that affects its output (`crossModule` only)
- The root configuration changes — config-editor apply, edit to an open config
  buffer, or a disk change seen by the server-owned watcher: the shared refresh
  tail replaces the root config, invalidates the per-directory config memo, and
  rechecks every indexed file
  ([LSPARCH-CONFIG](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG))

When invalidated, the file is re-analysed **through the salsa engine**
([CHKARCH-INCREMENTAL-SALSA]), which re-runs only the queries whose inputs
actually changed; `source_hash` and `diagnostics` update and the result is
re-published. Cross-file dependency tracking is content-precise: the salsa
queries record edges on the imported files' tracked text and export sets, so a
body-only edit in a dependency backdates (importers' memos stay valid) while an
export change recomputes exactly the affected importers — no file-granularity
cascade walk.

### Open-File Priority {#ANALYSIS-INDEX-OPEN}

For an open file, the in-memory text (`didOpen`/`didChange`) is authoritative; file-watcher events for that path are ignored while `is_open == true`. On `didClose`, on-disk text is re-read to rebuild the `FileEntry`.

---

## Import Graph {#ANALYSIS-GRAPH}

The import graph serves the navigation handlers' **reverse lookups** ("who
imports this file?") for cross-file references and rename
([ANALYSIS-CROSSLSP-REFS] / [ANALYSIS-CROSSLSP-RENAME]). It plays no role in
invalidation — that is the salsa engine's job ([ANALYSIS-INDEX-INVAL],
[CHKARCH-INCREMENTAL-SALSA]), which tracks cross-file dependencies
content-precisely rather than at file granularity.

### Structure {#ANALYSIS-GRAPH-STRUCT}

```rust
// crates/basilisk-lsp/src/import_graph.rs
pub struct ImportGraph {
    forward: HashMap<PathBuf, HashSet<PathBuf>>,   // file → files it imports
    reverse: HashMap<PathBuf, HashSet<PathBuf>>,   // file → files that import it
}
```

### Construction {#ANALYSIS-GRAPH-BUILD}

`build_from_index()` walks `ImportInfo.resolved_path` for every file in the `WorkspaceIndex`, populating forward and reverse edges. It is rebuilt after every workspace-wide re-analysis in `crossModule` mode.

---

## Cross-Module Symbol Sharing {#ANALYSIS-SYMBOLS}

### External Symbols {#ANALYSIS-SYMBOLS-EXT}

```rust
// crates/basilisk-resolver/src/scope/external_symbol.rs
pub struct ExternalSymbol {
    pub kind: ExternalSymbolKind,  // Function, Class, Variable, ReExport
    pub name: String,
    pub type_annotation: Option<String>,
    pub source_path: PathBuf,
    pub source_span: Span,
    pub signature: Option<String>,
}
```

Each `ResolvedModule` carries `imported_symbols: HashMap<String, ExternalSymbol>` — symbols imported from other modules, resolved during the cross-module pass.

### Population {#ANALYSIS-SYMBOLS-POP}

Population lives in the checker's memoized cross-module salsa queries
(`crates/basilisk-checker/src/incremental.rs`, helpers in
`crates/basilisk-checker/src/exports.rs`), which the LSP's engine runs in
`crossModule` mode ([CHKARCH-INCREMENTAL-SALSA]):

1. **Export extraction**: the `module_exports(file)` query parses a
   workspace-tracked file's **current** (possibly in-memory) text and
   `extract_exports()` collects its public symbols (functions, classes,
   module-level variables), building signatures via
   `build_function_signature()`. The result is memoized per file and — because
   salsa backdates unchanged values — a body-only edit re-derives an equal
   export set and leaves every importer's memo valid.
2. **Import resolution**: the `cross_resolved_module(file)` query walks the
   file's resolved imports and populates `imported_symbols`
   (`populate_imported_symbols()`): workspace-tracked targets resolve through
   `module_exports`; external `.pyi` stubs and PEP 561 `py.typed` packages are
   parsed from disk on demand; non-`py.typed` packages are skipped (opt-in
   only). A `from`-import binds only its named symbols; plain and star imports
   publish the whole export set.

### Invalidation Cascading {#ANALYSIS-SYMBOLS-INVAL}

When a file changes — on disk (file watcher) or in the editor (`didChange`/`didSave` on an **open** file) — its exports are diffed:

1. Re-analyse the changed file.
2. `exported_symbol_names()` compares old and new exports.
3. If exports changed, re-analyse the workspace through the salsa engine
   (`reresolve_imports_and_recheck`) so cross-module diagnostics refresh
   without a reload: the engine is primed with every indexed file's current
   text (open files contribute their in-memory buffers), and salsa recomputes
   exactly the files whose dependencies changed — the rest are revalidated
   memos. Only files whose diagnostics actually **changed** are republished
   (an identical set is a client no-op; the edited/reloaded file itself always
   republishes). Watcher path uses `reload_and_diff_exports`; open-file path
   uses `set_open_refresh_dependents` — the watcher's `reload_from_disk` skips
   open files, so editing an open module would otherwise leave dependents
   stale (GitHub #56).
4. If exports unchanged, skip the cascade. (For a **stub** dependency the
   export diff still changes — `.pyi` files parse as Python — so an in-memory
   edit to an open user stub refreshes its importers' `imports_module_attribute`
   diagnostics through the same path, with the salsa query re-capturing the
   stub API from the tracked text rather than stale disk.)

---

## Cross-File LSP Features {#ANALYSIS-CROSSLSP}

Features enabled by `crossModule`, unavailable or degraded in `wholeModule`:

### Cross-File Go to Definition {#ANALYSIS-CROSSLSP-GOTODEF}

Follow `ImportInfo.resolved_path` to the symbol's `name_span` in the target `ResolvedModule`; re-exports are followed across the import chain.

### Cross-File Find All References {#ANALYSIS-CROSSLSP-REFS}

Use import-graph reverse edges: for a symbol defined in file A, search all importers of A for usage of that name.

### Cross-File Rename {#ANALYSIS-CROSSLSP-RENAME}

Produces a multi-file `WorkspaceEdit`: definition site + import sites (`from module import old_name` → `new_name`) + all usage sites in importing files.

### Auto-Import Completion {#ANALYSIS-CROSSLSP-IMPORT}

`SymbolIndex` (`auto_import.rs`) indexes all workspace exports. Typing an unknown symbol suggests imports with `additionalTextEdits` that insert the import statement.

---

## Startup Behaviour {#ANALYSIS-STARTUP}

### openFilesOnly Startup {#ANALYSIS-STARTUP-OPEN}

No workspace scan; the server waits for `didOpen` notifications.

### wholeModule Startup {#ANALYSIS-STARTUP-WHOLE}

During `initialize`, Basilisk resolves the selected step-3 stdlib source from
local sources only and gates the first check on it
([STUBRES-TYPESHED-OFFLINE](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-OFFLINE)).
This implements "Typeshed stubs for the standard library" in the pinned typing
order ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
The source is the custom path or the pinned commit verified offline from the
store/bundle; a missing pin is a terminal `NO SOURCE`, never a substitute. The scan
then builds import paths, primes Salsa, analyzes each workspace file once, and
publishes diagnostics. Open buffers are re-analyzed from editor text. Progress
and the selected source appear in Service Info
([STUBRES-TYPESHED-WARN](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).

### crossModule Startup {#ANALYSIS-STARTUP-CROSS}

Same as `wholeModule` — the scan's engine pass runs the cross-module queries ([ANALYSIS-SYMBOLS-POP]), so every file's `imported_symbols` reflect the other modules' exports — plus the import graph is built from `ImportInfo` for navigation reverse-lookups ([ANALYSIS-GRAPH]).

---

## Incremental Updates {#ANALYSIS-INCR}

### didChange {#ANALYSIS-INCR-CHANGE}

Incremental edits are applied to the in-memory buffer, then parse → resolve → check runs for the changed file. In `crossModule`, direct importers are queued for re-analysis if the exported symbol table changed.

### Import resolution on incremental re-check {#ANALYSIS-INCR-IMPORTS}

The `resolve` step of any incremental re-check (`didOpen`, `didChange`, disk reload, dependent invalidation) MUST resolve third-party and workspace imports against the **same** `ImportSearchPaths` (venv site-packages, workspace members, stub paths, the resolved typeshed stdlib source, uv registry) the full scan used. The full scan builds and caches these on the workspace index; incremental re-checks reuse the cached value (site-packages discovery touches the filesystem and MUST NOT run per keystroke). Discovery resolves the interpreter's `site-packages` from its own layout — PATH lookup plus prefix inspection (`detect_for_interpreter_on`, `crates/basilisk-lsp/src/import_resolver/ambient.rs`); starting Python to recover a path already encoded in the layout is startup cost a short-lived `basilisk check` must not pay. The interpreter probe remains the authoritative fallback for custom layouts.

The cached `ImportSearchPaths` include the selected typeshed path and exact source
identity — an identity string, held by a `SharedSnapshot` that resolves its
`Snapshot` on first use. Activation therefore names the source without building
it: the CLI spawns the build (plus a `prewarm_builtin_classes` of the
`builtins.pyi` index) on a background thread before collecting files, and a
cache-hit run that never reads a stub never forces it at all. A
`typeshed-path` or `typeshed-commit` change invalidates the cached paths and
rebuilds step 3 before rechecking
([STUBRES-TYPESHED-PIN](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-PIN)).
This derived cache changes performance only; it preserves the pinned resolution
order ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
It is reused across keystrokes, never across a source or configuration change.

Otherwise the syntactic resolver marks every import `Unresolved`, resurrecting false `imports_unresolved` for packages that resolve cleanly on the CLI and at startup. The diagnostics an incremental re-check **publishes** MUST reflect import resolution — not just the cached symbol table used by navigation.

### File-Watcher Event {#ANALYSIS-INCR-WATCH}

If the file is open, the event is ignored. Otherwise read from disk; if `source_hash` is unchanged, leave the entry as-is; if changed, re-run the pipeline.

### Debouncing {#ANALYSIS-INCR-DEBOUNCE}

File-watcher events are trailing-debounced 200 ms (`FILE_WATCHER_DEBOUNCE_MS`, `crates/basilisk-lsp/src/server/mod.rs`) to avoid thrashing during bulk saves: each `workspace/didChangeWatchedFiles` batch cancels any pending re-analysis task and schedules a fresh one, so a burst of events triggers work only once it settles. `didChange` events are NOT debounced — latency matters.

---

## Diagnostic Publishing Contract {#ANALYSIS-PUBLISH}

| Mode | Which files get diagnostics published |
|------|--------------------------------------|
| `openFilesOnly` | Only currently open documents |
| `wholeModule` | All workspace files (open and closed) |
| `crossModule` | All workspace files + any file whose diagnostics changed due to cross-module re-analysis |

On **delete**, publish empty diagnostics to clear the error panel. On runtime mode switch, clear all diagnostics, re-analyse, re-publish.

Publication must reflect the index state at **publish time**, not at compute
time (GitHub #264):

- **Scans never block the message loop.** Every workspace scan (startup, mode
  switch, re-enable, workspace-folder change) runs in a spawned task
  (`run_workspace_scan`) — a scan computed inside a notification handler
  starves every other message, including the `didClose` whose clear the editor
  is waiting on, for the scan's full duration.
- **Stale scan results are dropped.** Before publishing each scan entry, the
  server re-checks the index: a file removed mid-scan (closed in
  `openFilesOnly`, or an out-of-workspace file closed in any mode) is skipped —
  republishing its snapshot would resurrect diagnostics `didClose` already
  cleared, with nothing left to ever clear them. After a mid-scan switch to
  `openFilesOnly`, only open files publish (`publish_scan_results` /
  `scan_entry_still_current`).
- **Mode switches never queue an index write.** `WorkspaceIndex::set_mode` is
  interior-mutable and applied through a READ guard. A mode-flip writer queued
  behind a running scan's read guard would make tokio's fair `RwLock` block
  every subsequent reader, saturating tower-lsp's bounded handler slots and
  stalling the entire message loop — the `didClose` clear then arrives seconds
  late (the original flaky-clear failure).
- **Tab close sends a synthetic `didClose`** for every file the server would
  clear on close: all files in `openFilesOnly`, and out-of-workspace files in
  the whole-workspace modes. VS Code disposes closed documents lazily, so the
  language client's own `didClose` can lag a tab close unboundedly
  (`registerTabTracking` in `vscode-extension/src/lsp-client.ts`). In-workspace
  files in `wholeModule`/`crossModule` keep their diagnostics per the table
  above.
- **Exactly one publisher.** `store.reset()` MUST stop (dispose) the
  LanguageClient it replaces before starting a new one. A forgotten-but-alive
  client is a zombie publisher: it keeps forwarding `didOpen`/`didClose` to
  its own server process and publishing into its own diagnostics collection —
  VS Code merges collections, so the zombie's late republishes resurrect
  diagnostics the real server already cleared (the observed root cause of the
  flaky openFilesOnly clear, GitHub #264).

Covered end-to-end by the `#264` regression test in
`vscode-extension/src/test/suite/lsp-analysis-mode.test.ts` (file closed
mid-scan must stay cleared).

---

## Type Checking Toggle {#ANALYSIS-ENABLED}

The `basilisk.enabled` setting (surfaced as the **Diagnostics** toggle in the
activity panel, [EXTACT-INFO-FEATURE-STATUS]) gates **all diagnostic
publication**. The LSP is authoritative for diagnostics in every mode, so the
toggle is honoured **server-side** — the editor's own
[`subprocess-mode`](VSIX-SPEC.md) path mirrors it only as a fallback.

Contract (GitHub #65 / #119):

- **Forwarded:** the editor MUST include `enabled` in `initializationOptions` and
  in every `workspace/didChangeConfiguration` payload (both the flat top-level
  key and the nested `basilisk.enabled` shape are accepted).
- **On disable** (`true → false`): publish **empty** diagnostics for every
  indexed URI (clearing stale errors everywhere they surface — editor squiggles,
  Problems panel, module tree) and **suppress** all further publication. The
  index keeps tracking edits; only publication is gated.
- **While disabled:** `didOpen` / `didChange` / `didSave` / `didClose`, the
  file-watcher re-analysis, the startup scan, and registry rebuilds all run but
  publish **nothing**.
- **On enable** (`false → true`): re-scan per the active mode and re-publish, so
  diagnostics cleared on disable come back.
- **At startup:** a client that initializes with `enabled = false` gets **no**
  diagnostics from the initial workspace scan.
- **Grading surfaces are gated too** (the v0.25.0 showstopper reopen of #119):
  `basilisk.workspaceModules` and `basilisk.typeHealth` MUST NOT serve grading
  while disabled. Every payload stamps `typeCheckingEnabled`; when `false`, the
  grading fields (`coveragePercent`, `errors`, `warnings`, `adopted`, symbol
  totals) are **omitted** — never zeroed — so no client can render a
  `"NN% typed"` header, coverage-tinted (red) rows, or tally badges from a
  stale shape. The module list itself stays as a navigation surface, and
  `typeHealth` serves an empty `modules` list. Clients render an explicit
  `"Type checking disabled"` header state ([EXTACT-MODULES-HEADER]).
- **Panel refresh on transition:** flipping the toggle repaints the panel
  immediately in both directions — the clear/re-publish above fires the
  client's diagnostics listener, which bumps `analysisRevision`
  ([EXTACT-REACTIVE-STATE]) even in a diagnostics-free workspace, because
  clearing publishes an (empty) set for every indexed URI.

Implemented in `crates/basilisk-lsp/src/server/init.rs`
(`apply_type_checking_toggle`, `clear_all_diagnostics`, `rescan_after_enable`),
the gated publish paths in `crates/basilisk-lsp/src/server/document.rs`, and the
gated panel builders in `crates/basilisk-lsp/src/server/activity_panel/`
(`build_module_tree`, `build_type_health`); forwarded by
`readBasiliskSettings()` in `vscode-extension/src/lsp-client.ts`; rendered by
`workspaceHealthMessage` / `workspaceHealthBadge` / `ModuleTreeItem` in
`vscode-extension/src/module-explorer.ts`.
Exercised by `ws_test_type_checking_toggle.rs` (real LSP) and
`type-checking-toggle.test.ts` (real VS Code window — diagnostics clearing,
panel-payload gating, header/row neutrality, and zero-diagnostics refresh).

---

## LSP Capabilities {#ANALYSIS-CAPS}

Workspace capabilities are **mode-independent**. `build_capabilities`
(`crates/basilisk-lsp/src/server/init.rs`) is parameterised only by whether the
formatter engine is enabled ([LSPFMT-CAPABILITIES](LSP-FORMATTING-SPEC.md#LSPFMT-CAPABILITIES)),
so the `initialize` response advertises the same workspace block in every
`analysisMode`, `openFilesOnly` included:

```json
"workspace": {
  "workspaceFolders": { "supported": true, "changeNotifications": true },
  "fileOperations": {
    "willRename": {
      "filters": [
        { "scheme": "file", "pattern": { "glob": "**/*.py", "matches": "file" } }
      ]
    }
  }
}
```

`workspace/willRenameFiles` is the only file-operation method the server serves
(`crates/basilisk-lsp/src/server/handlers/file_operations.rs`): renaming a module
returns a `WorkspaceEdit` rewriting the imports that pointed at it
([REFACTOR-RENAMEMOD](LSP-REFACTORING-SPEC.md#REFACTOR-RENAMEMOD)). The
`didCreate` / `didDelete` / `didRename` notifications are neither advertised nor
handled — creations and deletions reach the index through
`workspace/didChangeWatchedFiles`, which is where the mode gate actually lives:
that handler returns early in `openFilesOnly` after refreshing configuration
([ANALYSIS-INCR-WATCH]).

---

## Performance Constraints {#ANALYSIS-PERF}

| Metric | Target |
|--------|--------|
| Startup scan (wholeModule, 10 K LOC) | < 2 s |
| Startup scan (wholeModule, 100 K LOC) | < 10 s |
| Single-file incremental update | < 50 ms |
| Diagnostic publish latency (open file) | < 100 ms after last keystroke |
| Memory per file in index | < 500 KB average |

Large workspaces (> 500 K LOC) MAY show a cancellable progress notification.

---

## Error Handling {#ANALYSIS-ERRORS}

- File unreadable (permissions, encoding): log a `window/logMessage` warning and skip. Do not crash.
- Workspace root missing: skip silently.
- Workspace scan exceeds 30 s: log a warning, continue in degraded mode.
- Circular imports: detect, emit diagnostic, break cycle for ordering.
