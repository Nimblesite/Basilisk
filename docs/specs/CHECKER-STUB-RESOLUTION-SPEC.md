# Stub Resolution & Type Provenance — Specification {#STUBRES}

> **Crate**: `basilisk-stubs` (resolution, typeshed bundling), `basilisk-config` (overrides)
> **Related**: [LSP-UV-INTEGRATION-SPEC.md §LSPUV-LOCK-REGISTRY](LSP-UV-INTEGRATION-SPEC.md#LSPUV-LOCK-REGISTRY) — `PackageRegistry` accelerates stub discovery

---

## PEP 561 Resolution Order {#STUBRES-PEP561}

Following [PEP 561](https://peps.python.org/pep-0561/), the resolution order is:

1. **User stubs** — `.pyi` files in `stub-paths` config directories
2. **User source** — `.py` files in the project
3. **Stub-only packages** — installed `foopkg-stubs` packages (e.g. `types-requests`)
4. **Inline-typed packages** — installed packages with `py.typed` marker
5. **Bundled typeshed** — stdlib stubs compiled into the binary from `basilisk-stubs`
6. **No stubs found** — type resolves to `Unknown`, imports_unresolved fires

> **uv fast path**: In uv projects, steps 3–4 are accelerated by the `PackageRegistry` parsed from `uv.lock`. The registry knows every installed package and whether a companion stub package exists — no site-packages directory walk needed. See [LSP-UV-INTEGRATION-SPEC.md §LSPUV-LOCK-REGISTRY](LSP-UV-INTEGRATION-SPEC.md#LSPUV-LOCK-REGISTRY).

---

## Stub Discovery Engine {#STUBRES-ENGINE}

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

### typeshed Bundling {#STUBRES-TYPESHED}

- `build.rs` in `basilisk-stubs` reads typeshed `.pyi` files at compile time
- Produces a `phf` hash map for O(1) module lookup
- `lookup_builtin()` queries this index
- The stdlib whitelist becomes derived data, not a maintained list

### .pyi File Parsing {#STUBRES-PYI}

Since Basilisk uses `ruff_python_parser`, the same parser handles `.pyi` files:

- Only signatures matter (function defs, class defs, variable annotations)
- Bodies are `...` or `pass` — ignored
- `@overload` decorator is significant
- No runtime code analysis needed

---

## Type Provenance {#STUBRES-PROVENANCE}

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

### Diagnostic Behaviour by Provenance {#STUBRES-PROVENANCE-DIAG}

| Provenance | imports_unresolved | Downstream type errors | LSP hover | Code Action |
|------------|-----------|----------------------|-----------|-------------|
| Source | not fired | normal errors | shows inferred type | — |
| StubTier1 | not fired | normal errors | shows stub type | — |
| StubTier2 | not fired | normal errors | shows type + "(auto-generated stub)" | — |
| StubTier3 | downgraded to info | warnings only | shows type + "(best-effort, may be inaccurate)" | — |
| Untyped | error (default) | **suppressed** | shows "Unknown (no stubs)" | one-click install (typeshed) or create-local stub via LSP |

One diagnostic at the import site is worth more than fifty cascading errors at use sites. When provenance is `Untyped`:

1. imports_unresolved fires once at the import
2. The imported symbol becomes `Unknown` with `Untyped` provenance
3. Downstream rules check provenance — if one operand is `Untyped`, the cascade is suppressed
4. The developer fixes it **with a single click** — the LSP provides code actions (quick fixes) that execute the appropriate `uv` command automatically

### Code Actions for Unresolved Imports {#STUBRES-CODEACTIONS}

**Principle**: Diagnostics MUST NOT tell users to run CLI commands. The LSP provides one-click code actions that do the work. The user should never leave the editor to fix a missing import.

Every imports_unresolved and BSK-E0152 diagnostic MUST have an associated code action:

| Diagnostic | Scenario | Code Action | LSP Command |
|------------|----------|-------------|-------------|
| imports_unresolved | Package not installed | "Add dependency: `{pkg}`" | `basilisk.uv.add` |
| imports_unresolved | Package not in deps (transitive only) | "Add dependency: `{pkg}`" | `basilisk.uv.add` |
| imports_unresolved | Package declared but not synced | "Sync environment" | `basilisk.uv.sync` |
| BSK-E0152 | Package installed, typeshed stub exists | "Install type stubs: `types-{pkg}`" | `basilisk.uv.addDev` |
| BSK-E0152 | Package installed, **no** typeshed stub | "Create local type stub for `{pkg}`" | `basilisk.stubs.createLocal` |

The `uv`-backed code actions execute via `workspace/executeCommand`. The LSP spawns `uv` as a subprocess, reports progress via `window/logMessage`, and triggers a full re-resolve on completion — the diagnostic clears automatically.

The create-local action is offered for **every** BSK-E0152 (it is the *only* fix when typeshed publishes nothing, and a fallback when it does), so the "every diagnostic has a code action" guarantee above holds even for packages with no published stubs — the case that previously had no fix at all.

Diagnostic help text should describe **what's wrong**, not what CLI command to run. The code action is the fix. See [LSP-UV-INTEGRATION-SPEC.md §LSPUV-ACTIONS](LSP-UV-INTEGRATION-SPEC.md#LSPUV-ACTIONS) for the full code action specification.

#### Create Local Stub {#STUBRES-CREATE-LOCAL}

`basilisk.stubs.createLocal` (arg: module name) scaffolds a **strict** local
stub for an untyped package so the developer — or an AI assisting in the
editor — has a concrete, authoritative starting point instead of a dead-end error.

- **Target**: `<workspace-root>/.basilisk/stubs/{module}.pyi`, the same Tier-3
  stub cache the resolver auto-includes on its search path (see
  [§STUBRES-AUTOGEN](#STUBRES-AUTOGEN) and `ImportSearchPaths::from_config`).
  Writing there means the import re-resolves with **no config edit**.
- **Skeleton (strict by default)**: header comments only — it declares *nothing*.
  Creating it clears `BSK-E0152` (a stub now exists), and because the stub is
  authoritative ([§STUBRES-MEMBER via E0154](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STUB-MEMBER)),
  the developer is then prompted by `imports_module_attribute` to declare each name they use.
  The comment documents the opt-out — adding a module-level
  `def __getattr__(name: str) -> Any: ...` makes every attribute `Any` and turns
  strictness off — but the skeleton deliberately does **not** emit it, so the
  stub is strict out of the box.
- **Idempotent**: an existing stub is never clobbered (the handler returns
  `created: false` and preserves the developer's work).
- After writing, the handler calls `rebuild_registry_and_resolve` so BSK-E0152
  clears automatically — matching the `uv` quick-fix behaviour above.

The BSK-E0152 `help`/`note` text names this `stub-paths`/`.pyi` route and links
[PEP 561](https://peps.python.org/pep-0561/) and the
[stub-writing guide](https://typing.python.org/en/latest/guides/writing_stubs.html);
those lines are folded onto the LSP diagnostic message so the editor surfaces
them (the LSP `Diagnostic` has no `help`/`note` fields). No shell command
appears in the help — the code action does the work.

#### Add Member {#STUBRES-ADD-MEMBER}

`basilisk.stubs.addMember` (args: stub path, snippet line) is the quick fix for
`imports_module_attribute` ([§CHKARCH-DIAG-STUB-MEMBER](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STUB-MEMBER)):
"add the undeclared member to the local stub", closing the loop opened by the
strict create-local skeleton.

- **Code action** (`code_actions/stubs.rs`): parses the module/attribute and stub
  path out of the (folded) diagnostic message, then inspects the source at the
  access site. A call `module.attr(a, kw=b)` → a method whose parameters are
  inferred from the call arguments (positional → `argN: Any`, keyword → `kw: Any`,
  a `*`/`**` splat → `*args: Any, **kwargs: Any`); a plain `module.attr` → an
  attribute `attr: Any`.
- **Handler** (`server/stub_handlers.rs::execute_add_stub_member`): appends the
  snippet to the existing `.pyi`, inserting `from typing import Any` once when the
  snippet needs it, then re-resolves so `imports_module_attribute` clears. Safety: only an
  existing `.pyi` inside a workspace root is ever written.
- The developer then tightens the `Any` placeholders into real signatures — the
  stub grows precise one member at a time.

### Provenance in Hover {#STUBRES-PROVENANCE-HOVER}

| Cursor on | Hover display |
|-----------|---------------|
| Untyped import | `fastmcp (no type stubs available)` |
| Tier 3 stub symbol | `FastMCP (best-effort stub, may be inaccurate)` |
| typeshed symbol | `os.path.join (typeshed)` |
| Tier 1 stub symbol | `requests.get(...) -> Response` (no annotation — trusted) |

> **uv enrichment** (future): In uv projects, import hovers additionally show package version, direct/transitive classification, and stub package status from the `PackageRegistry`. See [LSP-UV-INTEGRATION-SPEC.md §LSPUV-HOVER](LSP-UV-INTEGRATION-SPEC.md#LSPUV-HOVER).

---

## Suppression System {#STUBRES-SUPPRESSION}

Four-mode severity for every rule: `error`, `warning`, `info`, `disabled`. Configurable at every scope:

```python
# Per-line suppression:
from fastmcp import FastMCP  # type: ignore[imports_unresolved]

# Per-line severity demotion:
from fastmcp import FastMCP  # type: warning[imports_unresolved]

# Block suppression:
# type: disabled[imports_unresolved]
from fastmcp import FastMCP
from result import Result, Ok, Err
# type: end-disabled[imports_unresolved]

# Per-file:
# basilisk: file-disabled[imports_unresolved]

# Per-file relaxed mode (all errors become warnings):
# basilisk: relaxed
```

**Precedence** (most specific wins): line > block > file > per-path > per-module > global rule > rule default.

---

## Configuration {#STUBRES-CONFIG}

| Setting Key | Type | Default | Description |
|------------|------|---------|-------------|
| `basilisk.stubPaths` | `string[]` | `[]` | Additional directories to search for `.pyi` stubs |

`pyproject.toml` configuration:

```toml
[tool.basilisk]
stub-paths = ["stubs/"]

[tool.basilisk.rules]
"imports_unresolved" = "warning"

[tool.basilisk.per-module-overrides."fastmcp"]
ignore-missing-stubs = true

[tool.basilisk.per-module-overrides."django.*"]
ignore-missing-stubs = true

[tool.basilisk.per-path-overrides."vendor/**"]
disabled = ["imports_unresolved"]
```

---

## Auto-Stub Generation {#STUBRES-AUTOGEN}

```bash
basilisk stubs generate requests      # generate stubs for one package
basilisk stubs generate --all         # generate for all untyped imports
basilisk stubs status                 # show stub coverage report
```

Generated stubs go into `.basilisk/stubs/`, tagged as Tier 3. The provenance system ensures these produce warnings, not false confidence.

### Generation Modes {#STUBRES-AUTOGEN-MODES}

| Mode | Source | Accuracy |
|------|--------|----------|
| Runtime introspection | `inspect.signature()` via subprocess | Highest — sees actual signatures |
| AST-based inference | Parse `.py` source, infer types | Medium — misses dynamic patterns |
| Hybrid | Prefer runtime, fall back to AST | Best of both |

---

## Constraints {#STUBRES-RISKS}

| Constraint | Resolution |
|------|------------|
| Bundled typeshed stubs add binary size | Compress with `include_bytes!`, bundle stdlib only initially |
| PEP 561 discovery needs `sys.path` | Require `python-path` or `venv-path` in config; fall back to `python3 -c "import sys; print(sys.path)"`. In uv projects, `uv.lock` + `.python-version` eliminate the subprocess |
