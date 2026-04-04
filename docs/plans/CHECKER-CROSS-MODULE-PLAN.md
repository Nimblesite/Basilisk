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
4. After type provenance: packages without stubs show one import-site error instead of cascading noise. See [LSP-STUBBING-PLAN.md](LSP-STUBBING-PLAN.md).
5. After auto-stub generation: run `basilisk stubs generate --all` for best-effort stubs on remaining packages. See [LSP-STUBBING-PLAN.md](LSP-STUBBING-PLAN.md).

### With uv Integration (Future)

See [LSP-UV-INTEGRATION-SPEC.md](../specs/LSP-UV-INTEGRATION-SPEC.md):

1. **Zero config**: auto-detect `uv.lock`, `.python-version`, full dependency graph
2. **Actionable diagnostics**: BSK-E0010 says "run `uv add requests`" with one-click code action
3. **Stub suggestions**: BSK-W0010 offers `uv add --dev types-requests`
4. **Hot reload**: `uv.lock` changes picked up via file watcher — no LSP restart
5. **Workspace support**: uv workspaces map to LSP multi-root with cross-member import resolution

---

## TODO

Phases 3–6 (Multi-Root Workspace, Type Provenance, Salsa Integration, Auto-Stub Generation) have been consolidated into [LSP-STUBBING-PLAN.md](LSP-STUBBING-PLAN.md).
