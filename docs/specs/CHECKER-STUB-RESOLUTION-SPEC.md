# Stub Resolution & Type Provenance — Specification {#STUBRES}

> **Crate**: `basilisk-stubs` (resolution, typeshed bundling), `basilisk-config` (overrides)
> **Related**: [LSP-UV-INTEGRATION-SPEC.md §LSPUV-LOCK-REGISTRY](LSP-UV-INTEGRATION-SPEC.md#LSPUV-LOCK-REGISTRY) — `PackageRegistry` accelerates stub discovery

---

## Import Resolution Order {#STUBRES-PEP561}

Basilisk MUST resolve modules that carry type information in the exact order
mandated by the Python typing specification —
[Distributing type information → Import resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)
(the normative successor to [PEP 561](https://peps.python.org/pep-0561/)). The
specification states that a type checker "SHOULD resolve modules containing type
information" in this order (quoted verbatim):

> 1. Stubs or Python source manually put in the beginning of the path. Type
>    checkers SHOULD provide this to allow the user complete control of which
>    stubs to use, and to patch broken stubs or inline types from packages.
> 2. User code - the files the type checker is running on.
> 3. Typeshed stubs for the standard library. These will usually be vendored by
>    type checkers, but type checkers SHOULD provide an option for users to
>    provide a path to a directory containing a custom or modified version of
>    typeshed; if this option is provided, type checkers SHOULD use this as the
>    canonical source for standard-library types in this step.
> 4. Stub packages - these packages SHOULD supersede any installed inline
>    package. They can be found in directories named `foopkg-stubs` for package
>    `foopkg`.
> 5. Packages with a `py.typed` marker file - if there is nothing overriding the
>    installed package, _and_ it opts into type checking, the types bundled with
>    the package SHOULD be used (be they in `.pyi` type stub files or inline in
>    `.py` files).
> 6. If the type checker chooses to additionally vendor any third-party stubs
>    (from typeshed or elsewhere), these SHOULD come last in the module
>    resolution order.

Each numbered step maps onto a Basilisk mechanism:

| Spec step | Basilisk mechanism | Config key |
|---|---|---|
| 1 — manual stubs at head of path | User `.pyi` stubs in `stub-paths` directories, plus the auto-discovered `.basilisk/stubs/` cache ([§STUBRES-CREATE-LOCAL](#STUBRES-CREATE-LOCAL)). They sit at the head of the path and MAY shadow any later module, stdlib or third-party. | `stub-paths` |
| 2 — user code | Workspace `.py` source under the configured roots / `include`. | roots, `include` |
| 3 — stdlib typeshed | The typeshed standard-library stubs bundled into `basilisk-stubs` ([§STUBRES-TYPESHED](#STUBRES-TYPESHED)), **overridable** by a custom typeshed directory ([§STUBRES-CUSTOM-TYPESHED](#STUBRES-CUSTOM-TYPESHED)). | `typeshed-path` |
| 4 — stub-only packages | Installed `foopkg-stubs` / typeshed `types-foopkg` distributions, discovered in site-packages. They supersede an inline-typed install of the same package. | (auto) |
| 5 — `py.typed` packages | Installed packages shipping a `py.typed` marker (stubs in `.pyi` or inline in `.py`). | (auto) |
| 6 — vendored third-party stubs | Basilisk vendors **no** third-party stubs for resolution; the bundled typeshed *distribution index* drives only the "install stubs" quick fix ([§STUBRES-CODEACTIONS](#STUBRES-CODEACTIONS)), never module resolution — nothing occupies this last slot. | — |

A module that matches no step resolves to `Unknown` and `imports_unresolved`
fires ([§STUBRES-PROVENANCE-DIAG](#STUBRES-PROVENANCE-DIAG)).

> **uv fast path**: In uv projects, steps 4–5 are accelerated by the `PackageRegistry` parsed from `uv.lock`. The registry knows every installed package and whether a companion stub package exists — no site-packages directory walk needed. See [LSP-UV-INTEGRATION-SPEC.md §LSPUV-LOCK-REGISTRY](LSP-UV-INTEGRATION-SPEC.md#LSPUV-LOCK-REGISTRY).

### Custom typeshed override {#STUBRES-CUSTOM-TYPESHED}

Step 3 of the resolution order requires that "type checkers SHOULD provide an
option for users to provide a path to a directory containing a custom or
modified version of typeshed; if this option is provided, type checkers SHOULD
use this as the canonical source for standard-library types in this step"
([typing spec, import resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)).

Basilisk satisfies this with `typeshed-path` (`pyproject.toml`; LSP JSON:
`typeshedPath`) — a single path to the root of a typeshed-layout directory that
supplies standard-library stubs:

```toml
[tool.basilisk]
typeshed-path = "typeshed-micropython"   # stdlib/*.pyi replacing the bundled typeshed
```

Normative behaviour:

- When `typeshed-path` is set, that directory is the **canonical source for
  standard-library types** (spec step 3): Basilisk MUST resolve stdlib modules
  against it and MUST NOT consult the bundled typeshed for any stdlib module the
  custom directory supplies.
- A stdlib module absent from the custom directory falls through to the later
  resolution steps exactly as an absent bundled stub would — the override
  *replaces* the bundled stdlib typeshed, it does not truncate resolution.
- A relative `typeshed-path` is resolved against the workspace root, mirroring
  `stub-paths`.
- `typeshed-path` is distinct from `stub-paths`: `stub-paths` (step 1)
  *prepends* extra stub directories at the head of the path and can shadow
  individual modules; `typeshed-path` (step 3) *replaces the vendored stdlib
  typeshed wholesale* as the canonical standard-library source.

This is the mechanism embedded- and alternative-Python toolchains use to teach
Basilisk a dialect's standard library — e.g. MicroPython's
[`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs),
whose `os`, `time`, and `machine` signatures diverge from CPython typeshed
([issue #271](https://github.com/Nimblesite/Basilisk/issues/271)).

---

## Stub Discovery Engine {#STUBRES-ENGINE}

`basilisk-stubs` provides stub resolution:

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

`ruff_python_parser` handles `.pyi` files too:

- Only signatures matter (function defs, class defs, variable annotations)
- Bodies (`...`/`pass`) ignored; no runtime analysis
- `@overload` is significant

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

When provenance is `Untyped`, one diagnostic at the import site replaces cascading use-site errors:

1. imports_unresolved fires once at the import
2. The imported symbol becomes `Unknown` with `Untyped` provenance
3. Downstream rules check provenance — if one operand is `Untyped`, the cascade is suppressed
4. The fix is one click: the LSP provides code actions that run the appropriate `uv` command

### Code Actions for Unresolved Imports {#STUBRES-CODEACTIONS}

**Principle**: Diagnostics MUST NOT tell users to run CLI commands; the LSP provides one-click code actions that do the work. Every imports_unresolved and BSK-E0152 diagnostic MUST have an associated code action:

| Diagnostic | Scenario | Code Action | LSP Command |
|------------|----------|-------------|-------------|
| imports_unresolved | Package not installed | "Add dependency: `{pkg}`" | `basilisk.uv.add` |
| imports_unresolved | Package not in deps (transitive only) | "Add dependency: `{pkg}`" | `basilisk.uv.add` |
| imports_unresolved | Package declared but not synced | "Sync environment" | `basilisk.uv.sync` |
| BSK-E0152 | Package installed, typeshed stub exists | "Install type stubs: `types-{pkg}`" | `basilisk.uv.addDev` |
| BSK-E0152 | Package installed, **no** typeshed stub | "Create local type stub for `{pkg}`" | `basilisk.stubs.createLocal` |

The `uv`-backed actions execute via `workspace/executeCommand`: the LSP spawns `uv` as a subprocess, reports progress via `window/logMessage`, and re-resolves on completion — the diagnostic clears automatically.

The create-local action is offered for **every** BSK-E0152 (the only fix when typeshed publishes nothing, a fallback when it does), so the "every diagnostic has a code action" guarantee holds even for packages with no published stubs.

Diagnostic help text describes **what's wrong**, not a CLI command; the action is the fix. See [LSP-UV-INTEGRATION-SPEC.md §LSPUV-ACTIONS](LSP-UV-INTEGRATION-SPEC.md#LSPUV-ACTIONS).

#### Create Local Stub {#STUBRES-CREATE-LOCAL}

`basilisk.stubs.createLocal` (arg: module name) scaffolds a **strict** local
stub for an untyped package.

- **Target**: `<workspace-root>/.basilisk/stubs/{module}.pyi`, the same Tier-3
  stub cache the resolver auto-includes on its search path (see
  [§STUBRES-AUTOGEN](#STUBRES-AUTOGEN) and `ImportSearchPaths::from_config`), so
  the import re-resolves with **no config edit**.
- **Skeleton (strict by default)**: header comments only, declaring *nothing*.
  Creating it clears `BSK-E0152`; because the stub is authoritative
  ([§STUBRES-MEMBER via E0154](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STUB-MEMBER)),
  `imports_module_attribute` then prompts the developer to declare each name used.
  The comment documents the opt-out (a module-level
  `def __getattr__(name: str) -> Any: ...` makes every attribute `Any`), but the
  skeleton deliberately does **not** emit it.
- **Idempotent**: an existing stub is never clobbered (handler returns
  `created: false`).
- After writing, the handler calls `rebuild_registry_and_resolve` so BSK-E0152
  clears automatically.

The BSK-E0152 `help`/`note` text names this `stub-paths`/`.pyi` route and links
[PEP 561](https://peps.python.org/pep-0561/) and the
[stub-writing guide](https://typing.python.org/en/latest/guides/writing_stubs.html),
folded onto the LSP diagnostic message (the LSP `Diagnostic` has no `help`/`note`
fields). No shell command appears in the help.

#### Add Member {#STUBRES-ADD-MEMBER}

`basilisk.stubs.addMember` (args: stub path, snippet line) is the quick fix for
`imports_module_attribute` ([§CHKARCH-DIAG-STUB-MEMBER](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STUB-MEMBER)) —
"add the undeclared member to the local stub", closing the loop opened by the
strict create-local skeleton.

- **Code action** (`code_actions/stubs.rs`): parses module/attribute and stub
  path from the folded diagnostic message, then inspects the access site. A call
  `module.attr(a, kw=b)` → a method with parameters inferred from the call
  (positional → `argN: Any`, keyword → `kw: Any`, `*`/`**` splat →
  `*args: Any, **kwargs: Any`); a plain `module.attr` → an attribute `attr: Any`.
- **Handler** (`server/stub_handlers.rs::execute_add_stub_member`): appends the
  snippet to the existing `.pyi`, inserting `from typing import Any` once if
  needed, then re-resolves so `imports_module_attribute` clears. Only an existing
  `.pyi` inside a workspace root is ever written.
- The developer then tightens the `Any` placeholders into real signatures.

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

Four-mode severity per rule (`error`, `warning`, `info`, `disabled`), configurable at every scope:

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
| `basilisk.stubPaths` | `string[]` | `[]` | Additional directories to search for `.pyi` stubs (resolution step 1 — [§STUBRES-PEP561](#STUBRES-PEP561)) |
| `basilisk.typeshedPath` | `string` | _(bundled)_ | Path to a custom/modified typeshed directory that becomes the canonical source for standard-library types (resolution step 3 — [§STUBRES-CUSTOM-TYPESHED](#STUBRES-CUSTOM-TYPESHED)) |

Both keys accept the `pyproject.toml` kebab-case (`stub-paths`, `typeshed-path`)
and the LSP JSON camelCase (`stubPaths`, `typeshedPath`) spellings.

`pyproject.toml` configuration:

```toml
[tool.basilisk]
stub-paths = ["stubs/"]
typeshed-path = "typeshed-micropython"   # optional: override the bundled stdlib typeshed

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

Generated stubs go into `.basilisk/stubs/`, tagged Tier 3 so provenance makes them produce warnings, not false confidence.

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
