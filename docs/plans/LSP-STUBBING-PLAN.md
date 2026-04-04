# Type Provenance, Cascade Suppression & Auto-Stub Generation Plan

## Context

When users import third-party libraries (FastAPI, SQLAlchemy, etc.) that lack type stubs, Basilisk fires BSK-E0010 at the import — but then **cascades dozens of downstream errors** for every use of the imported symbols. This noise buries real bugs. The fix: track where type information came from (provenance), fire one error at the import site, and suppress all downstream noise.

Additionally, users need a way to auto-generate best-effort stubs for untyped packages, and all of this must be configurable via `pyproject.toml`.

This plan consolidates TODOs from:
- `docs/plans/CHECKER-CROSS-MODULE-PLAN.md` (Phases 3-6)
- `docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md` (Sections 3, 6)
- `docs/specs/LSP-UV-INTEGRATION-SPEC.md` (Sections 5, 7, 8)

---

## Phase 1: TypeProvenance + Cascade Suppression (Highest Value)

**Goal**: One error at the import site. Zero cascading noise from untyped imports.

### 1.1 Add `TypeProvenance` enum

**File**: `crates/basilisk-stubs/src/types.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeProvenance {
    Source,      // from source code annotations or inference
    StubTier1,   // from typeshed, hand-written stubs
    StubTier2,   // from auto-generated, community-reviewed stubs
    StubTier3,   // from best-effort auto-generated stubs
    Untyped,     // no type information available
}
```

Add `From<(&StubSource, &StubTier)>` conversion. Export from `crates/basilisk-stubs/src/lib.rs`.

### 1.2 Add provenance to `ExternalSymbol`

**File**: `crates/basilisk-resolver/src/scope/external_symbol.rs`

Add `pub provenance: Option<basilisk_stubs::TypeProvenance>` field. This requires adding `basilisk-stubs` as a dependency of `basilisk-resolver` (safe — no cycle, `basilisk-stubs` is a leaf crate).

### 1.3 Populate provenance during cross-module resolution

**File**: `crates/basilisk-lsp/src/cross_module.rs`

When building `imported_symbols`, derive provenance from `ImportInfo.resolution`:
- `ImportResolution::Unresolved` -> `TypeProvenance::Untyped`
- `ImportResolution::StubPyi` -> look up `StubTier` from resolution context -> `StubTier1`/`StubTier2`/`StubTier3`
- `ImportResolution::SourcePy` -> `TypeProvenance::Source`

### 1.4 Build untyped-symbol set in checker

**File**: `crates/basilisk-checker/src/lib.rs` (in `check_with_config()`)

Before the existing diagnostic filter chain:
```rust
let untyped_names: HashSet<String> = module.imports.iter()
    .filter(|i| i.resolution == ImportResolution::Unresolved)
    .flat_map(|i| i.names.iter().cloned())
    .collect();
```

### 1.5 Cascade suppression filter

**File**: `crates/basilisk-checker/src/lib.rs` (in `check_with_config()`)

New filter step after per-module overrides, before inline overrides:
```rust
if !untyped_names.is_empty() && should_suppress_cascade(&diag, &untyped_names, source) {
    continue; // suppress downstream error from untyped import
}
```

`should_suppress_cascade()` checks if the diagnostic's span references any name in `untyped_names`. BSK-E0010 itself is NOT suppressed — only downstream rules (E0012, E0013, etc.).

### 1.6 Tests

- Test that importing an unresolved module fires BSK-E0010 exactly once
- Test that using symbols from that unresolved module does NOT fire downstream errors
- Test that resolved imports still fire downstream errors normally
- Test provenance `From` conversions

---

## Phase 2: Provenance in Hover & Diagnostics

**Goal**: Better UX — hover shows where type info came from, diagnostics tagged with provenance.

### 2.1 Add provenance to `Diagnostic` struct

**File**: `crates/basilisk-checker/src/diagnostic.rs`

Add `pub provenance: Option<TypeProvenance>` field (defaults to `None`). Existing rule code unaffected.

### 2.2 Tag BSK-E0010 and BSK-W0010 with provenance

**Files**: `crates/basilisk-checker/src/rules/e0010.rs`, `w0010.rs`

Set `provenance: Some(TypeProvenance::Untyped)` on emitted diagnostics.

### 2.3 Tier-based severity adjustment

**File**: `crates/basilisk-checker/src/lib.rs`

In the filter chain: if a diagnostic's provenance is `StubTier3`, downgrade severity to `Info`. Central filter, not per-rule.

### 2.4 Provenance annotations in hover

**File**: `crates/basilisk-lsp/src/hover.rs`

When hovering over an imported symbol, look up `imported_symbols` provenance and append:
- `Untyped` -> `" (no type stubs available)"`
- `StubTier3` -> `" (best-effort stub, may be inaccurate)"`
- `StubTier1` + typeshed -> `" (typeshed)"`

### 2.5 uv-enriched hover (from LSP-UV-INTEGRATION-SPEC section 8)

When `PackageRegistry` is available, also show:
- Package version and direct/transitive classification
- Stub package status (installed or available)

### 2.6 Tests

- Test hover output includes provenance annotations
- Test Tier3 diagnostics downgraded to Info
- Test hover with uv package metadata

---

## Phase 3: Multi-Root Workspace (from CHECKER-CROSS-MODULE-PLAN Phase 3)

### 3.1 Per-root config resolution

Each workspace folder gets its own `BasiliskConfig` resolution (its own `pyproject.toml [tool.basilisk]`).

### 3.2 Merged WorkspaceIndex for multi-root

Single `WorkspaceIndex` spans all roots. Imports cross root boundaries correctly.

### 3.3 Tests

- Test multi-root workspace with different configs per root
- Test cross-root import resolution

---

## Phase 4: Auto-Stub Generation (from CHECKER-CROSS-MODULE-PLAN Phase 6)

**Goal**: `basilisk stubs generate` produces best-effort `.pyi` files for untyped packages.

### 4.1 Module structure

**New directory**: `crates/basilisk-stubs/src/generate/`

- `mod.rs` — public API, `StubGenMode` enum
- `runtime.rs` — `inspect.signature()` via Python subprocess
- `ast.rs` — parse `.py` source with `ruff_python_parser`, extract signatures
- `hybrid.rs` — try runtime first, fall back to AST
- `cache.rs` — `.basilisk/stubs/` cache management, keyed by (source hash, python version, mode)

### 4.2 Runtime introspection (`runtime.rs`)

Spawns `python -c "import {module}; import inspect; ..."`, parses JSON output. Handles:
- Configurable timeout (default 10s)
- Import failure (module has side effects)
- C extensions where `inspect.signature()` fails

### 4.3 AST-based inference (`ast.rs`)

Uses `ruff_python_parser` to parse `.py` files. Extracts:
- Function signatures (params + annotations if present, else `Any`)
- Class definitions with methods
- Module-level variable annotations
- `__all__` for export filtering

### 4.4 Hybrid mode (`hybrid.rs`)

Try runtime first; for any function where introspection fails, fall back to AST. Merge results.

### 4.5 Cache management (`cache.rs`)

Generated stubs go to `.basilisk/stubs/{module_name}.pyi`. On cache hit (same source hash), skip regeneration.

### 4.6 CLI subcommand

**File**: `crates/basilisk-cli/src/main.rs`

```
basilisk stubs generate requests        # one package
basilisk stubs generate --all           # all untyped imports
basilisk stubs status                   # stub coverage report
```

### 4.7 Wire into import resolution

**File**: `crates/basilisk-lsp/src/import_resolver.rs`

Add `.basilisk/stubs/` as search path (after user stubs, before site-packages). Tag as `StubTier::Tier3`.

### 4.8 pyproject.toml configuration

**File**: `crates/basilisk-config/src/parse.rs`

New config keys under `[tool.basilisk]`:
```toml
[tool.basilisk]
auto-stub-mode = "hybrid"          # "runtime" | "ast" | "hybrid" | "disabled"
auto-stub-path = ".basilisk/stubs" # where generated stubs go
```

### 4.9 Tests

- Test runtime introspection generates valid `.pyi`
- Test AST-based generation for annotated and unannotated code
- Test hybrid fallback behavior
- Test cache hit/miss
- Test CLI subcommand end-to-end
- Test generated stubs resolve correctly in import resolver

---

## Phase 5: Salsa Integration (from CHECKER-CROSS-MODULE-PLAN Phase 5) — Deferred

Performance optimization. Current `DashMap + Arc` works correctly. Defer until Phases 1-4 are stable.

When ready:
- Add `salsa` to `crates/basilisk-lsp/Cargo.toml`
- Define Salsa database in `crates/basilisk-lsp/src/salsa_db.rs`
- Migrate parse/resolve/check to `#[salsa::tracked]` queries
- Cross-file invalidation becomes automatic

---

## Configuration Summary (pyproject.toml)

All stub/provenance config via `[tool.basilisk]` — already partially implemented in `crates/basilisk-config/`:

| Key | Type | Default | Status |
|-----|------|---------|--------|
| `stub-paths` | `string[]` | `[]` | DONE |
| `rules."BSK-E0010"` | severity | `"error"` | DONE |
| `per-module-overrides.{mod}.ignore-missing-stubs` | `bool` | `false` | DONE |
| `per-path-overrides.{glob}.disabled` | `string[]` | `[]` | DONE |
| `uv.stub-suggestions` | `bool` | `true` | DONE |
| `uv.dependency-diagnostics` | `bool` | `false` | DONE |
| `auto-stub-mode` | `string` | `"hybrid"` | TODO (Phase 4) |
| `auto-stub-path` | `string` | `".basilisk/stubs"` | TODO (Phase 4) |

---

## Critical Files

| File | Changes |
|------|---------|
| `crates/basilisk-stubs/src/types.rs` | Add `TypeProvenance` enum |
| `crates/basilisk-stubs/src/lib.rs` | Export `TypeProvenance` |
| `crates/basilisk-resolver/src/scope/external_symbol.rs` | Add `provenance` field |
| `crates/basilisk-resolver/Cargo.toml` | Add `basilisk-stubs` dependency |
| `crates/basilisk-checker/src/lib.rs` | Cascade suppression filter in `check_with_config()` |
| `crates/basilisk-checker/src/diagnostic.rs` | Add `provenance` field to `Diagnostic` |
| `crates/basilisk-checker/src/rules/e0010.rs` | Tag with provenance |
| `crates/basilisk-checker/src/rules/w0010.rs` | Tag with provenance |
| `crates/basilisk-lsp/src/cross_module.rs` | Populate provenance from import resolution |
| `crates/basilisk-lsp/src/hover.rs` | Provenance + uv metadata in hover |
| `crates/basilisk-lsp/src/import_resolver.rs` | Add `.basilisk/stubs/` search path |
| `crates/basilisk-stubs/src/generate/*.rs` | New auto-stub generation module |
| `crates/basilisk-cli/src/main.rs` | `basilisk stubs` subcommand |
| `crates/basilisk-config/src/parse.rs` | `auto-stub-mode`, `auto-stub-path` keys |

---

## Verification

1. **Cascade suppression**: Create a Python file that imports an untyped package and uses it extensively. Confirm BSK-E0010 fires once at import, zero downstream errors.
2. **Provenance hover**: Hover over stdlib import -> "(typeshed)". Hover over untyped import -> "(no type stubs available)".
3. **Auto-stub generation**: `basilisk stubs generate requests` produces valid `.pyi` in `.basilisk/stubs/`. Re-check shows BSK-E0010 cleared, BSK-W0010 shows "(best-effort stub)".
4. **Config**: Set `rules."BSK-E0010" = "warning"` in `pyproject.toml`, confirm severity changes.
5. **uv integration**: In a uv project, confirm hover shows package version and stub status.
6. **Full CI**: `cargo clippy`, `cargo test`, `cargo fmt --check` all pass.

---

## TODO List

### Phase 1: TypeProvenance + Cascade Suppression ✅ DONE
- [x] Add `TypeProvenance` enum to `crates/basilisk-stubs/src/types.rs`
- [x] Add `From<(&StubSource, &StubTier)>` impl for `TypeProvenance`
- [x] Export `TypeProvenance` from `crates/basilisk-stubs/src/lib.rs`
- [x] Add `basilisk-stubs` dependency to `crates/basilisk-resolver/Cargo.toml`
- [x] Add `provenance: Option<TypeProvenance>` to `ExternalSymbol`
- [x] Populate provenance in `crates/basilisk-lsp/src/cross_module.rs`
- [x] Build untyped-symbol set in `check_with_config()`
- [x] Implement `should_suppress_cascade()` filter
- [x] Wire cascade suppression into diagnostic filter chain
- [x] Tests: cascade suppression (untyped import = 1 error, not N)
- [x] Tests: resolved imports still fire downstream errors

### Phase 2: Provenance in Hover & Diagnostics ✅ DONE
- [x] Add `provenance: Option<TypeProvenance>` to `Diagnostic` struct
- [x] Tag BSK-E0010 diagnostics with `TypeProvenance::Untyped`
- [x] Tag BSK-W0010 diagnostics with provenance
- [x] Tier-based severity adjustment (Tier3 -> Info) in `check_with_config()`
- [x] Provenance annotations in hover tooltips
- [x] uv-enriched hover (package version, stub status) — already done, enhanced with provenance labels
- [x] Tests: hover provenance annotations
- [x] Tests: Tier3 severity downgrade — central filter in `check_with_config()`

### Phase 3: Multi-Root Workspace — IN PROGRESS
- [ ] Add `root_configs: HashMap<PathBuf, BasiliskConfig>` to `WorkspaceIndex`
- [ ] Load config per-root in `initialize()` and `did_change_workspace_folders()`
- [ ] Add `owner_root` to `FileEntry` for file-to-root mapping
- [ ] Use per-root config in `check_with_config()` based on file's owner root
- [ ] Tests: multi-root with different configs
- [ ] Tests: cross-root import resolution

### Phase 4: Auto-Stub Generation
- [ ] Create `crates/basilisk-stubs/src/generate/mod.rs`
- [ ] Runtime introspection mode (`generate/runtime.rs`)
- [ ] AST-based inference (`generate/ast.rs`)
- [ ] Hybrid mode (`generate/hybrid.rs`)
- [ ] Cache management (`generate/cache.rs`)
- [ ] CLI subcommand: `basilisk stubs generate`, `basilisk stubs status`
- [ ] Wire `.basilisk/stubs/` into import resolver search path
- [ ] Add `auto-stub-mode` and `auto-stub-path` to `BasiliskConfig`
- [ ] Parse new config keys in `pyproject.toml` and `basilisk.json`
- [ ] Tests: runtime stub generation
- [ ] Tests: AST-based stub generation
- [ ] Tests: hybrid fallback
- [ ] Tests: cache hit/miss
- [ ] Tests: CLI e2e
- [ ] Tests: generated stubs resolve in import resolver

### Phase 5: Salsa Integration (Deferred)
- [ ] Add `salsa` dependency to `crates/basilisk-lsp/Cargo.toml`
- [ ] Define Salsa database in `salsa_db.rs`
- [ ] Migrate parse -> Salsa query
- [ ] Migrate resolve -> Salsa query
- [ ] Migrate check -> Salsa query
- [ ] Cross-file invalidation via Salsa
