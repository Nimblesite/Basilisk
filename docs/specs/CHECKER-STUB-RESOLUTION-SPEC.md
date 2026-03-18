# Stub Resolution & Type Provenance — Specification

> **Crate**: `basilisk-stubs` (resolution, typeshed bundling), `basilisk-config` (overrides)
> **Related**: [LSP-UV-INTEGRATION-SPEC.md](LSP-UV-INTEGRATION-SPEC.md) — `PackageRegistry` accelerates stub discovery

---

## 1. PEP 561 Resolution Order

Following [PEP 561](https://peps.python.org/pep-0561/), matching Pyright's behaviour:

1. **User stubs** — `.pyi` files in `stub-paths` config directories
2. **User source** — `.py` files in the project
3. **Stub-only packages** — installed `foopkg-stubs` packages (e.g. `types-requests`)
4. **Inline-typed packages** — installed packages with `py.typed` marker
5. **Bundled typeshed** — stdlib stubs compiled into the binary from `basilisk-stubs`
6. **No stubs found** — type resolves to `Unknown`, BSK-E0010 fires

> **uv fast path**: In uv projects, steps 3–4 are accelerated by the `PackageRegistry` parsed from `uv.lock`. The registry knows every installed package and whether a companion stub package exists — no site-packages directory walk needed. See `LSP-UV-INTEGRATION-SPEC.md` section 3.

---

## 2. Stub Discovery Engine

The `basilisk-stubs` crate provides stub resolution:

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

### typeshed Bundling

- `build.rs` in `basilisk-stubs` reads typeshed `.pyi` files at compile time
- Produces a `phf` hash map for O(1) module lookup
- `lookup_builtin()` queries this index
- The stdlib whitelist becomes derived data, not a maintained list

### `.pyi` File Parsing

Since Basilisk uses `ruff_python_parser`, the same parser handles `.pyi` files:

- Only signatures matter (function defs, class defs, variable annotations)
- Bodies are `...` or `pass` — ignored
- `@overload` decorator is significant
- No runtime code analysis needed

---

## 3. Type Provenance

Types carry metadata about where their type information came from:

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

### Diagnostic Behaviour by Provenance

| Provenance | BSK-E0010 | Downstream type errors | LSP hover |
|------------|-----------|----------------------|-----------|
| Source | not fired | normal errors | shows inferred type |
| StubTier1 | not fired | normal errors | shows stub type |
| StubTier2 | not fired | normal errors | shows type + "(auto-generated stub)" |
| StubTier3 | downgraded to info | warnings only | shows type + "(best-effort, may be inaccurate)" |
| Untyped | error (default) | **suppressed** | shows "Unknown (no stubs)" |

One diagnostic at the import site is worth more than fifty cascading errors at use sites. When provenance is `Untyped`:

1. BSK-E0010 fires once at the import
2. The imported symbol becomes `Unknown` with `Untyped` provenance
3. Downstream rules check provenance — if one operand is `Untyped`, the cascade is suppressed
4. The developer fixes the root cause (add stubs, suppress, or configure) rather than fighting noise

### Provenance in Hover

| Cursor on | Hover display |
|-----------|---------------|
| Untyped import | `fastmcp (no type stubs available)` |
| Tier 3 stub symbol | `FastMCP (best-effort stub, may be inaccurate)` |
| typeshed symbol | `os.path.join (typeshed)` |
| Tier 1 stub symbol | `requests.get(...) -> Response` (no annotation — trusted) |

> **uv enrichment** (future): In uv projects, import hovers additionally show package version, direct/transitive classification, and stub package status from the `PackageRegistry`. See `LSP-UV-INTEGRATION-SPEC.md` section 8.

---

## 4. Suppression System

Four-mode severity for every rule: `error`, `warning`, `info`, `disabled`. Configurable at every scope:

```python
# Per-line suppression:
from fastmcp import FastMCP  # type: ignore[BSK-E0010]

# Per-line severity demotion:
from fastmcp import FastMCP  # type: warning[BSK-E0010]

# Block suppression:
# type: disabled[BSK-E0010]
from fastmcp import FastMCP
from result import Result, Ok, Err
# type: end-disabled[BSK-E0010]

# Per-file:
# basilisk: file-disabled[BSK-E0010]

# Per-file relaxed mode (all errors become warnings):
# basilisk: relaxed
```

**Precedence** (most specific wins): line > block > file > per-path > per-module > global rule > rule default.

---

## 5. Configuration

| Setting Key | Type | Default | Description |
|------------|------|---------|-------------|
| `basilisk.stubPaths` | `string[]` | `[]` | Additional directories to search for `.pyi` stubs |

`pyproject.toml` configuration:

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

---

## 6. Auto-Stub Generation (CLI)

```bash
basilisk stubs generate requests      # generate stubs for one package
basilisk stubs generate --all         # generate for all untyped imports
basilisk stubs status                 # show stub coverage report
```

Generated stubs go into `.basilisk/stubs/`, tagged as Tier 3. The provenance system ensures these produce warnings, not false confidence.

### Generation Modes

| Mode | Source | Accuracy |
|------|--------|----------|
| Runtime introspection | `inspect.signature()` via subprocess | Highest — sees actual signatures |
| AST-based inference | Parse `.py` source, infer types | Medium — misses dynamic patterns |
| Hybrid | Prefer runtime, fall back to AST | Best of both |

---

## 7. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| typeshed ~40MB bloats binary | Compress with `include_bytes!`, bundle stdlib only initially |
| PEP 561 discovery needs `sys.path` | Require `python-path` or `venv-path` in config; fall back to `python3 -c "import sys; print(sys.path)"`. In uv projects, `uv.lock` + `.python-version` eliminate the subprocess |
| Auto-generated stubs may be wrong | Tier system + provenance = auto-stubs produce warnings, never silent false positives |
