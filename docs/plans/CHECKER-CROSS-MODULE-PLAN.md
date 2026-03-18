# Cross-Module Analysis — Plan

> **Spec**: [LSP-ANALYSIS-MODES-SPEC.md](../specs/LSP-ANALYSIS-MODES-SPEC.md)
> **Stubs Spec**: [CHECKER-STUB-RESOLUTION-SPEC.md](../specs/CHECKER-STUB-RESOLUTION-SPEC.md)
> **Branch**: `crossmodule`

---

## Completed

- **Stub infrastructure** (Phase 1) — PEP 561 resolution, typeshed bundling, `.pyi` parsing, config overrides, Python env detection. See [CHECKER-STUB-RESOLUTION-SPEC.md](../specs/CHECKER-STUB-RESOLUTION-SPEC.md).
- **Import graph** (Phase 2) — `ImportGraph` with forward/reverse edges, topological ordering, cycle detection, `ExternalSymbol` model, two-pass cross-module population, invalidation cascading. See [LSP-ANALYSIS-MODES-SPEC.md](../specs/LSP-ANALYSIS-MODES-SPEC.md) §4-5.
- **Cross-file Go to Definition** — follow `resolved_path`, find symbol's `name_span` in target `ResolvedModule`
- **Cross-file Find All References** — use import graph reverse edges, search all importers
- **Auto-import completion** — `SymbolIndex`, `suggest_imports()`, `additionalTextEdits` for import insertion
- **Re-export handling in Go to Definition** — `follow_reexport_chain()` in `navigation.rs`, follows `imported_symbols` chain up to 10 levels
- **Cross-file Rename** — multi-file `WorkspaceEdit` in `navigation.rs`: definition site + import graph importers + source definition via `imported_symbols`
- **Import statement updates for Rename** — `find_identifier_occurrences()` replaces all occurrences including import statements

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Provenance threading touches many rules | Introduce as optional field first; cascade suppression is a central filter in `check()`, not per-rule |
| Salsa is a large architectural change | Independent of other phases — current `DashMap` + `Arc` works without it, Salsa is a performance optimization |
| Cross-file rename scope explosion | Limit to workspace files; warn user about external package references |

---

## Migration Path

A project adopting Basilisk with third-party imports:

1. Install `-stubs` packages (`pip install types-requests`). Basilisk auto-discovers via PEP 561. E0010 disappears.
2. Per-module overrides in `pyproject.toml` suppress noise for untyped packages.
3. Cross-file Go to Definition, Find References, Rename, and auto-import work today.
4. After Phase 4: packages without stubs show one import-site error instead of cascading noise.
5. After Phase 6: run `basilisk stubs generate --all` for best-effort stubs on remaining packages.

### With uv Integration (Future)

See [LSP-UV-INTEGRATION-SPEC.md](../specs/LSP-UV-INTEGRATION-SPEC.md):

1. **Zero config**: auto-detect `uv.lock`, `.python-version`, full dependency graph
2. **Actionable diagnostics**: BSK-E0010 says "run `uv add requests`" with one-click code action
3. **Stub suggestions**: BSK-W0010 offers `uv add --dev types-requests`
4. **Hot reload**: `uv.lock` changes picked up via file watcher — no LSP restart
5. **Workspace support**: uv workspaces map to LSP multi-root with cross-member import resolution

---

## TODO

### Phase 3: Multi-Root Workspace

- [ ] Per-root config — each workspace folder gets its own config resolution
- [ ] Merged index for multi-root — single `WorkspaceIndex` spans all roots, imports cross root boundaries

### Phase 4: Type Provenance

- [ ] Add `TypeProvenance` enum to `crates/basilisk-checker/src/types.rs`
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

- [ ] Runtime introspection mode in `crates/basilisk-stubs/src/generate/runtime.rs`
- [ ] AST-based inference in `crates/basilisk-stubs/src/generate/ast.rs`
- [ ] Hybrid mode in `crates/basilisk-stubs/src/generate/hybrid.rs`
- [ ] Cache management in `crates/basilisk-stubs/src/cache.rs`
- [ ] CLI subcommand — `basilisk stubs generate`, `basilisk stubs status`
