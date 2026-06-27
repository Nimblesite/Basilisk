# Type Provenance, Cascade Suppression & Auto-Stub Generation Plan

## Context

When users import third-party libraries (FastAPI, SQLAlchemy, etc.) that lack type stubs, Basilisk fires imports_unresolved at the import — but then **cascades dozens of downstream errors** for every use of the imported symbols. This noise buries real bugs. The fix: track where type information came from (provenance), fire one error at the import site, and suppress all downstream noise.

Additionally, users need a way to auto-generate best-effort stubs for untyped packages, and all of this must be configurable via `pyproject.toml`.

This plan consolidates TODOs from:
- `docs/plans/CHECKER-CROSS-MODULE-PLAN.md` (Phases 3-6)
- [CHECKER-STUB-RESOLUTION-SPEC.md §STUBRES-PROVENANCE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PROVENANCE) and [§STUBRES-AUTOGEN](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-AUTOGEN)
- [LSP-UV-INTEGRATION-SPEC.md §LSPUV-WORKSPACE](../specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV-WORKSPACE), [§LSPUV-HOVER](../specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV-HOVER), and [§LSPUV-CMDS](../specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV-CMDS)

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

`should_suppress_cascade()` checks if the diagnostic's span references any name in `untyped_names`. imports_unresolved itself is NOT suppressed — only downstream rules (E0012, E0013, etc.).

### 1.6 Tests

- Test that importing an unresolved module fires imports_unresolved exactly once
- Test that using symbols from that unresolved module does NOT fire downstream errors
- Test that resolved imports still fire downstream errors normally
- Test provenance `From` conversions

---

## Phase 2: Provenance in Hover & Diagnostics

**Goal**: Better UX — hover shows where type info came from, diagnostics tagged with provenance.

### 2.1 Add provenance to `Diagnostic` struct

**File**: `crates/basilisk-checker/src/diagnostic.rs`

Add `pub provenance: Option<TypeProvenance>` field (defaults to `None`). Existing rule code unaffected.

### 2.2 Tag imports_unresolved and BSK-E0152 with provenance

**Files**: `crates/basilisk-checker/src/rules/e0010.rs`, `e0152.rs`

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

### 2.5 uv-enriched hover (from [LSP-UV-INTEGRATION-SPEC.md §LSPUV-HOVER](../specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV-HOVER))

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
| `rules."imports_unresolved"` | severity | `"error"` | DONE |
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
| `crates/basilisk-checker/src/rules/e0152/mod.rs` | Tag with provenance |
| `crates/basilisk-lsp/src/cross_module.rs` | Populate provenance from import resolution |
| `crates/basilisk-lsp/src/hover.rs` | Provenance + uv metadata in hover |
| `crates/basilisk-lsp/src/import_resolver.rs` | Add `.basilisk/stubs/` search path |
| `crates/basilisk-stubs/src/generate/*.rs` | New auto-stub generation module |
| `crates/basilisk-cli/src/main.rs` | `basilisk stubs` subcommand |
| `crates/basilisk-config/src/parse.rs` | `auto-stub-mode`, `auto-stub-path` keys |

---

## Verification

1. **Cascade suppression**: Create a Python file that imports an untyped package and uses it extensively. Confirm imports_unresolved fires once at import, zero downstream errors.
2. **Provenance hover**: Hover over stdlib import -> "(typeshed)". Hover over untyped import -> "(no type stubs available)".
3. **Auto-stub generation**: `basilisk stubs generate requests` produces valid `.pyi` in `.basilisk/stubs/`. Re-check shows imports_unresolved cleared, BSK-E0152 shows "(best-effort stub)".
4. **Config**: Set `rules."imports_unresolved" = "warning"` in `pyproject.toml`, confirm severity changes.
5. **uv integration**: In a uv project, confirm hover shows package version and stub status.
6. **One-click code actions**: imports_unresolved and BSK-E0152 diagnostics show quick fix code actions. Clicking the quick fix installs the package/stubs automatically. No CLI commands in help text.
7. **Full CI**: `cargo clippy`, `cargo test`, `cargo fmt --check` all pass.

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
- [x] Tag imports_unresolved diagnostics with `TypeProvenance::Untyped`
- [x] Tag BSK-E0152 diagnostics with provenance
- [x] Tier-based severity adjustment (Tier3 -> Info) in `check_with_config()`
- [x] Provenance annotations in hover tooltips
- [x] uv-enriched hover (package version, stub status) — already done, enhanced with provenance labels
- [x] Tests: hover provenance annotations
- [x] Tests: Tier3 severity downgrade — central filter in `check_with_config()`

### Phase 3: Multi-Root Workspace ✅ DONE
- [x] Add `root_configs: HashMap<PathBuf, BasiliskConfig>` to `WorkspaceIndex`
- [x] `config_for_file()` — finds owning root, returns per-root config
- [x] Auto-load per-root configs on construction (falls back to passed config if no config file)
- [x] All `analyse_with_config()` calls now use `config_for_file()` instead of single config
- [x] Tests: multi-root with different pyproject.toml configs
- [x] Tests: config_for_file falls back to default for unknown paths

### Phase 4: Auto-Stub Generation ✅ DONE
- [x] Create `crates/basilisk-stubs/src/generate/mod.rs` — public API, `StubGenMode` enum
- [x] Runtime introspection mode (`generate/runtime.rs`) — Python subprocess + inspect.signature
- [x] AST-based inference (`generate/ast.rs`) — ruff_python_parser, handles __all__, async, varargs
- [x] Hybrid mode (`generate/hybrid.rs`) — runtime first, AST fallback
- [x] Cache management (`generate/cache.rs`) — source hash invalidation, `.basilisk/stubs/`
- [x] CLI subcommand: `basilisk stubs generate`, `basilisk stubs status`
- [x] Wire `.basilisk/stubs/` into import resolver search path (auto-detected in `from_config()`)
- [x] Add `auto-stub-mode` and `auto-stub-path` to `BasiliskConfig`
- [x] Parse new config keys in `pyproject.toml` and `basilisk.json`
- [x] Tests: runtime stub generation (entries_to_pyi, format_function_stub)
- [x] Tests: AST-based stub generation (annotated, private, __all__, unannotated, async)
- [x] Tests: cache roundtrip, hash determinism
- [x] All clippy, fmt, tests passing

### Phase 4b: Diagnostic Help Text Cleanup ✅ DONE
- [x] Remove "Run `uv add ...`" CLI instructions from imports_unresolved help text (`e0010.rs`)
- [x] Remove "Run `uv sync`" CLI instructions from imports_unresolved help text
- [x] Remove "`uv add --dev types-...`" CLI instructions from BSK-E0152 help text
- [x] Remove "Run `uv lock`" CLI instructions from BSK-W0013 help text
- [x] Remove "run `uv add --dev pytest`" from pytest-not-found messages (init.rs, test_handlers.rs)
- [x] Replace with problem descriptions — the code action is the fix, not a CLI command
- [x] All imports_unresolved/E0152/W0013/W0014 scenarios have corresponding code actions in `code_actions/mod.rs`
- [x] Update tests to match new help text
- [x] All tests passing (18 checker, 57 code action)

### Phase 5: Salsa Integration (Deferred)
- [ ] Add `salsa` dependency to `crates/basilisk-lsp/Cargo.toml`
- [ ] Define Salsa database in `salsa_db.rs`
- [ ] Migrate parse -> Salsa query
- [ ] Migrate resolve -> Salsa query
- [ ] Migrate check -> Salsa query
- [ ] Cross-file invalidation via Salsa
