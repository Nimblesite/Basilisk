# Type Stub Strategy

**Status**: Partially Implemented
**Date**: 2026-03-06
**Last Updated**: 2026-03-06

---

## The Problem

Basilisk fires BSK-E0010 for every import that isn't in a hardcoded stdlib whitelist (`STDLIB_ROOTS` in `crates/basilisk-checker/src/rules/e0010.rs`). This is correct behavior -- untyped imports are a hole in the type system. But it's currently unusable at scale.

A real project importing `requests`, `fastmcp`, `django`, `pydantic`, and `sqlalchemy` gets five E0010 errors before a single line of application code is checked. The only escape hatch is `# type: ignore`, which:

- Suppresses **all** diagnostics on the line, not just E0010
- Has no error-code awareness
- Offers no per-file or per-project control
- Provides no path toward actually resolving the missing types

The `basilisk-stubs` crate (`crates/basilisk-stubs/src/lib.rs`) is a placeholder that returns `None` for every lookup. There is no stub discovery, no PEP 561 support, no typeshed bundling.

We need a layered strategy that provides immediate relief, builds toward full stub resolution, and introduces Basilisk's differentiating feature: **type provenance tracking**.

---

## What the Competition Does

### Pyright

- Searches `.pyi` stubs before `.py` files
- `stubPath` config for custom stub directories (default: `./typings`)
- Discovers PEP 561 packages (`py.typed` markers, `-stubs` packages)
- Falls back to `Unknown` when no stubs found
- Per-file: `# pyright: reportMissingTypeStubs=false`
- Per-project: `pyrightconfig.json` severity overrides
- Quick-fix code actions to generate stub skeletons

### mypy

- `--ignore-missing-imports` flag (global suppression)
- Per-module config: `[mypy-package.*] ignore_missing_imports = True`
- `--install-types` auto-discovers and installs stub packages from PyPI
- `# type: ignore[import-untyped]` per-line with error code
- `# mypy: disable-error-code="import-untyped"` per-file
- Treats missing stubs as `Any` (permissive -- the opposite of what we want)

### PEP 561

Defines three ways to distribute type information:

1. **Inline types** -- `.py` files with annotations + `py.typed` marker at package root
2. **Stub files** -- `.pyi` alongside runtime code + `py.typed` marker
3. **Stub-only packages** -- separate `foopkg-stubs` distribution (e.g., `types-requests`)

Standard resolution order:
1. User-specified stubs
2. User code
3. Stub-only packages (`*-stubs`)
4. Packages with `py.typed` marker
5. Typeshed (stdlib and third-party)

### typeshed

Community-maintained repository of `.pyi` stubs for the Python standard library and popular third-party packages. All major type checkers bundle the stdlib portion. Third-party stubs are distributed via PyPI as `types-<package>` packages (e.g., `types-requests`, `types-PyYAML`).

---

## Strategy: Four Layers

### Layer 1: Immediate Relief -- Suppression and Configuration ✅ IMPLEMENTED

**Status**: Core suppression system is implemented and working. pyproject.toml config loading is not yet implemented.

**Goal**: Make Basilisk usable on real projects today, without any stub infrastructure.

#### 1.1 Four-Mode Severity System

Every rule has four modes: `error`, `warning`, `info`, `disabled`. These can be set at every scope level, giving users total control without losing strictness as the default.

#### 1.2 Inline Suppression and Mode Override

Basilisk uses the `# type:` prefix for maximum compatibility with existing tooling:

```python
# Standard compatibility (suppresses everything on the line):
from fastmcp import FastMCP  # type: ignore

# Code-specific suppression:
from fastmcp import FastMCP  # type: ignore[BSK-E0010]

# Severity demotion (not suppression -- still shows, just not an error):
from fastmcp import FastMCP  # type: warning[BSK-E0010]
from fastmcp import FastMCP  # type: info[BSK-E0010]

# Disable the rule entirely on this line:
from fastmcp import FastMCP  # type: disabled[BSK-E0010]
```

#### 1.3 Block-Level Suppression

For files with many third-party imports, block directives avoid repeating per-line comments:

```python
# type: disabled[BSK-E0010]
from fastmcp import FastMCP
from result import Result, Ok, Err
from errors import AutomatorError, ErrorCode
from models import Platform, Credentials
# type: end-disabled[BSK-E0010]
```

#### 1.4 Per-File Mode

```python
# basilisk: relaxed
# All errors become warnings in this file

# basilisk: file-disabled[BSK-E0010]
# Disable E0010 for the entire file
```

#### 1.5 Per-Module and Per-Path Configuration in pyproject.toml

```toml
[tool.basilisk]
stub-paths = ["stubs/"]

# Global rule severity overrides
[tool.basilisk.rules]
"BSK-E0010" = "warning"    # demote globally

# Per-module overrides (for third-party imports)
[tool.basilisk.per-module-overrides."fastmcp"]
ignore-missing-stubs = true

[tool.basilisk.per-module-overrides."django.*"]
ignore-missing-stubs = true

# Per-path overrides
[tool.basilisk.per-path-overrides."vendor/**"]
rules.disabled = ["BSK-E0010"]
rules.warning = ["BSK-E0001", "BSK-E0002"]
```

#### 1.6 LSP Code Action Upgrades

Update `crates/basilisk-lsp/src/code_actions.rs` to offer for every diagnostic:
- "Ignore `BSK-E0010` on this line" -- inserts `# type: ignore[BSK-E0010]`
- "Demote `BSK-E0010` to warning on this line" -- inserts `# type: warning[BSK-E0010]`
- "Disable `BSK-E0010` for this file" -- inserts `# basilisk: file-disabled[BSK-E0010]`
- "Disable `BSK-E0010` in project config" -- opens/edits pyproject.toml

#### 1.7 Suppression Precedence

Most specific wins: line > block > file > per-path > per-module > global rule > rule default.

#### 1.8 Implementation Status

| File | Change | Status |
|------|--------|--------|
| `crates/basilisk-checker/src/diagnostic.rs` | Added `Severity::Info` and `RuleMode` enum | ✅ Done |
| `crates/basilisk-checker/src/suppression.rs` | New centralized suppression parser with all modes | ✅ Done |
| `crates/basilisk-checker/src/lib.rs` | Replaced `line_has_type_ignore` with full suppression system | ✅ Done |
| `crates/basilisk-checker/src/rules/e0010.rs` | Removed duplicate suppression logic | ✅ Done |
| `crates/basilisk-lsp/src/server.rs` | Maps `Info` severity to `DiagnosticSeverity::INFORMATION` | ✅ Done |
| `crates/basilisk-lsp/src/code_actions.rs` | Code actions: ignore, demote to warning, disable for file | ✅ Done |
| `crates/basilisk-cli/src/output.rs` | Added `Info` severity to JSON output | ✅ Done |
| New: `crates/basilisk-config/` | Config parsing crate for pyproject.toml | ❌ Not started |

**What works now:**
- `# type: ignore`, `# type: ignore[CODE]` -- per-line suppression
- `# type: warning[CODE]`, `# type: info[CODE]`, `# type: disabled[CODE]` -- per-line severity override
- Block directives: `# type: disabled[CODE]` ... `# type: end-disabled[CODE]`
- File directives: `# basilisk: relaxed`, `# basilisk: file-disabled[CODE]`, `# basilisk: file-warning[CODE]`
- LSP code actions for every diagnostic: ignore, demote to warning, disable for file
- Suppression precedence: line > block > file
- SPEC.md updated with sections 4.1.3-4.1.6 covering the full system

**What's missing from Layer 1:**
- pyproject.toml config loading (per-module overrides, per-path overrides, global rule severity)
- "Disable in project config" code action (needs config crate first)
- Per-path and per-module override precedence levels

---

### Layer 2: Stub Discovery -- PEP 561 and typeshed ❌ NOT STARTED

**Status**: Design only. `basilisk-stubs` crate is still a placeholder returning `None` for all lookups.

**Goal**: Automatically find and load type information for packages that provide it.

#### 2.1 Resolution Order

Following PEP 561, matching Pyright's behavior:

1. **User stubs** -- `.pyi` files in `stub-paths` directories
2. **User source** -- `.py` files in the project (already handled)
3. **Stub-only packages** -- installed `foopkg-stubs` packages
4. **Inline-typed packages** -- installed packages with `py.typed` marker
5. **Bundled typeshed** -- compiled into binary from `basilisk-stubs`
6. **No stubs found** -- type resolves to `Unknown`, BSK-E0010 fires

#### 2.2 Stub Discovery Engine

New module in `basilisk-stubs`:

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

Discovery process:
1. Scan `stub-paths` from config for `.pyi` files matching the import
2. Scan Python environment's `sys.path` for `-stubs` packages (look for `{module}-stubs/` with `__init__.pyi`)
3. Scan `sys.path` for packages with `py.typed` marker
4. Query the bundled typeshed index

#### 2.3 typeshed Bundling

Replace the hardcoded `STDLIB_ROOTS` in `e0010.rs` with a compiled typeshed index:

- `build.rs` in `basilisk-stubs` reads typeshed `.pyi` files at compile time
- Produces a `phf` hash map or serialized lookup table
- `lookup_builtin()` queries this index instead of returning `None`
- The stdlib whitelist becomes derived data, not a maintained list

#### 2.4 Resolver Integration

The resolver (`basilisk-resolver`) becomes stub-aware:

- Add `basilisk-stubs` as a dependency (as SPEC already specifies)
- `ImportInfo` in `crates/basilisk-resolver/src/scope.rs` gains: `resolution: Option<StubResolution>`
- The visitor queries the stub engine during import collection
- E0010 checks `import.resolution` instead of the hardcoded `STDLIB_ROOTS`

#### 2.5 .pyi File Parsing

Since Basilisk uses `ruff_python_parser`, the same parser handles `.pyi` files. Differences:
- Only signatures matter (function defs, class defs, variable annotations)
- Bodies are `...` or `pass` -- ignored
- `@overload` decorator is significant
- No runtime code analysis needed

#### 2.6 Files to Change

| File | Change |
|------|--------|
| `crates/basilisk-stubs/src/lib.rs` | Replace placeholder with stub discovery engine |
| `crates/basilisk-stubs/Cargo.toml` | Add deps: `walkdir`, `phf` |
| New: `crates/basilisk-stubs/build.rs` | typeshed bundling at compile time |
| `crates/basilisk-resolver/Cargo.toml` | Add `basilisk-stubs` dependency |
| `crates/basilisk-resolver/src/scope.rs` | Add `resolution` to `ImportInfo` |
| `crates/basilisk-resolver/src/visitor.rs` | Query stub engine during import resolution |
| `crates/basilisk-checker/src/rules/e0010.rs` | Replace `STDLIB_ROOTS` with stub resolution check |

---

### Layer 3: Type Provenance Tracking ❌ NOT STARTED

**Status**: Design only. No `TypeProvenance` or `TrackedType` structs exist yet.

**Goal**: Track where type information came from and use that to control diagnostic behavior. This is Basilisk's key differentiator.

#### 3.1 The Problem with Plain Unknown

```python
import requests   # has types-requests stubs (Tier 1)
import fastmcp    # no stubs at all

r = requests.get("https://example.com")  # InferredType: Response (from stub)
m = fastmcp.something()                  # InferredType: Unknown
```

With plain `Unknown`, every downstream use of `m` either:
- **Silently passes** (permissive -- defeats Basilisk's purpose)
- **Fires a cascade of errors** (strict -- unusable noise: 1 bad import = 50 errors)

Neither is acceptable.

#### 3.2 Solution: Types Carry Trust Metadata

Add provenance to the type system in `crates/basilisk-checker/src/types.rs`:

```rust
pub enum TypeProvenance {
    /// Type from source code annotations or inference
    Source,
    /// Type from a Tier 1 stub (typeshed, hand-written)
    StubTier1,
    /// Type from a Tier 2 stub (auto-generated, reviewed)
    StubTier2,
    /// Type from a Tier 3 stub (best-effort inference)
    StubTier3,
    /// No type information available
    Untyped,
}

pub struct TrackedType {
    pub ty: InferredType,
    pub provenance: TypeProvenance,
}
```

#### 3.3 Diagnostic Behavior by Provenance

| Provenance | BSK-E0010 | Downstream type errors | LSP hover |
|------------|-----------|----------------------|-----------|
| Source | not fired | normal errors | shows inferred type |
| StubTier1 | not fired | normal errors | shows stub type |
| StubTier2 | not fired | normal errors | shows type + "(auto-generated stub)" |
| StubTier3 | downgraded to info | warnings only | shows type + "(best-effort, may be inaccurate)" |
| Untyped | error (default) | **suppressed** | shows "Unknown (no stubs)" |

**The key insight**: one diagnostic at the import site is worth more than fifty cascading errors at use sites. When provenance is `Untyped`:
1. BSK-E0010 fires once at the import
2. The imported symbol becomes `Unknown` with `Untyped` provenance
3. Downstream rules check provenance before emitting -- if one operand is `Untyped`, suppress the cascade
4. The developer fixes the root cause (add stubs, suppress, or configure) rather than fighting noise

#### 3.4 Cascade Suppression

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

Rules that should respect provenance (argument mismatch, return mismatch, attribute access, etc.) tag their diagnostics with the provenance of the types involved. The central filter suppresses cascades.

#### 3.5 LSP Integration

- Hover over an untyped import shows: `fastmcp (no type stubs available)`
- Hover over a symbol from a Tier 3 stub shows: `FastMCP (best-effort stub, may be inaccurate)`
- Hover over a typeshed symbol shows: `os.path.join (typeshed)`

#### 3.6 Files to Change

| File | Change |
|------|--------|
| `crates/basilisk-checker/src/types.rs` | Add `TypeProvenance` and `TrackedType` |
| `crates/basilisk-checker/src/inference.rs` | Propagate provenance through inference |
| `crates/basilisk-checker/src/lib.rs` | Add cascade suppression filter |
| `crates/basilisk-checker/src/rules/e0012.rs` | Tag diagnostics with provenance |
| `crates/basilisk-checker/src/rules/e0013.rs` | Tag diagnostics with provenance |
| `crates/basilisk-lsp/src/server.rs` | Show provenance in hover tooltips |

---

### Layer 4: Auto-Stub Generation (Phase 5) ❌ NOT STARTED

**Status**: Design only.

**Goal**: For packages without any stubs, generate best-effort Tier 3 stubs automatically.

#### 4.1 Three Modes

1. **Runtime introspection** -- import the package in a subprocess, inspect `__annotations__`, `inspect.signature()`, emit `.pyi`
2. **AST-based inference** -- parse package source with `ruff_python_parser`, infer from defaults, docstrings, return statements
3. **Hybrid** -- prefer runtime data, fall back to AST

#### 4.2 CLI Commands

```bash
basilisk stubs generate requests      # generate stubs for one package
basilisk stubs generate --all         # generate for all untyped imports
basilisk stubs status                 # show stub coverage report
```

Generated stubs go into `.basilisk/stubs/` cache directory, tagged as Tier 3. The provenance system ensures these produce warnings, not false confidence.

#### 4.3 Files to Create

| File | Purpose |
|------|---------|
| `crates/basilisk-stubs/src/generate/runtime.rs` | Runtime introspection mode |
| `crates/basilisk-stubs/src/generate/ast.rs` | AST-based inference |
| `crates/basilisk-stubs/src/generate/hybrid.rs` | Combined mode |
| `crates/basilisk-stubs/src/cache.rs` | `.basilisk/stubs/` cache management |

---

## Why Not Result<T,E> Wrapper Stubs?

The initial idea: wrap every return type from untyped modules in `Result<T, UntypedOrigin>` so the type system forces you to handle the uncertainty.

This is conceptually elegant but practically infeasible:

1. **Breaks the Python type model.** A function returning `Response` should not become `Result[Response, UntypedOrigin]`. Every call site would need unwrapping.

2. **Cannot compose.** `requests.get().json()` becomes `Result[Result[dict, UntypedOrigin], UntypedOrigin]`. Nesting is unavoidable and unbounded.

3. **Conflates two dimensions.** The *type* of a value (what operations it supports) and the *confidence* in that type (where the info came from) are orthogonal concerns. Encoding confidence in the type forces every consumer to destructure it.

4. **Provenance tracking achieves the same goal without the cost.** By carrying `TypeProvenance` alongside `InferredType`, we get:
   - One error at the import site (not N at use sites)
   - Cascade suppression for downstream operations
   - Trust metadata in LSP hover
   - Configurable behavior per tier
   - No changes to how developers write Python

---

## Implementation Roadmap

| Phase | Layer | Deliverable | Effort | Dependencies | Status |
|-------|-------|-------------|--------|--------------|--------|
| **Now** | 1a | Per-code suppression (`# type: ignore[CODE]`, warning, info, disabled) | Small | None | ✅ Done |
| **Now** | 1b | Per-file markers (`# basilisk: relaxed`, `file-disabled[CODE]`) | Small | None | ✅ Done |
| **Now** | 1c | Block directives (`# type: disabled[CODE]` ... `# type: end-disabled[CODE]`) | Small | None | ✅ Done |
| **Now** | 1d | LSP code actions (ignore, demote, disable for file) | Small | Layer 1a | ✅ Done |
| **Next** | 1e | pyproject.toml config parsing + per-module overrides | Medium | New `basilisk-config` crate | ❌ Not started |
| **Next** | 2a | typeshed bundling (replace hardcoded `STDLIB_ROOTS`) | Medium | None | ❌ Not started |
| **Next** | 2b | User `stub-paths` resolution | Medium | Layer 1e (config) | ❌ Not started |
| **Soon** | 2c | PEP 561 discovery (`-stubs` packages, `py.typed`) | Medium | Layer 2b | ❌ Not started |
| **Soon** | 2d | `.pyi` parsing and type extraction | Large | Layer 2c | ❌ Not started |
| **Later** | 3 | `TypeProvenance` tracking + cascade suppression | Large | Layer 2d | ❌ Not started |
| **Phase 5** | 4 | Auto-stub generation engine | Large | Layer 3 | ❌ Not started |

Layer 1 (suppression) is mostly complete -- inline, block, and file-level directives all work with LSP code actions. The next priority is Layer 1e (pyproject.toml config) which is foundational infrastructure needed by stub discovery. Layer 2a eliminates the technical debt of the hardcoded stdlib list. Layer 3 is the big differentiator.

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| typeshed is ~40MB of `.pyi` files; bundling bloats binary | Compress with `include_bytes!`, or lazy-load sidecar. Bundle stdlib only initially. |
| PEP 561 discovery needs Python env's `sys.path` | Require `python-path` or `venv-path` in config. Fall back to `python3 -c "import sys; print(sys.path)"`. |
| Provenance threading touches many rules | Introduce as optional field on `ImportInfo` first. Cascade suppression is a central filter in `check()`, not per-rule logic. |
| Auto-generated stubs may be wrong | Tier system + provenance = auto-stubs produce warnings, never silent false positives. |
| Config crate is a new dependency for every crate | Keep lightweight (`serde` + `toml` only). Could be part of `basilisk-db`. |

---

## Migration Path for Existing Users

A project adopting Basilisk with third-party imports:

1. **Today (Layer 1)**: Add `# basilisk: ignore[BSK-E0010]` to imports, or `# basilisk: relaxed` at file top, or `per-module-overrides` in pyproject.toml
2. **After Layer 2**: Install `-stubs` packages (`pip install types-requests`). Basilisk auto-discovers them. E0010 disappears.
3. **After Layer 3**: Packages without stubs show one import-site error instead of cascading noise. LSP shows provenance in hover.
4. **After Layer 4**: Run `basilisk stubs generate --all` to create best-effort stubs for remaining packages.
