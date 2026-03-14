# Cross-Module Analysis Plan

> **Spec**: `docs/specs/WHOLE-MODULE-ANALYSIS-SPEC.md` — sections 2.3, 5.3, 8
> **LSP Spec**: `docs/specs/LSP-SPEC.md` — stub strategy, type provenance
> **Future**: `docs/specs/UV-INTEGRATION-SPEC.md` — uv lock file intelligence, package registry, environment detection
> **Depends on**: Whole-module analysis (complete), import resolver (complete)
> **Branch**: `crossmodule`

---

## Architecture

### PEP 561 Resolution Order (implemented in `import_resolver.rs`)

1. **User stubs** — `.pyi` files in `stub-paths` directories
2. **User source** — `.py` files in the project
3. **Stub-only packages** — installed `foopkg-stubs` packages
4. **Inline-typed packages** — installed packages with `py.typed` marker
5. **Bundled typeshed** — compiled into binary from `basilisk-stubs`
6. **No stubs found** — type resolves to `Unknown`, BSK-E0010 fires

### Stub Resolution Data Model (`crates/basilisk-stubs/src/types.rs`)

```rust
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

### Type Provenance (Phase 4)

```rust
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

| Provenance | BSK-E0010 | Downstream type errors | LSP hover |
|------------|-----------|----------------------|-----------|
| Source | not fired | normal errors | shows inferred type |
| StubTier1 | not fired | normal errors | shows stub type |
| StubTier2 | not fired | normal errors | shows type + "(auto-generated stub)" |
| StubTier3 | downgraded to info | warnings only | shows type + "(best-effort, may be inaccurate)" |
| Untyped | error (default) | **suppressed** | shows "Unknown (no stubs)" |

### Config Format (`pyproject.toml`)

```toml
[tool.basilisk]
stub-paths = ["stubs/"]

[tool.basilisk.rules]
"BSK-E0010" = "warning"

[tool.basilisk.per-module-overrides."fastmcp"]
ignore-missing-stubs = true

[tool.basilisk.per-module-overrides."django.*"]
ignore-missing-stubs = true

[tool.basilisk.per-path-overrides."vendor/**"]
rules.disabled = ["BSK-E0010"]
```

### Auto-Stub Generation (Phase 6)

```bash
basilisk stubs generate requests      # generate stubs for one package
basilisk stubs generate --all         # generate for all untyped imports
basilisk stubs status                 # show stub coverage report
```

Generated stubs go into `.basilisk/stubs/` cache directory, tagged as Tier 3.

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| typeshed ~40MB bloats binary | Compress with `include_bytes!`, bundle stdlib only initially |
| PEP 561 discovery needs Python env's `sys.path` | Require `python-path` or `venv-path` in config; fall back to `python3 -c "import sys; print(sys.path)"`. In uv projects, `uv.lock` + `.python-version` eliminate the subprocess entirely |
| Provenance threading touches many rules | Introduce as optional field first; cascade suppression is a central filter in `check()`, not per-rule |
| Auto-generated stubs may be wrong | Tier system + provenance = auto-stubs produce warnings, never silent false positives |
| Salsa is a large architectural change | Phase 5 is independent — current `DashMap` + `Arc` approach works without it, Salsa is a performance optimization |
| Circular imports in import graph | Detect cycles, emit diagnostic, break cycles for analysis ordering |
| Cross-file rename scope explosion | Limit to workspace files; warn user about external package references |

---

## Migration Path

A project adopting Basilisk with third-party imports:

1. **Today (suppression system)**: Add `# type: ignore[BSK-E0010]` to imports, or `# basilisk: relaxed` at file top, or per-module overrides in `pyproject.toml`
2. **Phase 1 DONE**: Install `-stubs` packages (`pip install types-requests`). Basilisk auto-discovers them via PEP 561. E0010 disappears. Per-module overrides in `pyproject.toml` suppress noise for untyped packages.
3. **Phase 2 DONE**: Import graph built, cross-file symbols shared, incremental invalidation operational. Foundation ready for cross-file LSP features.
4. **After Phase 3**: Cross-file Go to Definition, Find References, Rename all work.
5. **After Phase 4**: Packages without stubs show one import-site error instead of cascading noise. LSP shows provenance in hover.
6. **After Phase 6**: Run `basilisk stubs generate --all` for best-effort stubs on remaining packages.

### With uv Integration (Future)

For projects using uv, the migration path becomes significantly smoother — see `docs/specs/UV-INTEGRATION-SPEC.md` and `docs/plans/UV-INTEGRATION-PLAN.md`:

1. **Zero config**: Basilisk auto-detects `uv.lock`, reads `.python-version`, understands the full dependency graph without any configuration.
2. **Actionable diagnostics**: BSK-E0010 says "run `uv add requests`" instead of a generic "module not found". One-click code actions execute the command.
3. **Stub suggestions**: BSK-W0010 detects missing stubs and offers `uv add --dev types-requests` as a code action.
4. **Hot reload**: Changes to `uv.lock` are picked up instantly via file watcher — no LSP restart after installing packages.
5. **Workspace support**: uv workspaces (`[tool.uv.workspace]`) map to LSP multi-root workspaces with correct cross-member import resolution.

uv integration is additive to cross-module analysis — it accelerates Phases 1 (stub discovery), 2 (import graph seeding), and 4 (provenance classification) but does not replace them. Non-uv projects continue to work via the standard PEP 561 pipeline.

---

## TODO

### Phase 1: Stub Infrastructure
- [x] Create `crates/basilisk-stubs/build.rs` — `phf_codegen` generates `phf::Set<&str>` with 220+ CPython 3.12 stdlib modules
- [x] Replace hardcoded `STDLIB_ROOTS` in `e0010.rs` with `basilisk_stubs::is_stdlib_module()`
- [x] Add `phf` + `phf_codegen` build dependencies to `crates/basilisk-stubs/Cargo.toml`
- [x] Add `stub_paths: Vec<PathBuf>` to `WorkspaceConfig` in `crates/basilisk-lsp/src/config.rs`
- [x] Parse `stub-paths` from `pyproject.toml [tool.basilisk]` and `basilisk.json`
- [x] Add `stub_paths` field to `ImportSearchPaths`, implement `try_resolve_stub_only()` in `import_resolver.rs`
- [x] Wire user stubs as first priority in PEP 561 resolution order
- [x] Implement `is_inline_typed_package()` — `py.typed` marker detection in `import_resolver.rs`
- [x] Implement `has_stub_package()` + `try_resolve_stub_package()` — `-stubs` package discovery in `import_resolver.rs`
- [x] Implement full PEP 561 resolution order in `resolve_module()`: user stubs → user source → stub packages → inline-typed → typeshed → unresolved
- [x] Create `StubResolution`, `StubSource`, `StubTier` data model in `crates/basilisk-stubs/src/types.rs`
- [x] Create `crates/basilisk-config/` crate — `serde` + `toml`, parses `basilisk.json` and `[tool.basilisk]` from `pyproject.toml`
- [x] Implement `ModuleOverride` + `module_matches_pattern()` with `django.*` wildcard in `crates/basilisk-config/src/overrides.rs`
- [x] Implement `PathOverride` + `path_matches_pattern()` with `vendor/**` glob in `crates/basilisk-config/src/overrides.rs`
- [x] `.pyi` file parsing — extract signatures, class defs, variable annotations, `@overload` support
- [x] Wire `basilisk-config` overrides into checker — apply per-module/per-path overrides during `check()`
- [x] "Disable in project config" code action — `suppress.rs` offers code action, `commands.rs` handles `basilisk.disableRule` to edit `pyproject.toml`
- [x] Python env detection — `python3 -c "import sys; print(sys.path)"` fallback for site-packages discovery via `detect_python_site_packages()` in `import_resolver.rs`
- [x] E2E tests: config overrides through full pipeline — 9 tests in `e2e_config_overrides.rs` (global severity, per-module, per-path, combined)
- [x] E2E tests: stub resolution through full pipeline — 11 tests in `e2e_stub_resolution.rs` (PEP 561, stub priority, round-trip resolve+parse)

### Phase 2: Import Graph
- [x] Create `ImportGraph` struct in `crates/basilisk-lsp/src/import_graph.rs` — `HashMap<PathBuf, HashSet<PathBuf>>` forward + reverse edges
- [x] Implement `build_from_index()` — walk `ImportInfo.resolved_path` for every file, populate edges
- [x] Implement `topological_order()` — Kahn's algorithm, imported-first ordering
- [x] Implement `detect_cycles()` — DFS with white/gray/black coloring, `ImportCycle` struct
- [x] Implement `transitive_importers()` — BFS over reverse edges
- [x] Wire `build_import_graph()` into `WorkspaceIndex` in `workspace.rs`
- [x] Create `ExternalSymbol` struct in `crates/basilisk-resolver/src/scope/external_symbol.rs` — `ExternalSymbolKind` (Function/Class/Variable/ReExport), name, type_annotation, source_path, source_span, signature
- [x] Add `imported_symbols: HashMap<String, ExternalSymbol>` to `ResolvedModule`
- [x] Create `crates/basilisk-lsp/src/cross_module.rs` — `extract_exports()`, `build_function_signature()`, `populate_cross_module_symbols()` (two-pass algorithm)
- [x] Implement `invalidate_dependents()` in `workspace.rs` — re-analyse changed file, cascade to transitive importers if exports changed
- [x] Implement `exported_symbol_names()` in `workspace.rs` — export diffing, skip cascade if unchanged
- [x] Wire `imported_symbols` into checker rules — e0018 checks cross-module symbols, init.rs calls `populate_cross_module_symbols()` + `recheck_with_cross_module_symbols()`

### Phase 3: Cross-File LSP Features
- [x] Cross-file Go to Definition — follow `resolved_path`, find symbol's `name_span` in target `ResolvedModule`
- [ ] Handle re-exports in Go to Definition — follow import chain across modules
- [x] Cross-file Find All References — use import graph reverse edges, search all importers for symbol usage
- [ ] Cross-file Rename — multi-file `WorkspaceEdit`: definition site + import sites + usage sites
- [ ] Import statement updates for Rename — `from module import old_name` → `from module import new_name`
- [ ] Build workspace symbol index in `auto_import.rs` — index all exported symbols from all workspace files
- [ ] Auto-import completion — suggest imports from workspace symbol index for unknown symbols
- [ ] Import insertion — generate `TextEdit` to add import statement at top of file
- [ ] Per-root config — each workspace folder gets its own config resolution
- [ ] Merged index for multi-root — single `WorkspaceIndex` spans all roots, imports cross root boundaries

### Phase 4: Type Provenance
- [ ] Add `TypeProvenance` enum (Source/StubTier1/StubTier2/StubTier3/Untyped) to `crates/basilisk-checker/src/types.rs`
- [ ] Add `TrackedType` struct — `InferredType` + `TypeProvenance`
- [ ] Propagate provenance through inference in `crates/basilisk-checker/src/inference.rs`
- [ ] Cascade suppression filter in `check()` — suppress downstream errors from untyped imports
- [ ] Tag diagnostics with provenance in `e0012.rs`, `e0013.rs`
- [ ] Provenance in hover tooltips — show "(typeshed)", "(no stubs)", "(best-effort)" in `hover.rs`

### Phase 5: Salsa Integration
- [ ] Add `salsa` dependency to `crates/basilisk-lsp/Cargo.toml`
- [ ] Define Salsa database in `crates/basilisk-lsp/src/salsa_db.rs` — input: source text, tracked: AST, resolved module, diagnostics
- [ ] Migrate parse → Salsa query — `parse(file) -> ParsedModule` memoized
- [ ] Migrate resolve → Salsa query — `resolve(file) -> ResolvedModule` memoized
- [ ] Migrate check → Salsa query — `check(file) -> Vec<Diagnostic>` memoized
- [ ] Cross-file invalidation via Salsa — changing a file only re-computes transitive dependents

### Phase 6: Auto-Stub Generation
- [ ] Runtime introspection mode in `crates/basilisk-stubs/src/generate/runtime.rs` — subprocess Python, `inspect.signature()`
- [ ] AST-based inference in `crates/basilisk-stubs/src/generate/ast.rs` — parse `.py` source, infer types
- [ ] Hybrid mode in `crates/basilisk-stubs/src/generate/hybrid.rs` — prefer runtime, fall back to AST
- [ ] Cache management in `crates/basilisk-stubs/src/cache.rs` — `.basilisk/stubs/` directory
- [ ] CLI subcommand — `basilisk stubs generate`, `basilisk stubs status`
