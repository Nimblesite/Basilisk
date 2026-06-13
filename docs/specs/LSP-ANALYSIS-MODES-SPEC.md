# LSP Analysis Modes — Specification {#LSPMODES}

> **Scope**: How the LSP server decides which files to analyse and how symbol graphs are shared
> **Related**: [LSP-ARCHITECTURE-SPEC.md §LSPARCH-FEATURES](LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES) — LSP features and protocol

---

## Analysis Modes {#ANALYSIS-MODES}

Three modes govern which files are analysed and how symbol information flows between them.

### openFilesOnly {#ANALYSIS-OPEN}

| Property | Value |
|----------|-------|
| **Scope** | Files currently open in the editor |
| **Trigger** | `didOpen`, `didChange`, `didSave` notifications |
| **Symbol graph** | Per-file, no cross-file sharing |
| **Startup scan** | None |
| **Performance cost** | Minimal — only active documents are analysed |

Diagnostics are published only for open documents. Suitable for large monorepos where full workspace analysis is too expensive.

### wholeModule {#ANALYSIS-WHOLE}

| Property | Value |
|----------|-------|
| **Scope** | All `.py` / `.pyi` files reachable from workspace roots, respecting `include`/`exclude` config |
| **Trigger** | Startup scan + `didOpen` / `didChange` / `didSave` / file-watcher events |
| **Symbol graph** | Per-file `ResolvedModule` cached in the workspace index; updated incrementally on change |
| **Startup scan** | Full workspace scan; diagnostics published for every file |
| **Performance cost** | Higher startup cost; incremental updates are fast |

This is the default mode. It corresponds to how Pyright's `basic` / `standard` mode works: the entire project is indexed and diagnostics are visible for **all** files, not just open ones.

### crossModule {#ANALYSIS-CROSS}

| Property | Value |
|----------|-------|
| **Scope** | Same as `wholeModule`, plus import graph traversal across module boundaries |
| **Trigger** | Same as `wholeModule`; additionally triggered by changes to imported modules |
| **Symbol graph** | Shared, reference-counted `ResolvedModule` graph with explicit import edges |
| **Startup scan** | Full workspace scan + import graph construction |
| **Performance cost** | Highest |

`crossModule` enables features that depend on knowing what a symbol *is* across file boundaries: cross-file Go to Definition, cross-file Find References, cross-file Rename, and auto-import suggestions.

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

Default: `"wholeModule"`. Basilisk is strict by default — the user must explicitly opt down to `openFilesOnly`.

### Config Priority {#ANALYSIS-CONFIG-PRI}

Config resolution order (highest wins):

1. Editor workspace setting (`basilisk.analysisMode`)
2. `analysisMode` in `basilisk.json`
3. `analysisMode` in `[tool.basilisk]` section of `pyproject.toml`
4. Hard default: `wholeModule`

---

## Workspace Index {#ANALYSIS-INDEX}

`wholeModule` and `crossModule` modes both require a **workspace index** — a persistent, process-scoped data structure that holds the resolved state of every file in the workspace.

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
- Any of its direct importers are invalidated (`crossModule` only)

When invalidated: re-parse, re-resolve, re-check, update `source_hash` and `diagnostics`, re-publish.

### Open-File Priority {#ANALYSIS-INDEX-OPEN}

When a file is open in the editor, the in-memory text (from `didOpen`/`didChange`) is authoritative. File-watcher events for the same path are silently ignored as long as `is_open == true`. When closed (`didClose`), on-disk text is re-read to rebuild the `FileEntry`.

---

## Import Graph {#ANALYSIS-GRAPH}

The import graph is the core data structure that distinguishes `crossModule` from `wholeModule`.

### Structure {#ANALYSIS-GRAPH-STRUCT}

```rust
// crates/basilisk-lsp/src/import_graph.rs
pub struct ImportGraph {
    forward: HashMap<PathBuf, HashSet<PathBuf>>,   // file → files it imports
    reverse: HashMap<PathBuf, HashSet<PathBuf>>,   // file → files that import it
}
```

### Construction {#ANALYSIS-GRAPH-BUILD}

`build_from_index()` walks `ImportInfo.resolved_path` for every file in the `WorkspaceIndex`, populating forward and reverse edges.

### Topological Ordering {#ANALYSIS-GRAPH-TOPO}

`topological_order()` uses Kahn's algorithm to produce an imported-first ordering. Files are analysed in this order so that imported symbols are available before importers are checked.

### Cycle Detection {#ANALYSIS-GRAPH-CYCLES}

`detect_cycles()` uses DFS with white/gray/black coloring. Detected cycles produce an `ImportCycle` diagnostic. Cycles are broken for analysis ordering — one edge is arbitrarily dropped to allow analysis to proceed.

### Transitive Importers {#ANALYSIS-GRAPH-TRANS}

`transitive_importers()` performs BFS over reverse edges. Used for invalidation cascading: when a file changes, all transitive importers may need re-analysis.

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

### Two-Pass Population {#ANALYSIS-SYMBOLS-POP}

`populate_cross_module_symbols()` in `cross_module.rs`:

1. **Export extraction pass**: For each file, `extract_exports()` collects all public symbols (functions, classes, module-level variables) and builds their signatures via `build_function_signature()`.
2. **Import resolution pass**: For each file's `ImportInfo`, look up the target file's exports and populate `imported_symbols` in the importer's `ResolvedModule`.

### Invalidation Cascading {#ANALYSIS-SYMBOLS-INVAL}

When a file changes — whether on disk (file watcher) or in the editor
(`didChange` / `didSave` on an **open** file) — its exports are diffed:

1. Re-analyse the changed file.
2. `exported_symbol_names()` compares old and new exports.
3. If exports changed, re-resolve the workspace and re-check importers
   (`reresolve_imports_and_recheck`) so their cross-module diagnostics refresh
   without a reload. The watcher path uses `reload_and_diff_exports`; the
   open-file path uses `set_open_refresh_dependents` — the watcher's
   `reload_from_disk` skips open files, so editing an open module would
   otherwise leave dependents stale (GitHub #56).
4. If exports unchanged, skip the cascade (most edits don't change public API).

---

## Cross-File LSP Features {#ANALYSIS-CROSSLSP}

Features enabled by `crossModule` that are unavailable or degraded in `wholeModule`:

### Cross-File Go to Definition {#ANALYSIS-CROSSLSP-GOTODEF}

Follow `ImportInfo.resolved_path`, find the symbol's `name_span` in the target `ResolvedModule`. Re-exports are followed across the import chain.

### Cross-File Find All References {#ANALYSIS-CROSSLSP-REFS}

Use import graph reverse edges. For a symbol defined in file A, search all importers of A for usage of that symbol name.

### Cross-File Rename {#ANALYSIS-CROSSLSP-RENAME}

Produces a multi-file `WorkspaceEdit`: definition site + import sites (`from module import old_name` → `from module import new_name`) + all usage sites in importing files.

### Auto-Import Completion {#ANALYSIS-CROSSLSP-IMPORT}

`SymbolIndex` (built in `auto_import.rs`) indexes all workspace exports. When the user types an unknown symbol, completion suggests imports with `additionalTextEdits` that insert the import statement.

---

## Startup Behaviour {#ANALYSIS-STARTUP}

### openFilesOnly Startup {#ANALYSIS-STARTUP-OPEN}

No workspace scan. The server waits passively for `didOpen` notifications.

### wholeModule Startup {#ANALYSIS-STARTUP-WHOLE}

On `initialized`: all `.py` / `.pyi` files under workspace roots are collected (respecting `include`/`exclude`), analysed in parallel, diagnostics published. Progress reported via `window/workDoneProgress`.

### crossModule Startup {#ANALYSIS-STARTUP-CROSS}

Same as `wholeModule`, with an additional pass: the import graph is built from `ImportInfo`, files are topologically sorted, and `populate_cross_module_symbols()` resolves inter-module references. Files whose diagnostics change are re-checked and re-published.

---

## Incremental Updates {#ANALYSIS-INCR}

### didChange {#ANALYSIS-INCR-CHANGE}

Incremental text edits are applied to the in-memory buffer, then parse → resolve → check runs for the changed file. In `crossModule`, direct importers are queued for re-analysis if the exported symbol table changed.

### Import resolution on incremental re-check {#ANALYSIS-INCR-IMPORTS}

The `resolve` step of any incremental re-check (`didOpen`, `didChange`, disk reload, dependent invalidation) MUST resolve third-party and workspace imports against the **same** `ImportSearchPaths` (venv site-packages, workspace members, stub paths, uv registry) that the full workspace scan used. The full scan builds these once and caches them on the workspace index; incremental re-checks reuse the cached value rather than recomputing it (site-packages discovery may touch the filesystem or spawn a subprocess and MUST NOT run per keystroke).

Without this, the syntactic resolver marks every import `Unresolved`, so opening or editing a file resurrects false `BSK-E0010` ("Cannot resolve import … no type information available") in the editor for packages that resolve cleanly on the CLI and during the startup scan. The diagnostics an incremental re-check **publishes** MUST already reflect import resolution — not just the cached symbol table used by navigation features.

### File-Watcher Event {#ANALYSIS-INCR-WATCH}

If the file is open, the event is ignored. Otherwise the file is read from disk; if `source_hash` is unchanged the entry is left as-is. If changed, the pipeline re-runs.

### Debouncing {#ANALYSIS-INCR-DEBOUNCE}

File-watcher events MUST be debounced with a 150 ms delay to avoid thrashing during bulk saves. `didChange` events are NOT debounced — latency matters.

---

## Diagnostic Publishing Contract {#ANALYSIS-PUBLISH}

| Mode | Which files get diagnostics published |
|------|--------------------------------------|
| `openFilesOnly` | Only currently open documents |
| `wholeModule` | All workspace files (open and closed) |
| `crossModule` | All workspace files + any file whose diagnostics changed due to cross-module re-analysis |

When a file is **deleted**, publish empty diagnostics to clear the error panel. When the user switches mode at runtime, clear all diagnostics, re-analyse, re-publish.

---

## LSP Capabilities {#ANALYSIS-CAPS}

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

## Performance Constraints {#ANALYSIS-PERF}

| Metric | Target |
|--------|--------|
| Startup scan (wholeModule, 10 K LOC) | < 2 s |
| Startup scan (wholeModule, 100 K LOC) | < 10 s |
| Single-file incremental update | < 50 ms |
| Diagnostic publish latency (open file) | < 100 ms after last keystroke |
| Memory per file in index | < 500 KB average |

Large workspaces (> 500 K LOC) MAY show a progress notification and allow the user to cancel.

---

## Error Handling {#ANALYSIS-ERRORS}

- If a file cannot be read (permissions, encoding), log a `window/logMessage` warning and skip it. Do not crash.
- If the workspace root does not exist, skip silently.
- If the workspace scan exceeds 30 s, log a warning and continue in degraded mode.
- Circular imports: detect, emit diagnostic, break cycle for ordering.
