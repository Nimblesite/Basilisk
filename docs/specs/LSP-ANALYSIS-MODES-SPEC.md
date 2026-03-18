# LSP Analysis Modes — Specification

> **Scope**: How the LSP server decides which files to analyse and how symbol graphs are shared
> **Related**: [LSP-ARCHITECTURE-SPEC.md](LSP-ARCHITECTURE-SPEC.md) — LSP features and protocol

---

## 1. Analysis Modes

Three modes govern which files are analysed and how symbol information flows between them.

### 1.1 `openFilesOnly`

| Property | Value |
|----------|-------|
| **Scope** | Files currently open in the editor |
| **Trigger** | `didOpen`, `didChange`, `didSave` notifications |
| **Symbol graph** | Per-file, no cross-file sharing |
| **Startup scan** | None |
| **Performance cost** | Minimal — only active documents are analysed |

Diagnostics are published only for open documents. Suitable for large monorepos where full workspace analysis is too expensive.

### 1.2 `wholeModule` (default)

| Property | Value |
|----------|-------|
| **Scope** | All `.py` / `.pyi` files reachable from workspace roots, respecting `include`/`exclude` config |
| **Trigger** | Startup scan + `didOpen` / `didChange` / `didSave` / file-watcher events |
| **Symbol graph** | Per-file `ResolvedModule` cached in the workspace index; updated incrementally on change |
| **Startup scan** | Full workspace scan; diagnostics published for every file |
| **Performance cost** | Higher startup cost; incremental updates are fast |

This is the default mode. It corresponds to how Pyright's `basic` / `standard` mode works: the entire project is indexed and diagnostics are visible for **all** files, not just open ones.

### 1.3 `crossModule`

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

## 2. Configuration

### 2.1 Config Sources

```json
{ "analysisMode": "wholeModule" }
```

| Value | Meaning |
|-------|---------|
| `"openFilesOnly"` | Analyse only open documents |
| `"wholeModule"` | Analyse all workspace files (default) |
| `"crossModule"` | Cross-file import graph analysis |

Default: `"wholeModule"`. Basilisk is strict by default — the user must explicitly opt down to `openFilesOnly`.

### 2.2 Config Priority

Config resolution order (highest wins):

1. Editor workspace setting (`basilisk.analysisMode`)
2. `analysisMode` in `basilisk.json`
3. `analysisMode` in `[tool.basilisk]` section of `pyproject.toml`
4. Hard default: `wholeModule`

---

## 3. Workspace Index

`wholeModule` and `crossModule` modes both require a **workspace index** — a persistent, process-scoped data structure that holds the resolved state of every file in the workspace.

### 3.1 Structure

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

### 3.2 Invalidation

A `FileEntry` is invalidated when:

- Its on-disk content changes (file-watcher event) AND `source_hash` changes
- The editor sends a `didChange` notification for it
- Any of its direct importers are invalidated (`crossModule` only)

When invalidated: re-parse, re-resolve, re-check, update `source_hash` and `diagnostics`, re-publish.

### 3.3 Open-File Priority

When a file is open in the editor, the in-memory text (from `didOpen`/`didChange`) is authoritative. File-watcher events for the same path are silently ignored as long as `is_open == true`. When closed (`didClose`), on-disk text is re-read to rebuild the `FileEntry`.

---

## 4. Import Graph (`crossModule`)

The import graph is the core data structure that distinguishes `crossModule` from `wholeModule`.

### 4.1 Structure

```rust
// crates/basilisk-lsp/src/import_graph.rs
pub struct ImportGraph {
    forward: HashMap<PathBuf, HashSet<PathBuf>>,   // file → files it imports
    reverse: HashMap<PathBuf, HashSet<PathBuf>>,   // file → files that import it
}
```

### 4.2 Construction

`build_from_index()` walks `ImportInfo.resolved_path` for every file in the `WorkspaceIndex`, populating forward and reverse edges.

### 4.3 Topological Ordering

`topological_order()` uses Kahn's algorithm to produce an imported-first ordering. Files are analysed in this order so that imported symbols are available before importers are checked.

### 4.4 Cycle Detection

`detect_cycles()` uses DFS with white/gray/black coloring. Detected cycles produce an `ImportCycle` diagnostic. Cycles are broken for analysis ordering — one edge is arbitrarily dropped to allow analysis to proceed.

### 4.5 Transitive Importers

`transitive_importers()` performs BFS over reverse edges. Used for invalidation cascading: when a file changes, all transitive importers may need re-analysis.

---

## 5. Cross-Module Symbol Sharing

### 5.1 External Symbols

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

### 5.2 Two-Pass Population Algorithm

`populate_cross_module_symbols()` in `cross_module.rs`:

1. **Export extraction pass**: For each file, `extract_exports()` collects all public symbols (functions, classes, module-level variables) and builds their signatures via `build_function_signature()`.
2. **Import resolution pass**: For each file's `ImportInfo`, look up the target file's exports and populate `imported_symbols` in the importer's `ResolvedModule`.

### 5.3 Invalidation Cascading

When a file changes:

1. Re-analyse the changed file
2. `exported_symbol_names()` compares old and new exports
3. If exports changed, `invalidate_dependents()` queues transitive importers for re-analysis
4. If exports unchanged, skip cascade (most edits don't change public API)

---

## 6. Cross-File LSP Features

Features enabled by `crossModule` that are unavailable or degraded in `wholeModule`:

### 6.1 Cross-File Go to Definition

Follow `ImportInfo.resolved_path`, find the symbol's `name_span` in the target `ResolvedModule`. Re-exports are followed across the import chain.

### 6.2 Cross-File Find All References

Use import graph reverse edges. For a symbol defined in file A, search all importers of A for usage of that symbol name.

### 6.3 Cross-File Rename

Produces a multi-file `WorkspaceEdit`: definition site + import sites (`from module import old_name` → `from module import new_name`) + all usage sites in importing files.

### 6.4 Auto-Import Completion

`SymbolIndex` (built in `auto_import.rs`) indexes all workspace exports. When the user types an unknown symbol, completion suggests imports with `additionalTextEdits` that insert the import statement.

---

## 7. Startup Behaviour

### 7.1 `openFilesOnly`

No workspace scan. The server waits passively for `didOpen` notifications.

### 7.2 `wholeModule`

On `initialized`: all `.py` / `.pyi` files under workspace roots are collected (respecting `include`/`exclude`), analysed in parallel, diagnostics published. Progress reported via `window/workDoneProgress`.

### 7.3 `crossModule`

Same as `wholeModule`, with an additional pass: the import graph is built from `ImportInfo`, files are topologically sorted, and `populate_cross_module_symbols()` resolves inter-module references. Files whose diagnostics change are re-checked and re-published.

---

## 8. Incremental Updates

### 8.1 On `didChange` (all modes)

Incremental text edits are applied to the in-memory buffer, then parse → resolve → check runs for the changed file. In `crossModule`, direct importers are queued for re-analysis if the exported symbol table changed.

### 8.2 On File-Watcher Event (`wholeModule` / `crossModule`)

If the file is open, the event is ignored. Otherwise the file is read from disk; if `source_hash` is unchanged the entry is left as-is. If changed, the pipeline re-runs.

### 8.3 Debouncing

File-watcher events MUST be debounced with a 150 ms delay to avoid thrashing during bulk saves. `didChange` events are NOT debounced — latency matters.

---

## 9. Diagnostic Publishing Contract

| Mode | Which files get diagnostics published |
|------|--------------------------------------|
| `openFilesOnly` | Only currently open documents |
| `wholeModule` | All workspace files (open and closed) |
| `crossModule` | All workspace files + any file whose diagnostics changed due to cross-module re-analysis |

When a file is **deleted**, publish empty diagnostics to clear the error panel. When the user switches mode at runtime, clear all diagnostics, re-analyse, re-publish.

---

## 10. LSP Capabilities

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

## 11. Performance Constraints

| Metric | Target |
|--------|--------|
| Startup scan (wholeModule, 10 K LOC) | < 2 s |
| Startup scan (wholeModule, 100 K LOC) | < 10 s |
| Single-file incremental update | < 50 ms |
| Diagnostic publish latency (open file) | < 100 ms after last keystroke |
| Memory per file in index | < 500 KB average |

Large workspaces (> 500 K LOC) MAY show a progress notification and allow the user to cancel.

---

## 12. Error Handling

- If a file cannot be read (permissions, encoding), log a `window/logMessage` warning and skip it. Do not crash.
- If the workspace root does not exist, skip silently.
- If the workspace scan exceeds 30 s, log a warning and continue in degraded mode.
- Circular imports: detect, emit diagnostic, break cycle for ordering.
