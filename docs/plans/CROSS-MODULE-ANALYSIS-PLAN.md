# Cross-Module Analysis Plan

> **Spec**: `docs/specs/WHOLE-MODULE-ANALYSIS-SPEC.md` — sections 2.3, 5.3, 8
> **LSP Spec**: `docs/specs/LSP-SPEC.md` — stub strategy, type provenance
> **Depends on**: Whole-module analysis (complete), import resolver (partial)
> **Branch**: `crossmodule`

---

## Current State

Whole-module analysis is complete. The workspace index scans all `.py`/`.pyi` files, caches per-file `ResolvedModule` in `Arc`, and publishes diagnostics for every file. The import resolver (`import_resolver.rs`) can resolve module names to filesystem paths. But:

- **No cross-file symbol sharing** — each file is resolved in isolation
- **No import graph** — resolved imports are not connected into a dependency graph
- **No stub infrastructure** — `basilisk-stubs` is a skeleton returning `None` for everything except basic builtins
- **No typeshed** — stdlib recognition uses a hardcoded `STDLIB_ROOTS` list in `e0010.rs`
- **No PEP 561 support** — no `py.typed` detection, no `-stubs` package discovery
- **No type provenance** — `Unknown` types have no metadata about *why* they're unknown
- **No Salsa** — full re-parse on every change, no memoized incremental computation
- **All LSP handlers are single-file** — definition, references, completion, hover query only the current `ResolvedModule`

### What Exists

| Component | File | Status |
|-----------|------|--------|
| `AnalysisMode::CrossModule` enum variant | `crates/basilisk-lsp/src/config.rs` | Defined, not operational |
| `ImportResolution` enum (`SourcePy`, `StubPyi`, `Unresolved`) | `crates/basilisk-resolver/src/scope.rs` | Defined, always `Unresolved` during resolve |
| `ImportInfo.resolved_path` field | `crates/basilisk-resolver/src/scope.rs` | Defined, populated by import resolver |
| Import resolver (module → filesystem path) | `crates/basilisk-lsp/src/import_resolver.rs` | Working — resolves absolute/relative imports across workspace roots, extraPaths, site-packages |
| `WorkspaceIndex` with `DashMap<PathBuf, FileEntry>` | `crates/basilisk-lsp/src/workspace.rs` | Working — per-file cache with `Arc<ResolvedModule>` |
| `resolve_workspace_imports()` | `crates/basilisk-lsp/src/import_resolver.rs` | Working — updates `ImportInfo.resolution` and `resolved_path` |
| `basilisk-stubs` crate | `crates/basilisk-stubs/src/lib.rs` | Skeleton — `lookup_builtin()` only |
| Suppression system (Layer 1) | `crates/basilisk-checker/src/suppression.rs` | Complete — inline, block, file-level directives |
| Config file reading | `crates/basilisk-lsp/src/config.rs` | Done — reads `pyrightconfig.json`, `pyproject.toml`, `basilisk.json` |

---

## Phase 1: Stub Infrastructure

> **Goal**: Replace the hardcoded stdlib list with real stub resolution. Make `basilisk-stubs` functional.

### 1.1 typeshed Bundling

Replace the hardcoded `STDLIB_ROOTS` in `e0010.rs` with a compiled typeshed index.

| Task | File | Description |
|------|------|-------------|
| Bundle typeshed stdlib stubs at compile time | New: `crates/basilisk-stubs/build.rs` | Read typeshed `.pyi` files, produce a `phf` hash map or serialized lookup table |
| Replace `STDLIB_ROOTS` | `crates/basilisk-checker/src/rules/e0010.rs` | Query compiled typeshed index instead of hardcoded list |
| Add build dependencies | `crates/basilisk-stubs/Cargo.toml` | `phf`, `walkdir` |

**Risk**: typeshed is ~40MB of `.pyi` files. Mitigate by bundling stdlib only initially, compressed with `include_bytes!`. Third-party typeshed stubs are distributed via PyPI anyway (`types-requests`, etc.).

### 1.2 User Stub Paths

Support `stub-paths` config for custom `.pyi` directories (like Pyright's `stubPath`).

| Task | File | Description |
|------|------|-------------|
| Add `stub-paths` to config | `crates/basilisk-lsp/src/config.rs` | Parse from `pyproject.toml [tool.basilisk]` and `basilisk.json` |
| Scan stub paths for `.pyi` files | `crates/basilisk-stubs/src/lib.rs` | New `resolve_user_stub(module: &str, stub_paths: &[PathBuf]) -> Option<PathBuf>` |
| Wire into import resolver | `crates/basilisk-lsp/src/import_resolver.rs` | Check user stubs first in resolution order |

### 1.3 PEP 561 Discovery

Discover type information from installed packages.

| Task | File | Description |
|------|------|-------------|
| Detect `py.typed` markers | `crates/basilisk-stubs/src/lib.rs` | Scan site-packages for packages with `py.typed` marker file |
| Detect stub-only packages | `crates/basilisk-stubs/src/lib.rs` | Scan site-packages for `{module}-stubs/` directories |
| Python env detection | `crates/basilisk-lsp/src/config.rs` | Use configured `python` path or fall back to `python3 -c "import sys; print(sys.path)"` |

### 1.4 PEP 561 Resolution Order

Following the standard, matching Pyright:

1. **User stubs** — `.pyi` files in `stub-paths` directories
2. **User source** — `.py` files in the project (already handled)
3. **Stub-only packages** — installed `foopkg-stubs` packages
4. **Inline-typed packages** — installed packages with `py.typed` marker
5. **Bundled typeshed** — compiled into binary from `basilisk-stubs`
6. **No stubs found** — type resolves to `Unknown`, BSK-E0010 fires

### 1.5 `.pyi` File Parsing

Since Basilisk uses `ruff_python_parser`, the same parser handles `.pyi` files. Only signatures matter — function defs, class defs, variable annotations. Bodies (`...` or `pass`) are ignored. `@overload` is significant.

### 1.6 Stub Resolution Data Model

```rust
// crates/basilisk-stubs/src/lib.rs

pub struct StubResolution {
    pub module: String,
    pub source: StubSource,
    pub pyi_path: Option<PathBuf>,
    pub tier: StubTier,
}

pub enum StubSource {
    UserStub,       // from stub-paths config
    StubPackage,    // from foopkg-stubs
    InlineTyped,    // from py.typed marker
    Typeshed,       // bundled
}

pub enum StubTier {
    Tier1,  // hand-written, verified (typeshed, official stubs)
    Tier2,  // auto-generated, community-reviewed
    Tier3,  // best-effort inference (auto-generated)
}
```

### 1.7 pyproject.toml Config

Foundation for per-module and per-path overrides. Required by stub discovery and suppression system.

```toml
[tool.basilisk]
stub-paths = ["stubs/"]

[tool.basilisk.rules]
"BSK-E0010" = "warning"    # demote globally

[tool.basilisk.per-module-overrides."fastmcp"]
ignore-missing-stubs = true

[tool.basilisk.per-module-overrides."django.*"]
ignore-missing-stubs = true

[tool.basilisk.per-path-overrides."vendor/**"]
rules.disabled = ["BSK-E0010"]
```

| Task | File | Description |
|------|------|-------------|
| Create config parsing crate | New: `crates/basilisk-config/` | `serde` + `toml` — parse `[tool.basilisk]` section |
| Wire into checker | `crates/basilisk-checker/src/lib.rs` | Apply per-module/per-path overrides during `check()` |
| "Disable in project config" code action | `crates/basilisk-lsp/src/code_actions.rs` | Opens/edits `pyproject.toml` |

---

## Phase 2: Import Graph

> **Goal**: Build a dependency graph from resolved imports. This is the foundation for all cross-file features.

### 2.1 Import Graph Construction

After the workspace scan resolves all imports to paths, build an explicit directed graph.

| Task | File | Description |
|------|------|-------------|
| Define `ImportGraph` struct | New: `crates/basilisk-lsp/src/import_graph.rs` | Adjacency list: `HashMap<PathBuf, Vec<PathBuf>>` (file → files it imports) + reverse edges |
| Build graph after workspace scan | `crates/basilisk-lsp/src/workspace.rs` | Walk `ImportInfo.resolved_path` for every file, populate edges |
| Topological sort | `crates/basilisk-lsp/src/import_graph.rs` | Order files for analysis — imported modules before importers |
| Cycle detection | `crates/basilisk-lsp/src/import_graph.rs` | Detect circular imports, emit BSK-W diagnostic, break cycles for analysis ordering |

### 2.2 Cross-File Symbol Table

Share symbols from imported modules so the checker can see types across file boundaries.

| Task | File | Description |
|------|------|-------------|
| Add `imported_symbols` to `ResolvedModule` | `crates/basilisk-resolver/src/scope.rs` | `HashMap<String, ExternalSymbol>` — symbols brought in by imports |
| `ExternalSymbol` struct | `crates/basilisk-resolver/src/scope.rs` | `{ name, kind (fn/class/var), type_annotation, source_path, source_span }` |
| Populate from imported `ResolvedModule` | `crates/basilisk-lsp/src/workspace.rs` | When resolving file F, look up each import's `resolved_path`, fetch its `ResolvedModule` from the index, extract exported symbols |
| Wire into checker | `crates/basilisk-checker/src/lib.rs` | Type checks can query `imported_symbols` for cross-file type info |

### 2.3 Incremental Invalidation

When a file changes, re-analyse files that import it.

| Task | File | Description |
|------|------|-------------|
| Reverse dependency lookup | `crates/basilisk-lsp/src/import_graph.rs` | Given a changed file, find all transitive importers |
| Selective re-analysis | `crates/basilisk-lsp/src/workspace.rs` | On `didChange`, re-analyse changed file, diff exported symbols, re-analyse dependents if exports changed |
| Export diffing | `crates/basilisk-lsp/src/workspace.rs` | Compare old vs new exported symbol table — only cascade if something actually changed |

---

## Phase 3: Cross-File LSP Features

> **Goal**: Make Go to Definition, Find References, Rename, and Completion work across files.

### 3.1 Cross-File Go to Definition

| Task | File | Description |
|------|------|-------------|
| Follow imports to source | `crates/basilisk-lsp/src/definition.rs` | If symbol is an import, look up `resolved_path`, find the symbol's `name_span` in the target `ResolvedModule` |
| Handle re-exports | `crates/basilisk-lsp/src/definition.rs` | If target module re-exports from another module, follow the chain |

### 3.2 Cross-File Find All References

| Task | File | Description |
|------|------|-------------|
| Search all importers | `crates/basilisk-lsp/src/references.rs` | Use import graph reverse edges to find files that import the symbol's module, then search for usage in each |

### 3.3 Cross-File Rename

| Task | File | Description |
|------|------|-------------|
| Multi-file `WorkspaceEdit` | `crates/basilisk-lsp/src/references.rs` | Rename at definition site + all import sites + all usage sites across files |
| Import statement updates | `crates/basilisk-lsp/src/references.rs` | Update `from module import old_name` → `from module import new_name` |

### 3.4 Auto-Import Suggestions

| Task | File | Description |
|------|------|-------------|
| Build workspace symbol index | New: `crates/basilisk-lsp/src/auto_import.rs` | Index all exported symbols from all workspace files |
| Completion with auto-import | `crates/basilisk-lsp/src/completion.rs` | When completing an unknown symbol, suggest imports from the index |
| Import insertion | `crates/basilisk-lsp/src/auto_import.rs` | Generate `TextEdit` to add the import statement at the top of the file |

### 3.5 Multi-Root Workspace Support

| Task | File | Description |
|------|------|-------------|
| Per-root config | `crates/basilisk-lsp/src/config.rs` | Each workspace folder gets its own config resolution |
| Merged index | `crates/basilisk-lsp/src/workspace.rs` | Single `WorkspaceIndex` spans all roots, imports can cross root boundaries |

---

## Phase 4: Type Provenance

> **Goal**: Track where type information came from. Basilisk's key differentiator.

### 4.1 The Problem

```python
import requests   # has types-requests stubs (Tier 1)
import fastmcp    # no stubs at all

r = requests.get("https://example.com")  # Type: Response (from stub)
m = fastmcp.something()                  # Type: Unknown
```

With plain `Unknown`, every downstream use of `m` either silently passes (permissive — defeats Basilisk's purpose) or fires a cascade of errors (strict — 1 bad import = 50 errors). Neither is acceptable.

### 4.2 Types Carry Trust Metadata

```rust
// crates/basilisk-checker/src/types.rs

pub enum TypeProvenance {
    Source,      // from source code annotations or inference
    StubTier1,   // from typeshed, hand-written stubs
    StubTier2,   // from auto-generated, community-reviewed stubs
    StubTier3,   // from best-effort auto-generated stubs
    Untyped,     // no type information available
}

pub struct TrackedType {
    pub ty: InferredType,
    pub provenance: TypeProvenance,
}
```

### 4.3 Diagnostic Behaviour by Provenance

| Provenance | BSK-E0010 | Downstream type errors | LSP hover |
|------------|-----------|----------------------|-----------|
| Source | not fired | normal errors | shows inferred type |
| StubTier1 | not fired | normal errors | shows stub type |
| StubTier2 | not fired | normal errors | shows type + "(auto-generated stub)" |
| StubTier3 | downgraded to info | warnings only | shows type + "(best-effort, may be inaccurate)" |
| Untyped | error (default) | **suppressed** | shows "Unknown (no stubs)" |

**Key insight**: one diagnostic at the import site is worth more than fifty cascading errors at use sites.

### 4.4 Cascade Suppression

Implemented as a filter in the central `check()` function, not per-rule:

```rust
pub fn check(module: &ResolvedModule, ctx: &CheckContext) -> Vec<Diagnostic> {
    let raw = rules::run_all(module, ctx);
    raw.into_iter()
        .filter(|d| !is_suppressed_by_comment(&module.source, d))
        .filter(|d| !is_cascade_from_untyped(d, &module.untyped_imports))
        .collect()
}
```

### 4.5 LSP Integration

- Hover over an untyped import: `fastmcp (no type stubs available)`
- Hover over a Tier 3 stub symbol: `FastMCP (best-effort stub, may be inaccurate)`
- Hover over a typeshed symbol: `os.path.join (typeshed)`

| Task | File | Description |
|------|------|-------------|
| Add `TypeProvenance` and `TrackedType` | `crates/basilisk-checker/src/types.rs` | Core data model |
| Propagate provenance through inference | `crates/basilisk-checker/src/inference.rs` | Every inferred type carries its provenance |
| Cascade suppression filter | `crates/basilisk-checker/src/lib.rs` | Central filter in `check()` |
| Tag diagnostics with provenance | `crates/basilisk-checker/src/rules/e0012.rs`, `e0013.rs` | Rules check provenance before emitting |
| Provenance in hover tooltips | `crates/basilisk-lsp/src/hover.rs` | Show source of type info |

---

## Phase 5: Salsa Integration

> **Goal**: Memoized incremental computation for sub-10ms updates on large projects.

| Task | File | Description |
|------|------|-------------|
| Add `salsa` dependency | `crates/basilisk-lsp/Cargo.toml` | Same framework as rust-analyzer |
| Define Salsa database | New: `crates/basilisk-lsp/src/salsa_db.rs` | Input: source text per file. Tracked: parsed AST, resolved module, diagnostics |
| Migrate parse → Salsa query | `crates/basilisk-lsp/src/workspace.rs` | `parse(file: File) -> ParsedModule` becomes memoized |
| Migrate resolve → Salsa query | `crates/basilisk-lsp/src/workspace.rs` | `resolve(file: File) -> ResolvedModule` becomes memoized |
| Migrate check → Salsa query | `crates/basilisk-lsp/src/workspace.rs` | `check(file: File) -> Vec<Diagnostic>` becomes memoized |
| Cross-file invalidation via Salsa | `crates/basilisk-lsp/src/workspace.rs` | Changing a file only re-computes queries that transitively depend on it |

---

## Phase 6: Auto-Stub Generation

> **Goal**: For packages without any stubs, generate best-effort Tier 3 stubs automatically.

### 6.1 Three Modes

1. **Runtime introspection** — import the package in a subprocess, inspect `__annotations__`, `inspect.signature()`, emit `.pyi`
2. **AST-based inference** — parse package source with `ruff_python_parser`, infer from defaults, docstrings, return statements
3. **Hybrid** — prefer runtime data, fall back to AST

### 6.2 CLI Commands

```bash
basilisk stubs generate requests      # generate stubs for one package
basilisk stubs generate --all         # generate for all untyped imports
basilisk stubs status                 # show stub coverage report
```

Generated stubs go into `.basilisk/stubs/` cache directory, tagged as Tier 3. The provenance system ensures these produce warnings, not false confidence.

| Task | File | Description |
|------|------|-------------|
| Runtime introspection mode | New: `crates/basilisk-stubs/src/generate/runtime.rs` | Subprocess Python, inspect signatures |
| AST-based inference | New: `crates/basilisk-stubs/src/generate/ast.rs` | Parse `.py` source, infer types |
| Hybrid mode | New: `crates/basilisk-stubs/src/generate/hybrid.rs` | Combine runtime + AST |
| Cache management | New: `crates/basilisk-stubs/src/cache.rs` | `.basilisk/stubs/` directory |
| CLI subcommand | `crates/basilisk-cli/src/main.rs` | `basilisk stubs generate`, `basilisk stubs status` |

---

## Implementation Order

| Priority | Phase | Deliverable | Effort | Dependencies |
|----------|-------|-------------|--------|--------------|
| **NOW** | 1.1 | typeshed bundling (replace hardcoded `STDLIB_ROOTS`) | Medium | None |
| **NOW** | 1.7 | pyproject.toml config parsing (`basilisk-config` crate) | Medium | None |
| **NEXT** | 1.2 | User `stub-paths` resolution | Medium | Phase 1.7 |
| **NEXT** | 1.3 | PEP 561 discovery (`-stubs` packages, `py.typed`) | Medium | Phase 1.2 |
| **NEXT** | 1.5 | `.pyi` parsing and type extraction | Large | Phase 1.3 |
| **NEXT** | 2.1 | Import graph construction | Medium | Phase 1 |
| **SOON** | 2.2 | Cross-file symbol table | Large | Phase 2.1 |
| **SOON** | 2.3 | Incremental invalidation | Medium | Phase 2.2 |
| **SOON** | 3.1–3.3 | Cross-file definition, references, rename | Medium each | Phase 2.2 |
| **SOON** | 3.4 | Auto-import suggestions | Hard | Phase 2.2, 3.1 |
| **LATER** | 4 | Type provenance + cascade suppression | Large | Phase 1.5 |
| **LATER** | 5 | Salsa integration | Very Hard | Phase 2 |
| **PHASE 6** | 6 | Auto-stub generation | Large | Phase 4 |

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| typeshed ~40MB bloats binary | Compress with `include_bytes!`, bundle stdlib only initially |
| PEP 561 discovery needs Python env's `sys.path` | Require `python-path` or `venv-path` in config; fall back to `python3 -c "import sys; print(sys.path)"` |
| Provenance threading touches many rules | Introduce as optional field first; cascade suppression is a central filter in `check()`, not per-rule |
| Auto-generated stubs may be wrong | Tier system + provenance = auto-stubs produce warnings, never silent false positives |
| Salsa is a large architectural change | Phase 5 is independent — current `DashMap` + `Arc` approach works without it, Salsa is a performance optimization |
| Circular imports in import graph | Detect cycles, emit diagnostic, break cycles for analysis ordering |
| Cross-file rename scope explosion | Limit to workspace files; warn user about external package references |

---

## Migration Path

A project adopting Basilisk with third-party imports:

1. **Today (suppression system)**: Add `# type: ignore[BSK-E0010]` to imports, or `# basilisk: relaxed` at file top, or per-module overrides in `pyproject.toml`
2. **After Phase 1**: Install `-stubs` packages (`pip install types-requests`). Basilisk auto-discovers them. E0010 disappears.
3. **After Phase 2**: Cross-file Go to Definition, Find References, Rename all work.
4. **After Phase 4**: Packages without stubs show one import-site error instead of cascading noise. LSP shows provenance in hover.
5. **After Phase 6**: Run `basilisk stubs generate --all` for best-effort stubs on remaining packages.
