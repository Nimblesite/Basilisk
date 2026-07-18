# Stub Resolution & Type Provenance — Specification {#STUBRES}

> **Crate**: `basilisk-stubs` (resolution, the runtime `python/typeshed` clone + on-disk cache, and the bundled offline baseline), `basilisk-config` (overrides)
> **Related**: [LSP-UV-INTEGRATION-SPEC.md §LSPUV-LOCK-REGISTRY](LSP-UV-INTEGRATION-SPEC.md#LSPUV-LOCK-REGISTRY) — `PackageRegistry` accelerates stub discovery

---

## Static Resolution Model {#STUBRES-STATIC-MODEL}

Import resolution is **purely static**. Basilisk resolves every import by
inspecting files on disk in the fixed order of [§STUBRES-PEP561](#STUBRES-PEP561),
and MUST NOT execute the target program or its import system to do so. There is
no embedded CPython, no interpreter subprocess, and no model of the runtime
`import` machinery — resolution is a filesystem search, never an execution. This
is what lets Basilisk ship as a single native binary with no Python runtime.

Two consequences follow directly, and both are **by design**, not gaps:

- **Computed / dynamic imports are unresolvable.** An import whose module name
  is not a static string literal — `importlib.import_module(name)`,
  `__import__(var)` — cannot be followed statically, because the name exists only
  at runtime. The call's result is `Any` (the declared return type of
  `importlib.import_module` / `__import__` in typeshed) and no member access on
  it is checked. Basilisk MUST NOT guess the target module.
- **`sys.meta_path` finders and custom loaders are not modelled.** A program MAY
  install import hooks (`sys.meta_path`, `sys.path_hooks`, custom
  `importlib.abc.MetaPathFinder`s) that make `import foo` resolve to something no
  filesystem search could find — generated modules, database-backed modules,
  zipimports. Honouring those hooks would mean running the target program, which
  a static checker MUST NOT do. Such a module matches no step of
  [§STUBRES-PEP561](#STUBRES-PEP561) and is treated exactly like any other
  unresolved import.

The typed-world answer for a module Basilisk cannot introspect statically is a
**stub** (`.pyi`), not a runtime model: authoring or installing a stub
([§STUBRES-PEP561](#STUBRES-PEP561) steps 1/4/5, or the "Create local stub" quick
fix [§STUBRES-CREATE-LOCAL](#STUBRES-CREATE-LOCAL)) gives an otherwise-dynamic
module precise types with zero execution.

**Terminal state.** When the static search of [§STUBRES-PEP561](#STUBRES-PEP561)
exhausts every step without a hit, the import lands in the terminal
`ImportResolution::Unresolved` state and its bound names carry an **implicit
`Any`**. Basilisk MUST NOT silently accept that `Any` the way a gradual checker
does: default-strict surfaces it as `imports_unresolved`
([§STUBRES-PROVENANCE-DIAG](#STUBRES-PROVENANCE-DIAG),
[CHKARCH-STRICTNESS-ANY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-ANY)). A
module MAY *stay* `Any` only through an explicit opt-out — a module-level
`def __getattr__(name: str) -> Any: ...` in its stub
([§STUBRES-CREATE-LOCAL](#STUBRES-CREATE-LOCAL)) — never implicitly.

---

## Import Resolution Order {#STUBRES-PEP561}

Basilisk MUST resolve modules that carry type information in the exact order
mandated by the Python typing specification —
[Distributing type information → Import resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)
(the normative successor to [PEP 561](https://peps.python.org/pep-0561/)).
The table maps that upstream order to Basilisk; the linked typing specification
is authoritative for the general rule.

| Spec step | Basilisk mechanism | Config key |
|---|---|---|
| 1 — manual stubs at head of path | User `.pyi` stubs in `stub-paths` directories, plus the auto-discovered `.basilisk/stubs/` cache ([§STUBRES-CREATE-LOCAL](#STUBRES-CREATE-LOCAL)). They sit at the head of the path and MAY shadow any later module, stdlib or third-party. | `stub-paths` |
| 2 — user code | Workspace `.py` source under the configured roots / `include`. | roots, `include` |
| 3 — stdlib typeshed | The on-disk **clone of [`python/typeshed`](https://github.com/python/typeshed)** that Basilisk acquires and refreshes at runtime ([§STUBRES-TYPESHED](#STUBRES-TYPESHED)); a small **bundled baseline** backs it until the first successful clone. Overridable by a custom typeshed directory ([§STUBRES-CUSTOM-TYPESHED](#STUBRES-CUSTOM-TYPESHED)). | `typeshed-path`, `typeshed-commit`, `typeshed-cache-path`, `typeshed-refresh-interval` |
| 4 — stub-only packages | Installed `foopkg-stubs` / typeshed `types-foopkg` distributions, discovered in site-packages. They supersede an inline-typed install of the same package. | (auto) |
| 5 — `py.typed` packages | Installed packages shipping a `py.typed` marker (stubs in `.pyi` or inline in `.py`). | (auto) |
| 6 — vendored third-party stubs | Basilisk vendors **no** third-party stubs for resolution; the typeshed *distribution map* (read from the runtime clone, or the bundled baseline while offline — [§STUBRES-TYPESHED](#STUBRES-TYPESHED)) drives only the "install stubs" quick fix ([§STUBRES-CODEACTIONS](#STUBRES-CODEACTIONS)), never module resolution — nothing occupies this last slot. | — |

A module that matches no step resolves to `Unknown` and `imports_unresolved`
fires ([§STUBRES-PROVENANCE-DIAG](#STUBRES-PROVENANCE-DIAG)).

> **uv fast path**: In uv projects, steps 4–5 are accelerated by the `PackageRegistry` parsed from `uv.lock`. The registry knows every installed package and whether a companion stub package exists — no site-packages directory walk needed. See [LSP-UV-INTEGRATION-SPEC.md §LSPUV-LOCK-REGISTRY](LSP-UV-INTEGRATION-SPEC.md#LSPUV-LOCK-REGISTRY).

### Custom typeshed override {#STUBRES-CUSTOM-TYPESHED}

Step 3 of the resolution order requires that "type checkers SHOULD provide an
option for users to provide a path to a directory containing a custom or
modified version of typeshed; if this option is provided, type checkers SHOULD
use this as the canonical source for standard-library types in this step"
([typing spec, import resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)).

Basilisk satisfies this with `typeshed-path` in the one project configuration —
`pyproject.toml` `[tool.basilisk]`
([CHKARCH-CONFIG-FILE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-FILE)); the
pyright-compat camelCase spelling `typeshedPath` is accepted in the same table
and in the pyright compatibility sources
([ANALYSIS-CONFIG-PRI](LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CONFIG-PRI)). The
value is a single path to the root of a typeshed-layout directory that
supplies standard-library stubs:

```toml
[tool.basilisk]
typeshed-path = "typeshed-micropython"   # your own stdlib/*.pyi; disables the auto-clone
```

Normative behaviour:

- When `typeshed-path` is set, that directory is the **canonical source for
  standard-library types** (spec step 3) and **disables the runtime clone
  entirely**: Basilisk MUST resolve stdlib modules against it and MUST NOT
  consult the runtime typeshed clone or the bundled baseline
  ([§STUBRES-TYPESHED](#STUBRES-TYPESHED)) for any module the custom directory
  supplies. This is also the mechanism for pointing Basilisk at an **existing
  typeshed tree already on disk** instead of letting it clone.
- A stdlib module absent from the custom directory falls through to the later
  resolution steps exactly as an unresolved module would — the override replaces
  stdlib recognition, it does not truncate resolution.
- A relative `typeshed-path` is resolved against the workspace root, mirroring
  `stub-paths`.
- `typeshed-path` is distinct from `stub-paths`: `stub-paths` (step 1)
  *prepends* extra stub directories at the head of the path and can shadow
  individual modules; `typeshed-path` (step 3) replaces the **default runtime
  typeshed clone** with a caller-supplied standard-library stub tree.
- `typeshed-path` is also distinct from `typeshed-cache-path`
  ([§STUBRES-TYPESHED-CONFIG](#STUBRES-TYPESHED-CONFIG)): `typeshed-cache-path`
  only relocates *where the automatic clone is stored*; `typeshed-path` supplies
  your *own* tree and turns cloning off.
- The directory uses typeshed layout: stdlib stubs live under `stdlib/`, and
  Basilisk resolves `<typeshed-path>/stdlib/<module>.pyi`.

### Resolution flow {#STUBRES-RESOLUTION-FLOW}

The full order, including the canonicality rule: when `typeshed-path` is set, a
stdlib module absent from it MUST NOT be rescued by the runtime clone or the
bundled baseline — the custom directory is canonical for step 3, so an absent
module falls through to steps 4–5 and, failing those, to `imports_unresolved`.
When `typeshed-path` is unset, step 3 resolves against the **runtime typeshed
clone** if one is available, and against the **bundled baseline** name-set only
while it is not (which also raises the CLI warning —
[§STUBRES-TYPESHED-WARN](#STUBRES-TYPESHED-WARN)).

```mermaid
flowchart TB
    A["import X"] --> B{"stub-paths<br/>(step 1)?"}
    B -- hit --> Z["Resolved (UserStub)"]
    B -- miss --> C{"user code<br/>(step 2)?"}
    C -- hit --> Z2["Resolved (Source)"]
    C -- miss --> D{"typeshed-path set?"}
    D -- yes --> E{"&lt;typeshed-path&gt;/stdlib/X.pyi?"}
    E -- hit --> Z3["Resolved (CustomTypeshed)"]
    E -- miss --> G["fall through<br/>(custom typeshed is canonical)"]
    D -- no --> CL{"typeshed clone<br/>available?"}
    CL -- yes --> EY{"&lt;cache&gt;/stdlib/X.pyi?"}
    EY -- hit --> Z4["Resolved (Typeshed, clone)"]
    EY -- miss --> G2["steps 4–5"]
    CL -- no --> BB{"bundled baseline<br/>name-set? (warn)"}
    BB -- yes --> Z4b["Recognised (Typeshed, baseline)"]
    BB -- miss --> G2
    G --> G2
    G2 --> H{"site-packages<br/>(steps 4–5)?"}
    H -- hit --> Z5["Resolved (StubPackage / InlineTyped)"]
    H -- miss --> U["Unknown → imports_unresolved"]
```

---

## Stub Discovery Engine {#STUBRES-ENGINE}

`basilisk-stubs` provides stub resolution.

### Type model {#STUBRES-TYPE-MODEL}

The resolver returns a `StubResolution` tagged with **where** the type info came
from (`StubSource`) and **how much to trust it** (`StubTier`). The data model is
defined in [typeDiagram](https://typediagram.dev) markup — source of truth
[`models/stub_resolution.td`](../../models/stub_resolution.td), rendered to
[`models/stub_resolution.td`](../../models/stub_resolution.td). The Rust
ADTs in `crates/basilisk-stubs/src/types.rs` are generated from it
(`typediagram --to rust models/stub_resolution.td`):

```td
alias PathBuf = String

type StubResolution {
  module: String
  source: StubSource
  pyi_path: Option<PathBuf>
  tier: StubTier
}

union StubSource {
  UserStub
  StubPackage
  InlineTyped
  Typeshed
  CustomTypeshed
}

union StubTier {
  Tier1
  Tier2
  Tier3
}
```

`StubSource` records which resolution step ([§STUBRES-PEP561](#STUBRES-PEP561)) supplied the stub:

| Variant | Resolution step | Meaning |
|---|---|---|
| `UserStub` | 1 | `.pyi` from a `stub-paths` directory (head of path) |
| `CustomTypeshed` | 3 | stdlib stub from a `typeshed-path` override ([§STUBRES-CUSTOM-TYPESHED](#STUBRES-CUSTOM-TYPESHED)) |
| `Typeshed` | 3 | stdlib resolved from the runtime `python/typeshed` clone (real `.pyi`), or recognised by the bundled baseline name-set when no clone is available ([§STUBRES-TYPESHED](#STUBRES-TYPESHED)) |
| `StubPackage` | 4 | installed `foopkg-stubs` package |
| `InlineTyped` | 5 | installed package with a `py.typed` marker |

A `CustomTypeshed` stub is `Tier1` (hand-written, trusted) and hovers as
`… (custom typeshed)`, so a MicroPython signature is never misreported as the
built-in CPython classification.

### Standard-library typeshed: runtime clone + bundled baseline {#STUBRES-TYPESHED}

Basilisk's canonical source for standard-library types is a **real clone of
[`python/typeshed`](https://github.com/python/typeshed) on disk**, acquired and
kept current at runtime — never a compile-time index. A small bundled baseline
exists only so the checker works with no network on first run. Two cooperating
mechanisms:

1. **Runtime clone (default, canonical)** — [§STUBRES-TYPESHED-CLONE](#STUBRES-TYPESHED-CLONE).
   On startup Basilisk clones `python/typeshed` into an on-disk cache and resolves
   stdlib against its real `stdlib/*.pyi` (types, signatures, hover, `__init__`
   hints — GitHub #289) and its `stdlib/VERSIONS` (the authoritative,
   version-gated set of stdlib module names). It also reads the
   `stubs/<DIST>/` tree to build the `types-<distribution>` map.
2. **Bundled baseline (offline day-one fallback)** — [§STUBRES-TYPESHED-BASELINE](#STUBRES-TYPESHED-BASELINE).
   Basilisk ships a small, loose, replaceable baseline — the stdlib module-name
   set (typeshed `VERSIONS` format) and the `types-<distribution>` map
   (`crates/basilisk-stubs/data/typeshed_stub_distributions.tsv`). It is **not**
   authoritative; it carries names and the distribution map only — never stdlib
   `.pyi` bodies.

**Override rule (normative).** A **successful clone wholesale overrides** the
bundled baseline: once a clone is available, both the stdlib name-set and the
`types-<distribution>` map are read from the clone and the baseline is **not**
consulted. The baseline is used **only** while no clone is available — offline,
clone failed, or the first check before the clone completes — and every such run
raises the CLI warning ([§STUBRES-TYPESHED-WARN](#STUBRES-TYPESHED-WARN)).

#### Runtime clone & cache {#STUBRES-TYPESHED-CLONE}

- **Acquire on startup.** On the LSP `initialized` notification (and before the
  first CLI check) Basilisk acquires/refreshes the cache in the background. The
  first check is **gated** on a ready stdlib source (clone or baseline) so no
  `import os` ever flashes `imports_unresolved` mid-clone
  ([LSP-ANALYSIS-MODES §ANALYSIS-STARTUP](LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-STARTUP)).
- **Pinned commit → frozen.** When `typeshed-commit` is set, the cache is checked
  out at that exact SHA and **frozen**; no update check ever runs. Fully
  deterministic.
- **Unpinned → tracks `main` on a TTL.** With no `typeshed-commit`, the cache
  tracks `python/typeshed@main`; Basilisk re-checks for updates every
  `typeshed-refresh-interval` (default `24h`).
- **Determinism (normative).** Every acquire and every refresh ends with
  `git fetch` + `git clean -x -f -d` + `git reset --hard <target>`, so the tree
  is byte-for-byte identical to the upstream commit — no locally modified file
  ever survives. This is what guarantees the resolution contract is exact.
- **Failure is never fatal.** A failed clone or refresh keeps the last-good cache
  if one exists — resolving silently against that clone, reported as *cloned but
  stale* ([§STUBRES-TYPESHED-WARN](#STUBRES-TYPESHED-WARN)), **no** baseline
  warning — and only when **no** cache exists falls back to the bundled baseline
  and warns. Offline day-one always works.
- **Location.** The cache defaults to the OS cache directory; `typeshed-cache-path`
  relocates it. Setting `typeshed-path` supplies your own tree and **disables
  cloning entirely** ([§STUBRES-CUSTOM-TYPESHED](#STUBRES-CUSTOM-TYPESHED)).

#### Bundled baseline {#STUBRES-TYPESHED-BASELINE}

- **Contents:** the stdlib module-name set (typeshed `VERSIONS` format, so one
  loader serves baseline *and* clone) and the `types-<distribution>` `.tsv`.
  **No** stdlib `.pyi` bodies.
- **Form:** loose, replaceable data files shipped in the package and loaded at
  runtime by the same loader that reads the clone — **not** compiled into the
  binary. (The build MAY additionally compile a copy in **only if** `make bench`
  proves it a material speedup; the loose file stays the source of truth and the
  override target, and the benchmark ratchet
  [CHKARCH-TESTING-BENCH-RATCHET](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING-BENCH-RATCHET)
  is the guard.)
- **Purpose:** offline day-one only — no false `imports_unresolved` on stdlib,
  and BSK-0152 works. **Call-level stdlib types and hover/`__init__` hints
  (#289) require the clone**; on the baseline alone, stdlib names resolve but
  carry no `.pyi` bodies.
- **Conformance stays green (normative).** The `python/typing` conformance suite
  needs only the stdlib **names** (never `.pyi` bodies), and its pinned fixtures
  import only long-stable stdlib modules — so the baseline satisfies conformance
  with no network ([CHKARCH-CONFORMANCE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE)).

#### Data-freshness reporting & baseline warning {#STUBRES-TYPESHED-WARN}

- **Every CLI run** prints a **muted, low-prominence** one-line report of the
  typeshed data in use — dimmed, never a banner — colour-coded by freshness:
  - *Cloned & current* (pinned commit resolved, or updated within the TTL):
    **dim green** — `typeshed <short-sha> · <commit-date>`.
  - *Cloned but stale* (a present clone kept after a skipped, failed, or offline
    refresh whose last update is older than the TTL): **dim amber** —
    `typeshed <short-sha> · <commit-date> — stale (refresh failed/offline); connect to refresh`.
  - *Bundled baseline in use* (no clone has ever been acquired — offline first
    run, or a failed initial clone): **dim amber** —
    `typeshed: bundled baseline <baseline-date> — not updated; connect to refresh`.
- The bundled-baseline warning fires **only** on a run that actually resolved
  against the bundled baseline; a retained stale clone shows the *cloned but
  stale* line above, never the baseline warning. Both amber states are the honest
  signal *"your standard-library types are not current."*
- The LSP surfaces the same state in the **Service Info tree**: a spinner while
  acquiring, then the resolved cache path and freshness
  ([LSP-CONFIGURATION-EDITOR §LSPCFGED-TYPESHED](LSP-CONFIGURATION-EDITOR-SPEC.md#LSPCFGED-TYPESHED)).

#### Config keys {#STUBRES-TYPESHED-CONFIG}

| Config key (`[tool.basilisk]`) | Type | Default | Meaning |
|---|---|---|---|
| `typeshed-commit` | `string` | _(unset → track `main`)_ | Pin the clone to an exact commit SHA and **freeze** it (no TTL polling). |
| `typeshed-cache-path` | `string` | _(OS cache dir)_ | Where the automatic clone is stored. **Folder-picker** in the config UI. |
| `typeshed-refresh-interval` | `string` | `"24h"` | Update-check TTL when unpinned. |
| `typeshed-path` | `string` | _(unset → auto-clone)_ | Supply your own typeshed tree; **disables cloning**. Also the way to use an existing on-disk tree. **Folder-picker** in the config UI ([§STUBRES-CUSTOM-TYPESHED](#STUBRES-CUSTOM-TYPESHED)). |

Precedence among the path keys is unambiguous: `typeshed-path` (your tree, no
clone) wins; otherwise the clone is stored at `typeshed-cache-path` and pinned/
refreshed per `typeshed-commit` / `typeshed-refresh-interval`.

### .pyi File Parsing {#STUBRES-PYI}

`ruff_python_parser` handles `.pyi` files too:

- Only signatures matter (function defs, class defs, variable annotations)
- Bodies (`...`/`pass`) ignored; no runtime analysis
- `@overload` is significant

#### Re-exports {#STUBRES-PYI-REEXPORTS}

A stub's public interface is not just the names it defines. Per the typing
spec's [import conventions](https://typing.python.org/en/latest/spec/distributing.html#import-conventions),
it also includes names the stub re-exports (GitHub #312):

- **Redundant aliases** — `from y import x as x` and `import x as x` re-export
  `x`.
- **`__all__`** — names listed in `__all__` (list or tuple of string literals,
  including `+=` extensions) are exported.
- **Star imports** — `from .sub import *` re-exports the target stub's export
  set: its `__all__` when it defines one (authoritative, exactly like runtime
  `import *`), otherwise its public (non-underscore) top-level names and
  re-exports, followed recursively. Targets resolve relative to the stub file;
  absolute star imports are not followed (intra-package re-exports in stubs
  are conventionally relative — typeshed style).

Version/platform-gated branches (`if sys.version_info >= …:`) are **unioned**
for `__all__` and re-export imports: for attribute existence an
over-approximation can only suppress false positives, never create one. The
extractor (`crates/basilisk-stubs/src/pyi_parser.rs`) records these on
`StubModule`; `crates/basilisk-stubs/src/reexports.rs` resolves star-import
chains; and the user-stub member API consumed by `imports_module_attribute`
([CHKARCH-DIAG-STUB-MEMBER](CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-stub-member))
folds the result into `member_names`
(`crates/basilisk-checker/src/imports/apply.rs`), so a package `__init__.pyi`
built from re-exports (e.g. `asyncio` in micropython-stdlib-stubs) surfaces
`sleep`/`Task`/`run` as attributes of the package.

---

## Type Provenance {#STUBRES-PROVENANCE}

Types carry a `TypeProvenance` value from
`crates/basilisk-stubs/src/types.rs`. It records source annotations,
built-in/custom typeshed recognition, community or generated stubs, and untyped
imports. There is no separate `TrackedType` wrapper.

### Diagnostic Behaviour by Provenance {#STUBRES-PROVENANCE-DIAG}

| Provenance | imports_unresolved | Downstream type errors | LSP hover | Code Action |
|------------|-----------|----------------------|-----------|-------------|
| Source | not fired | normal errors | shows inferred type | — |
| StubTier1 | not fired | normal errors | shows stub type + "(typeshed)" | — |
| StubCustomTypeshed | not fired | normal errors | shows stub type + "(custom typeshed)" | — |
| StubTier2 | not fired | normal errors | shows type + "(community stub)" | — |
| StubTier3 | downgraded to info | warnings only | shows type + "(best-effort stub, may be inaccurate)" | — |
| Untyped | error (default) | **suppressed** | shows type + "(no type stubs available)" | one-click install (typeshed) or create-local stub via LSP |

When provenance is `Untyped`, one diagnostic at the import site replaces cascading use-site errors:

1. imports_unresolved fires once at the import
2. The imported symbol becomes `Unknown` with `Untyped` provenance
3. Downstream rules check provenance — if one operand is `Untyped`, the cascade is suppressed
4. The fix is one click: the LSP provides code actions that run the appropriate `uv` command

### Code Actions for Unresolved Imports {#STUBRES-CODEACTIONS}

**Principle**: Diagnostics MUST NOT tell users to run CLI commands; the LSP provides one-click code actions that do the work. Every imports_unresolved and BSK-0152 diagnostic MUST have an associated code action:

| Diagnostic | Scenario | Code Action | LSP Command |
|------------|----------|-------------|-------------|
| imports_unresolved | Package not installed | "Add dependency: `{pkg}`" | `basilisk.uv.add` |
| imports_unresolved | Package not in deps (transitive only) | "Add dependency: `{pkg}`" | `basilisk.uv.add` |
| imports_unresolved | Package declared but not synced | "Sync environment" | `basilisk.uv.sync` |
| BSK-0152 | Package installed, typeshed stub exists | "Install type stubs: `types-{pkg}`" | `basilisk.uv.addDev` |
| BSK-0152 | Package installed, **no** typeshed stub | "Create local type stub for `{pkg}`" | `basilisk.stubs.createLocal` |

The `uv`-backed actions execute via `workspace/executeCommand`: the LSP spawns `uv` as a subprocess, reports progress via `window/logMessage`, and re-resolves on completion — the diagnostic clears automatically.

The create-local action is offered for **every** BSK-0152 (the only fix when typeshed publishes nothing, a fallback when it does), so the "every diagnostic has a code action" guarantee holds even for packages with no published stubs.

Diagnostic help text describes **what's wrong**, not a CLI command; the action is the fix. See [LSP-UV-INTEGRATION-SPEC.md §LSPUV-ACTIONS](LSP-UV-INTEGRATION-SPEC.md#LSPUV-ACTIONS).

#### Create Local Stub {#STUBRES-CREATE-LOCAL}

`basilisk.stubs.createLocal` (arg: module name) scaffolds a **strict** local
stub for an untyped package.

- **Target**: `<workspace-root>/.basilisk/stubs/{module}.pyi`, the same Tier-3
  stub cache the resolver auto-includes on its search path (see
  [§STUBRES-AUTOGEN](#STUBRES-AUTOGEN) and `ImportSearchPaths::from_config`), so
  the import re-resolves with **no config edit**.
- **Skeleton (strict by default)**: header comments only, declaring *nothing*.
  Creating it clears `BSK-0152`; because the stub is authoritative
  ([§STUBRES-MEMBER via E0154](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STUB-MEMBER)),
  `imports_module_attribute` then prompts the developer to declare each name used.
  The comment documents the opt-out (a module-level
  `def __getattr__(name: str) -> Any: ...` makes every attribute `Any`), but the
  skeleton deliberately does **not** emit it.
- **Idempotent**: an existing stub is never clobbered (handler returns
  `created: false`).
- After writing, the handler calls `rebuild_registry_and_resolve` so BSK-0152
  clears automatically.

The BSK-0152 `help`/`note` text names this `stub-paths`/`.pyi` route and links
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
| Tier 2 stub symbol | `pandas.read_csv(...) (community stub)` |
| typeshed symbol | `os.path.join (typeshed)` |
| custom-typeshed stdlib symbol | `os.uname (custom typeshed)` |
| Tier 1 stub symbol | `requests.get(...) -> Response` (no annotation — trusted) |

> uv projects enrich import hovers with package version and dependency classification from the `PackageRegistry`; see [LSPUV-HOVER](LSP-UV-INTEGRATION-SPEC.md#LSPUV-HOVER).

---

## Configuration {#STUBRES-CONFIG}

### Suppression ownership {#STUBRES-SUPPRESSION}

Stub/import diagnostics use the checker's ordinary severity and inline-suppression system;
there is no stub-specific suppression grammar. See
[CHKARCH-STRICTNESS-SUPPRESSION](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-SUPPRESSION).

| Config key (`[tool.basilisk]`) | Type | Default | Description |
|-------------------------------|------|---------|-------------|
| `stub-paths` | `string[]` | `[]` | Additional directories to search for `.pyi` stubs (resolution step 1 — [§STUBRES-PEP561](#STUBRES-PEP561)) |
| `typeshed-commit` | `string` | _(unset → track `main`)_ | Pin the runtime typeshed clone to an exact commit SHA and freeze it — no TTL polling ([§STUBRES-TYPESHED-CLONE](#STUBRES-TYPESHED-CLONE)) |
| `typeshed-cache-path` | `string` | _(OS cache dir)_ | Where the automatic clone is stored; folder-picker in the config UI ([§STUBRES-TYPESHED-CLONE](#STUBRES-TYPESHED-CLONE)) |
| `typeshed-refresh-interval` | `string` | `"24h"` | Update-check TTL when unpinned ([§STUBRES-TYPESHED-CLONE](#STUBRES-TYPESHED-CLONE)) |
| `typeshed-path` | `string` | _(unset → auto-clone)_ | Supply your own typeshed tree; disables the auto-clone and becomes the canonical source for standard-library types (resolution step 3 — [§STUBRES-CUSTOM-TYPESHED](#STUBRES-CUSTOM-TYPESHED)) |

All five keys live in the one project configuration — `pyproject.toml`
`[tool.basilisk]`
([CHKARCH-CONFIG-FILE](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-FILE)) —
with kebab-case as the canonical spelling. The camelCase spellings
(`stubPaths`, `typeshedPath`) are pyright-migration aliases accepted in the
same table and in the pyright compatibility sources (`[tool.pyright]`,
`pyrightconfig.json` —
[ANALYSIS-CONFIG-PRI](LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CONFIG-PRI)). There
is no separate LSP-side JSON configuration.

`pyproject.toml` configuration:

```toml
[tool.basilisk]
stub-paths = ["stubs/"]
# Standard-library typeshed is cloned and refreshed automatically. Tune it:
typeshed-commit = "83c2518a9e6abbda0c44592c3483de459198f887"  # optional: pin & freeze a commit
typeshed-cache-path = ".cache/typeshed"                       # optional: where the clone lives
typeshed-refresh-interval = "24h"                             # optional: TTL when unpinned (default)
# Or supply your own tree and turn cloning off (e.g. MicroPython):
# typeshed-path = "typeshed-micropython"

[tool.basilisk.rules]
"imports_unresolved" = "warning"
```

Scoping `imports_unresolved` differently for part of the tree (for example
vendored code) means placing a `pyproject.toml` with a `[tool.basilisk]` table
in that folder — the nearest entry wins per rule
([CHKARCH-CONFIG-MODEL](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL)) —
or using inline directives at the import site. There are no module-pattern or
glob-path override tables.

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
