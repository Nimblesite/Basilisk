# Basilisk: Complete Type Safety for Python {#CHKARCH}

**Version**: 0.1.0-draft
**Status**: Specification Draft
**License**: MIT

---

## No "strict mode" — behaviour is configuration only {#CHKARCH-CONFIGURATION-ONLY}

Basilisk has **no modes** (no `--strict`, no `off`/`basic`/`standard`/`strict` dial). Everything reported is decided by **configuration alone**: a flat set of per-rule severities set globally, per path, or per file.

1. **The default configuration is pure PEP conformance.** With no config file, Basilisk enables **every rule that implements the Python typing specification, and nothing else**. This unconfigured default is exactly what the conformance scorer runs — no `basilisk.json`, no "conformance mode" ([CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)).

2. **Everything beyond the spec is opt-in configuration.** House-style rules — require-annotation (`BSK-E0001`/`BSK-E0002`/`BSK-E0004`), require-`@override` (`BSK-E0025`), redundant-annotation (`BSK-W0050`), explicit-`Any` nudge (`BSK-W0014`), uv dependency hygiene, stub suggestions — are **off by default**, enabled only in configuration (`strict_annotations = true`, `uv_dependency_diagnostics = true`, …), never implicitly.

"Strict" is a property of a chosen configuration, never a precondition of the conformance score. No PEP rule may be disabled, deleted, or unregistered to move that number ([CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)).

---

## Capability Matrix {#CHKARCH-MATRIX}

| Capability | Pyright | mypy | ty | Pyrefly | Zuban | Ruff | **Basilisk** |
|---|---|---|---|---|---|---|---|
| Implementation | TypeScript | Python/C | Rust | Rust | Rust | Rust | **Rust** |
| License | MIT | MIT | MIT | MIT | AGPL | MIT | **MIT** |
| Default strictness | Gradual | Gradual | Gradual | Gradual | Gradual | N/A | **PEP by default; strict opt-in** |
| PEP conformance (current) | [live results][cf] | [cf] | [cf] | [cf] | [cf] | N/A | **<!--g:score-->100.0%<!--/g:score-->** (self-measured) |
| PEP conformance target | — | — | — | — | — | N/A | **100%** |
| LSP server | Yes | No | Yes | Yes | Yes | No | **Yes** |
| Incremental computation | Lazy eval | Daemon | Salsa | Module-level | No | N/A | **Salsa** |
| Ownership analysis | No | No | No | No | No | No | **Yes** |
| Immutability enforcement | No | No | No | No | No | No | **Yes** |
| Implicit coercion detection | No | No | No | No | No | No | **Yes** |
| Linting | No | No | No | No | No | **Yes** | Native import hygiene ([LSPFMT-IMPORTS](LSP-FORMATTING-SPEC.md#LSPFMT-IMPORTS)) |
| Formatting | No | No | No | No | No | **Yes** | Embeds Ruff formatter ([LSPFMT-ENGINE](LSP-FORMATTING-SPEC.md#LSPFMT-ENGINE)) |
| Plugin system | No | Python hooks | Planned | No | No | No | **WASM plugins** |
| Auto-stub generation | No | stubgen (basic) | No | Inference | No | No | **Tiered stubs** |
| CI output (SARIF/JUnit) | Limited | No | No | No | No | No | **SARIF + JUnit** |
| Multi-threaded | No | No | Yes | Yes | No | Yes | **Yes** |
| Migration tooling | N/A | N/A | No | No | No | N/A | **mypy + Pyright import** |
| VS Code extension | Pylance (proprietary) | No | Yes | Yes | Yes | Yes | **Yes (open source)** |
| No Microsoft dependency | No (Node.js) | Yes | Yes | Yes | Yes | Yes | **Yes** |

> Rival conformance figures move as those tools evolve, so rather than freeze (and inevitably misstate) them here, the rival cells link to the official, continuously-updated scoreboard. Basilisk's **<!--g:score-->100.0%<!--/g:score-->** is self-measured by that same suite's calculator run over the unmodified binary in its default config, against `python/typing@main` at the exact commit recorded in `conformance_report.json` ([CHKARCH-CONFORMANCE](#CHKARCH-CONFORMANCE)); it is not directly comparable to numbers produced under a different methodology or grading.

[cf]: https://github.com/python/typing/blob/main/conformance/results/results.html

---

## Dependency Strategy {#CHKARCH-DEPS}

Depend on established open-source tools rather than reimplementing them.

### Direct Dependencies {#CHKARCH-DEPS-DIRECT}

| Dependency | Purpose | License | Rationale |
|---|---|---|---|
| **`ruff_python_formatter`** | Code formatting | MIT | Embedded in-process — the formatter is Ruff's, no `ruff` CLI. Pinned to the same rev as the parser ([LSPFMT-ENGINE](LSP-FORMATTING-SPEC.md#LSPFMT-ENGINE)). |
| **`ruff_python_parser`** | Python AST parsing | MIT | Battle-tested Rust crate. Powers Ruff. Our parser. |
| **typeshed** | Standard library type stubs | Apache-2.0 | Community standard. We bundle it and extend it. |
| **Salsa** | Incremental computation framework | Apache-2.0/MIT | Powers rust-analyzer. Proven at scale. |
| **`lsp-server`** / **`tower-lsp`** | LSP implementation | MIT | Standard Rust LSP crates. |

### Tools We Do NOT Depend On {#CHKARCH-DEPS-EXCLUDED}

| Tool | Why Not |
|---|---|
| Pyright/Pylance | TypeScript, Microsoft ecosystem. Cannot link. Cannot extend. |
| mypy | Python, too slow for our architecture. Reference only. |
| ty | MIT Rust, but we build our own checker with different philosophy (configuration-driven, PEP-conformant by default). We may contribute upstream or share crates where sensible. |
| Pyrefly | MIT Rust, same reasoning as ty. Different design goals. |
| Node.js | No JavaScript runtime dependency anywhere in the stack. |

### Interoperability {#CHKARCH-DEPS-INTEROP}

| Tool | Interop Strategy |
|---|---|
| **Ruff** | Basilisk **embeds** the `ruff_python_formatter` crate in-process for formatting and reimplements import hygiene natively — the `ruff` CLI is never spawned ([LSPFMT-DECISION](LSP-FORMATTING-SPEC.md#LSPFMT-DECISION)). Configuration unified in `pyproject.toml` (`[tool.ruff.format]`). |
| **typeshed** | Bundled copy of typeshed stubs, updated with each Basilisk release. Users MAY prepend extra stubs via `stub-paths` (resolution step 1) or replace the bundled stdlib typeshed wholesale via `typeshed-path` (resolution step 3), per the typing-spec import-resolution ordering — see [STUBRES-PEP561](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561). |
| **mypy config** | `basilisk migrate --from mypy` reads `mypy.ini` / `setup.cfg` and produces `[tool.basilisk]` config. |
| **Pyright config** | `basilisk migrate --from pyright` reads `pyrightconfig.json` and produces `[tool.basilisk]` config. |
| **PEP 561** | Full support for `py.typed` packages, inline type annotations, and stub-only packages. |

---

## Core Type System {#CHKARCH-TYPESYS}

How Basilisk decides what to report (configuration, not modes), the PEPs it covers, and how it infers, narrows, and reasons about reachability.

### Strictness Model {#CHKARCH-STRICTNESS}

Behaviour is per-rule configuration. The subsections define the default (pure PEP conformance), suppression/override directives, and their precedence.

#### No Modes — Configuration Decides Everything {#CHKARCH-STRICTNESS-ONLY}

The require-annotation house rules (`BSK-E0001`/`BSK-E0002`) fire **only once enabled in configuration** ([CHKARCH-CONFIGURATION-ONLY](#CHKARCH-CONFIGURATION-ONLY)). Under the default config these snippets pass:

```python
# ERROR: Missing parameter type annotation [BSK-E0001]
def greet(name):
    return f"Hello, {name}"

# ERROR: Missing return type annotation [BSK-E0002]
def greet(name: str):
    return f"Hello, {name}"

# OK
def greet(name: str) -> str:
    return f"Hello, {name}"
```

#### `Any` Is Explicit, Never Implicit {#CHKARCH-STRICTNESS-ANY}

```python
from typing import Any

# ERROR: Implicit Any -- untyped import [imports_unresolved]
from untyped_lib import do_stuff

# OK: Explicit Any with reason
result: Any = do_stuff()  # basilisk: allow[imports_unresolved] -- untyped dependency, tracking in #1234

# ERROR (when the explicit-Any house rule is enabled): Bare Any without justification
def process(data: Any) -> Any:  # BSK-W0011: Explicit Any requires reason comment
    pass
```

#### Diagnostic Severity Modes {#CHKARCH-STRICTNESS-SEVERITY}

Every rule has four severity modes:

| Mode | Behavior | Blocks CI | LSP Indicator |
|---|---|---|---|
| `error` | Full diagnostic with fix suggestions | Yes | Red squiggly |
| `warning` | Diagnostic shown but does not block | No | Yellow squiggly |
| `info` | Informational hint only | No | Blue hint |
| `disabled` | Rule is not checked at all (zero cost) | No | Nothing |

The default mode for each rule comes from its code prefix (`E` = error, `W` = warning). All modes can be overridden per-line, per-block, per-file, and per-project.

#### Inline Suppression and Mode Override {#CHKARCH-STRICTNESS-SUPPRESSION}

Basilisk supports standard `# type: ignore` (mypy/Pyright compatible) plus its own comment directives.

**Per-line: standard compatibility**
```python
from fastmcp import FastMCP  # type: ignore
```

**Per-line: Basilisk-specific with error code**
```python
from fastmcp import FastMCP  # type: ignore[imports_unresolved]
```

**Per-line: severity override (demote or promote)**
```python
from fastmcp import FastMCP  # type: warning[imports_unresolved]
from fastmcp import FastMCP  # type: info[imports_unresolved]
from fastmcp import FastMCP  # type: disabled[imports_unresolved]
```

**Per-line: override all rules on this line**
```python
data = unsafe_cast(value)  # type: warning
data = unsafe_cast(value)  # type: disabled
```

**Per-block: override severity for a range of lines**
```python
# type: disabled[imports_unresolved]
from fastmcp import FastMCP
from result import Result, Ok, Err
from errors import AutomatorError, ErrorCode
from models import Platform, Credentials
# type: end-disabled[imports_unresolved]
```

Block directives work with all modes: `# type: warning[CODE]` / `# type: end-warning[CODE]`, `# type: info[CODE]` / `# type: end-info[CODE]`, `# type: disabled[CODE]` / `# type: end-disabled[CODE]`. Omitting the code applies to all rules.

**Per-file: file-level mode at the top of the file**
```python
# basilisk: relaxed
# All errors become warnings in this file
```

```python
# basilisk: file-disabled[imports_unresolved]
# Disable E0010 for the entire file
```

```python
# basilisk: file-warning[imports_unresolved, returns_compatibility]
# Demote E0010 and E0011 to warnings for the entire file
```

**Per-directory configuration** in `pyproject.toml`:
```toml
[tool.basilisk]
# No "strict"/"mode" switch; opt into house-style rules by name:
strict_annotations = true   # enable the require-annotation rules (BSK-E0001/E0002/E0004)

[tool.basilisk.per-path-overrides."legacy/**"]
disabled = ["returns_compatibility"]              # disable rules entirely for legacy code

[tool.basilisk.per-path-overrides."vendor/**"]
disabled = ["imports_unresolved"]
rules."BSK-E0001" = "warning"
rules."BSK-E0002" = "warning"
```

**Per-module override** (for third-party imports):
```toml
[tool.basilisk.per-module-overrides."requests"]
ignore-missing-stubs = true

[tool.basilisk.per-module-overrides."django.*"]
ignore-missing-stubs = true
```

**Global rule severity override**:
```toml
[tool.basilisk.rules]
"imports_unresolved" = "warning"    # demote globally
"BSK-W0050" = "error"      # promote globally
"dataclasses_order" = "disabled"   # disable globally
```

#### Suppression Precedence {#CHKARCH-STRICTNESS-PRECEDENCE}

When multiple overrides apply, the most specific wins:

1. **Per-line comment** (highest priority)
2. **Per-block comment**
3. **Per-file directive**
4. **Per-path override** in pyproject.toml
5. **Per-module override** in pyproject.toml
6. **Global rule override** in pyproject.toml
7. **Rule default** (lowest priority)

#### Compatibility {#CHKARCH-STRICTNESS-COMPAT}

Recognized comment formats:

| Comment | Behavior |
|---|---|
| `# type: ignore` | Suppress all diagnostics on this line (PEP 484 / mypy / Pyright compatible) |
| `# type: ignore[imports_unresolved]` | Suppress specific code (Basilisk extension, mypy-compatible syntax) |
| `# type: warning` | Demote all diagnostics to warnings (Basilisk-specific) |
| `# type: warning[imports_unresolved]` | Demote specific code to warning (Basilisk-specific) |
| `# type: info` | Demote all diagnostics to info (Basilisk-specific) |
| `# type: info[imports_unresolved]` | Demote specific code to info (Basilisk-specific) |
| `# type: disabled` | Disable all diagnostics on this line (Basilisk-specific) |
| `# type: disabled[imports_unresolved]` | Disable specific code on this line (Basilisk-specific) |
| `# basilisk: relaxed` | Per-file: all errors become warnings |
| `# basilisk: file-disabled[CODE]` | Per-file: disable specific rules |
| `# basilisk: file-warning[CODE]` | Per-file: demote specific rules to warnings |

The `# type:` prefix keeps compatibility with tools that recognize `# type: ignore`; others treat `# type: warning` as unknown and ignore it.

### Python Typing PEP Coverage {#CHKARCH-PEPS}

Basilisk's **target** is 100% conformance with the Python typing specification. We measure against the latest **`python/typing@main`**, recording the exact graded commit by hash in `conformance_report.json` (currently [`<!--g:short-->f4f2952<!--/g:short-->`](https://github.com/python/typing/tree/f4f2952f3ac94d7af819c5c71b60a50a100370e0/conformance)). Today the official scorer, run unmodified in CI on the binary in its default configuration (the PEP conformance set; see [CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)), reports **<!--g:pass-->141<!--/g:pass--> of <!--g:total-->141<!--/g:total--> files passing (<!--g:score-->100.0%<!--/g:score-->)**, with **<!--g:fp-->0<!--/g:fp--> false positives** and **<!--g:missed-->0<!--/g:missed--> missed required errors** (<!--g:caught-->970<!--/g:caught--> caught). We run that suite in CI on every change; the gate ratchets the pass-percentage **up** and the false-positive ceiling **down** — closed only by fixing the checker, never by disabling a rule.

#### Foundation PEPs {#CHKARCH-PEPS-FOUNDATION}

| PEP | Title | Status |
|---|---|---|
| 484 | Type Hints | Required |
| 526 | Variable Annotations | Required |
| 544 | Protocols (Structural Subtyping) | Required |
| 585 | Generics in Standard Collections | Required |
| 604 | Union `X \| Y` Syntax | Required |

#### Advanced PEPs {#CHKARCH-PEPS-ADVANCED}

| PEP | Title | Status |
|---|---|---|
| 586 | Literal Types | Required |
| 589 | TypedDict | Required |
| 591 | Final Qualifier | Required |
| 612 | ParamSpec | Required |
| 613 | TypeAlias | Required |
| 634 | Structural Pattern Matching | Required |
| 646 | Variadic Generics (TypeVarTuple) | Required |
| 647 | TypeGuard | Required |
| 673 | Self Type | Required |
| 675 | LiteralString | Required |
| 681 | Data Class Transforms | Required |
| 692 | TypedDict for **kwargs | Required |
| 695 | Type Parameter Syntax (`def f[T]()`) | Required |
| 696 | TypeVar Defaults | Required |
| 698 | Override Decorator | Required |
| 702 | Deprecated Decorator | Required |
| 742 | TypeIs (Exhaustive Narrowing) | Required |

### Type Inference Engine {#CHKARCH-INFERENCE}

Annotations are enforced on public APIs; local variable types are inferred:

```python
def process(items: list[str]) -> int:
    count = 0              # inferred: int
    filtered = [x for x in items if x.startswith("a")]  # inferred: list[str]
    count = len(filtered)  # OK: int = int
    return count
```

- **Public APIs** (module-level functions/variables, class methods): explicit annotations required
- **Local variables**: inferred from assignments, comprehensions, control flow
- **Cross-module**: does NOT cross boundaries for public symbols; imports from typed modules resolve to declared types, from untyped modules produce `imports_unresolved`

### Type Narrowing and Flow Analysis {#CHKARCH-NARROWING}

Full support for:
- `isinstance()` / `issubclass()` guards with bidirectional narrowing
- Truthiness narrowing (`if x:` narrows `Optional[T]` to `T`)
- Pattern matching exhaustiveness (PEP 634)
- Sentinel / `None` narrowing
- Custom type guards (`TypeGuard`, `TypeIs` per PEP 742)
- Negative narrowing in `else` branches
- Assignment-based narrowing

### Reachability Analysis {#CHKARCH-REACHABILITY}

- Dead code detection after narrowing
- Unreachable branch elimination
- `NoReturn` propagation from `sys.exit()`, `raise`, and custom `NoReturn` functions
- `assert_never()` for exhaustiveness checking
- Platform-aware reachability (default: assume code may run on any platform)

### Target Version and Platform {#CHKARCH-VERSION-TARGET}

Every rule runs against the **configured** target Python version — never a
hardcoded constant (issue #93).

- `BasiliskConfig.python_version` / `python_platform` (from `basilisk.json`
  `pythonVersion`/`pythonPlatform` or `pyproject.toml` `[tool.basilisk]`
  `python-version`/`python-platform`) parse into a typed
  `CheckContext { target_version: (major, minor), target_platform }`
  (`crates/basilisk-checker/src/context.rs`).
- The centralized default is `DEFAULT_TARGET_VERSION = (3, 12)` — the **only**
  place the default version constant lives. A malformed version string falls
  back to the default rather than panicking or disabling gating.
- When the checker config does not pin a version, the CLI and LSP detect it
  from project files per
  [`[LSPUV-PYTHON-VERSION-RESOLUTION-ORDER]`](LSP-UV-INTEGRATION-SPEC.md):
  `.python-version` → `[project].requires-python` lower bound → `uv.lock`
  `requires-python` lower bound (`basilisk_uv::python_version::resolve_target_python_version`).
- `rules::run_all(module, ctx)` threads the context into every
  `Rule::check(module, ctx, diagnostics)` — no rule may reference a literal
  target version.

#### Version/Platform Narrowing {#CHKARCH-VERSION-NARROWING}

- `directives_version_platform` evaluates `sys.version_info` / `sys.platform` guards against
  `ctx.target_version`, so dead-branch analysis follows the project's real
  target.
- `version_target_syntax` rejects PEP 695 syntax (`type X = …`, `class C[T]`, `def f[T]`)
  when `ctx.target_version < (3, 12)` — the target interpreter cannot even
  parse it.

Tests: `crates/basilisk-checker/tests/checker/version_target_tests.rs`.

---

## Mojo-Inspired Safety Analysis {#CHKARCH-MOJO-SAFETY}

> **Status: PLANNED (Phase 4 — see [CHKARCH-ROADMAP-P4](#CHKARCH-ROADMAP-P4)). Not yet implemented.**
> Forward-looking design for the `basilisk-mojo` crate, which is a stub **not wired into the pipeline**. The `generics_defaults`–`specialtypes_never` codes below are **illustrative only** — those numeric codes are currently used by shipping PEP-typing rules (see the [complete diagnostic reference](#CHKARCH-DIAG-REFERENCE) for what each does today).

When implemented, these are **opt-in** rules in the `basilisk-mojo` crate — off by default, enabled only via configuration ([CHKARCH-CONFIGURATION-ONLY](#CHKARCH-CONFIGURATION-ONLY)). They adapt Mojo's ownership, immutability, and coercion concepts as static analysis over standard Python using `typing.Annotated`, decorators, and `dataclass(frozen=True)`; no Mojo runtime required.

### Ownership and Lifetime Tracking {#CHKARCH-MOJO-OWNERSHIP}

Optional ownership annotations via `typing.Annotated`:

```python
from typing import Annotated
from basilisk.safety import Borrowed, Owned, InOut

def process(
    data: Annotated[list[int], Borrowed],     # read-only reference
    buffer: Annotated[list[int], InOut],       # mutable reference
    consumed: Annotated[list[int], Owned],     # ownership transferred
) -> list[int]:
    buffer.append(sum(data))  # OK: buffer is InOut
    data.append(1)            # ERROR: mutation of Borrowed parameter [generics_defaults]
    return consumed

result = process(data=items, buffer=buf, consumed=temp)
print(temp)  # ERROR: use after ownership transfer [directives_cast]
```

**Static analysis rules**:
- `generics_defaults`: Mutation of `Borrowed` parameter
- `directives_cast`: Use-after-move (value used after `Owned` transfer)
- `typeddicts_class_syntax_2`: Implicit copy of large structure (suggest explicit `.copy()`)
- `BSK-W0033`: Missing ownership annotation on mutable parameter (suggestion)

### Immutability by Default {#CHKARCH-MOJO-IMMUTABLE}

Parameters are immutable by default; mutation produces a diagnostic unless annotated `InOut`:

```python
def bad(items: list[int]) -> None:
    items.append(1)  # ERROR: mutation of parameter [enums_behaviors]
    items = [1, 2]   # ERROR: reassignment of parameter [calls_argument_count]

def good(items: Annotated[list[int], InOut]) -> None:
    items.append(1)  # OK
```

A plain `@dataclass` warns `prefer frozen=True [BSK-W0042]`; `@dataclass(frozen=True)` is OK.

### Structural Discipline {#CHKARCH-MOJO-STRUCTURAL}

```python
c = Config(host="localhost", port=8080)
c.timeout = 30  # ERROR: dynamic attribute on typed structure [aliases_newtype]
```

**Rules**:
- `aliases_newtype`: Dynamic attribute assignment on typed class
- `literals_parameterizations`: Missing `__init__` on class with type annotations
- `dataclasses_frozen`: Missing `__del__` on class managing resources (when detectable)
- `BSK-W0053`: Class should use `__slots__` for performance (suggestion)

### No Implicit Type Coercion {#CHKARCH-MOJO-COERCION}

```python
x: float = 1        # ERROR: implicit int-to-float coercion [dataclasses_order]
y: int = True        # ERROR: implicit bool-to-int coercion [enums_expansion]
z: str = b"hello"    # ERROR: implicit bytes-to-str [specialtypes_never]
```

Explicit conversions (`float(1)`, `int(True)`) are OK.

### Mojo-Inspired Rule Mapping {#CHKARCH-MOJO-COMPAT}

| Mojo Concept | Basilisk Equivalent | Syntax | Enforceable via Static Analysis? |
|---|---|---|---|
| `fn` (strict function) | All `def` is strict | `def f(x: int) -> int` | Yes |
| `borrowed` (immutable ref) | Default parameter behavior | No annotation needed | Yes |
| `inout` (mutable ref) | `Annotated[T, InOut]` | `from basilisk.safety import InOut` | Yes |
| `owned` (ownership transfer) | `Annotated[T, Owned]` | `from basilisk.safety import Owned` | Yes |
| `var` / `let` | Immutable by default | Mutation requires `InOut` | Yes |
| `struct` (static type) | Class with annotations | `class Foo:` + typed fields | Yes |
| `^` (transfer operator) | Use-after-move detection | Tracked via `Owned` annotation | Yes |
| `Copyable` / `Movable` traits | Protocol-based | `class Foo(Copyable):` | Yes |
| No implicit coercion | Explicit conversion required | `float(1)` not `1` | Yes |
| SIMD types | Not applicable | N/A (runtime feature) | No |
| Register-passable | Not applicable | N/A (compiler feature) | No |
| Compile-time parameters | Not applicable | N/A (compiler feature) | No |

---

## Diagnostic Rules {#CHKARCH-DIAG}

### Design Philosophy {#CHKARCH-DIAG-PHILOSOPHY}

Every diagnostic must be:
1. **Precise** -- exact location (file, line, column, span)
2. **Clear** -- explains what is wrong and why
3. **Actionable** -- suggests at least one fix
4. **Stable** -- error codes are never renumbered or reused

### Error Code System {#CHKARCH-DIAG-CODES}

Format: `BSK-Xnnnn` where X = default severity class:
- `E` = Error (blocks CI by default)
- `W` = Warning (does not block by default)
- `I` = Info (suggestion by default)

The prefix sets the **default** severity. Every rule can be overridden to any of the four modes (`error`, `warning`, `info`, `disabled`) at every scope level (line, block, file, path, global) — see [CHKARCH-STRICTNESS-SEVERITY](#CHKARCH-STRICTNESS-SEVERITY) and [CHKARCH-STRICTNESS-SUPPRESSION](#CHKARCH-STRICTNESS-SUPPRESSION).

### Rule Categories {#CHKARCH-DIAG-CATEGORIES}

> **Classification is by tags, not categories.** The authoritative classification is the tagging system — provenance tags (`pep`/`basilisk`), PEP-category tags, free-form tags. The code-range groupings below are a coarse legacy convenience; source of truth is [Rule Tagging](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG) ([CHKTAG]).

#### Missing Annotations (BSK-E0001 -- BSK-E0009) {#CHKARCH-DIAG-MISSING}
#### Type Safety (imports_unresolved -- typeddicts_class_syntax) {#CHKARCH-DIAG-TYPESAFETY}

These legacy code-range groupings are superseded by tagging and the complete reference below ([CHKARCH-DIAG-REFERENCE](#CHKARCH-DIAG-REFERENCE)); anchors retained for cross-reference continuity.

#### Complete diagnostic reference {#CHKARCH-DIAG-REFERENCE}

The full set of codes the checker emits — generated from rule source by `scripts/gen_rules_reference.py`, the authoritative list. Keep in sync after adding or renaming a rule.

| Code | Description |
|---|---|
| `BSK-E0001` | Missing parameter type annotation |
| `BSK-E0002` | Missing return type annotation |
| `BSK-E0003` | Missing variable type annotation |
| `BSK-E0004` | Missing `*args` / `**kwargs` type annotation |
| `BSK-E0005` | Missing class attribute type annotation |
| `imports_unresolved` | Unresolved import |
| `returns_compatibility` | Return type mismatch (literal return value incompatible with the declared return type) |
| `calls_argument_type` | Argument type mismatch at a call site |
| `returns_compatibility_2` | Return type mismatch — inferred return type incompatible with annotation |
| `assignment_compatibility` | Assignment type incompatibility (literal mismatches) |
| `callables_annotation` | Invalid type argument count or form |
| `classes_override` | Incompatible method override |
| `classes_override_2` | Incompatible class attribute override |
| `names_undefined` | Undefined variable used in a return statement |
| `names_unbound` | Unbound variable on some code paths |
| `overloads_definitions` | Missing `@overload` implementation |
| `overloads_consistency` | Overlapping `@overload` signatures |
| `dict_key_hashable` | Unhashable type used as a dict key |
| `match_exhaustiveness` | Non-exhaustive `match` statement |
| `annotations_typeexpr` | Invalid type form — numeric literal used as type annotation |
| `BSK-E0025` | Missing `@override` decorator |
| `generics_basic` | `TypeVar` declared with exactly one constraint |
| `generics_base_class` | Duplicate `TypeVar` in a `Generic[...]` base |
| `typeddicts_class_syntax` | Method defined inside a `TypedDict` class |
| `generics_defaults` | Non-default `TypeVar` follows a default `TypeVar` in `Generic[...]` |
| `directives_cast` | Invalid `cast()` call |
| `typeddicts_class_syntax_2` | Invalid keyword argument in `TypedDict` class definition |
| `directives_reveal_type` | Invalid `reveal_type()` call |
| `qualifiers_final_decorator` | `@final` decorator violations |
| `typeddicts_required` | `Required` / `NotRequired` used in an invalid context |
| `classes_classvar` | `ClassVar` used in an invalid context |
| `typeddicts_alt_syntax` | Invalid `TypedDict(...)` functional-syntax call |
| `typeddicts_inheritance` | Invalid `TypedDict` inheritance |
| `directives_assert_type` | Invalid `assert_type()` call |
| `enums_behaviors` | Invalid Enum subclassing |
| `calls_argument_count` | Too few arguments in a function call |
| `generics_syntax_compatibility` | PEP 695 type parameter syntax mixed with traditional `TypeVars` |
| `generics_basic_2` | Non-TypeVar argument in `Generic[...]` or `Protocol[...]` |
| `qualifiers_final_annotation` | `Final` used in an invalid position |
| `qualifiers_annotated` | Invalid first argument to `Annotated[...]` |
| `enums_members` | Enum member annotated with an explicit type |
| `annotations_forward_refs` | Invalid type expression in annotation |
| `aliases_implicit` | Invalid right-hand side for a `TypeAlias` annotation |
| `tuples_type_form` | Multiple unbounded tuple components in a single tuple type |
| `aliases_newtype` | Invalid `NewType(...)` call |
| `literals_parameterizations` | Invalid `Literal` parameterization |
| `dataclasses_frozen` | Assignment to attribute of a frozen dataclass instance, or invalid frozen/non-frozen dataclass inheritance |
| `directives_assert_type_2` | `assert_type()` type mismatch |
| `qualifiers_final_annotation_2` | `Final` type qualifier annotation violations |
| `generics_typevartuple_basic` | Invalid `TypeVar` / `TypeVarTuple` / `ParamSpec` keyword argument combination |
| `typeddicts_readonly` | Mutation of `ReadOnly` `TypedDict` fields |
| `aliases_type_statement` | Invalid RHS in a PEP 695 `type X = rhs` statement |
| `qualifiers_annotated_2` | `Annotated[...]` requires at least two arguments |
| `dataclasses_match_args` | Access to `__match_args__` on a dataclass with `match_args=False` |
| `dataclasses_order` | Invalid ordering comparison of dataclass instances |
| `enums_expansion` | `assert_type` with `Literal[Enum.MEMBER]` on enum-typed param |
| `specialtypes_never` | `-> NoReturn` / `-> Never` function can fall through |
| `dataclasses_hash` | Non-hashable dataclass assigned to a `Hashable`-annotated variable |
| `namedtuples_define_functional` | Invalid argument in a `NamedTuple` constructor call |
| `specialtypes_promotions` | Access to an `int`-only attribute on a `float`-typed parameter |
| `enums_member_values` | Enum member value incompatible with `_value_` type annotation |
| `enums_members_2` | Non-member referenced in `Literal[EnumClass.X]` annotation |
| `literals_parameterizations_2` | `Literal["EnumClass.MEMBER"]` (string) used where `Literal[EnumClass.MEMBER]` (enum member reference) is required |
| `dataclasses_kwonly` | Dataclass constructor argument violations |
| `specialtypes_never_2` | `Never` type compatibility violations |
| `historical_positional` | Historical positional-only parameter violations |
| `overloads_basic` | No matching overload for subscript indexing |
| `namedtuples_type_compat` | `NamedTuple`-to-tuple type incompatibility |
| `constructors_call_new` | Constructor call type mismatch with specialized generic class |
| `generics_self_attributes` | Incompatible type for `Self`-typed attribute |
| `overloads_evaluation` | Overload union expansion failure |
| `generics_self_protocols` | Protocol `Self`-return conformance violation |
| `generics_self_basic` | `Self` type violations in generics |
| `protocols_modules` | Module assigned to incompatible protocol type |
| `generics_upper_bound` | `TypeVar` upper bound violation at call site |
| `generics_typevartuple_unpack` | `TypeVarTuple` unpack minimum type argument violation |
| `generics_typevartuple_callable` | `TypeVarTuple` callable/tuple argument mismatch |
| `generics_typevartuple_basic_2` | `TypeVarTuple` must be unpacked with `*` operator |
| `generics_typevartuple_basic_3` | `TypeVarTuple` variance/bounds/constraints violation |
| `generics_typevartuple_args` | `TypeVarTuple` argument count mismatch |
| `generics_typevartuple_specialization` | Multiple `TypeVarTuple` unpacks in generic or tuple type |
| `BSK-E0087` | Reserved for future PEP 695 type parameter checks |
| `typeddicts_usage` | `TypedDict` runtime violation |
| `generics_syntax_declarations` | Invalid PEP 695 type parameter bound or constraint |
| `tuples_type_form_2` | Invalid tuple type syntax |
| `generics_defaults_2` | Incompatible `TypeVar` bound or constraint with its default |
| `generics_defaults_specialization` | Wrong number of type arguments to a generic class or type alias |
| `typeddicts_operations` | Invalid key or value type in `TypedDict` assignment |
| `generics_self_usage` | `Self` type used in an invalid location |
| `dataclasses_postinit` | `InitVar` field validation in dataclasses |
| `dataclasses_usage` | Type mismatch between a dataclass `field(default_factory=…)` and the field's declared type annotation |
| `protocols_definition` | Protocol method body sets self-attributes not declared in Protocol |
| `protocols_merging` | Non-Protocol base class in a Protocol definition |
| `protocols_explicit` | Direct instantiation of a Protocol class |
| `literals_semantics` | Augmented assignment widens `Literal` type |
| `narrowing_typeguard` | `TypeGuard` or `TypeIs` on method with no narrowing parameter |
| `generics_defaults_referential` | Invalid `TypeVar` default referencing another `TypeVar` |
| `tuples_index` | Tuple index out of bounds |
| `aliases_recursive` | Cyclical type alias reference |
| `generics_syntax_declarations_2` | Invalid attribute access on bounded type variable |
| `protocols_class_objects` | Protocol class used where `type[Proto]` is expected |
| `generics_variance` | Variance incompatibility in base class parameterisation |
| `dataclasses_slots` | Dataclass slots violations |
| `generics_upper_bound_2` | `TypeVar` bound violation at call site |
| `protocols_variance` | Protocol variance violation |
| `constructors_call_init` | Constructor call errors via `__init__` method |
| `narrowing_typeis` | TypeGuard/TypeIs return type incompatibility in callable arguments |
| `narrowing_typeis_2` | `TypeIs` narrows to a type inconsistent with the input type |
| `protocols_runtime_checkable` | Protocol `isinstance`/`issubclass` violations |
| `directives_deprecated` | Use of deprecated class, function, or method |
| `namedtuples_define_class` | `NamedTuple` class definition errors |
| `generics_scoping` | Unbound type variable in scope |
| `protocols_explicit_2` | Calling `super().method()` on an abstract method with no default implementation |
| `protocols_runtime_checkable_2` | Protocol `isinstance`/`issubclass` violations |
| `annotations_generators` | Generator return type and yield type violations |
| `protocols_definition_2` | Protocol conformance violation in an annotated assignment or call argument |
| `callables_protocol` | Callable call-site arity and argument validation |
| `protocols_explicit_3` | `super()` call on abstract protocol method with no default implementation |
| `protocols_subtyping` | Protocol attribute tuple element type mismatch |
| `generics_type_erasure` | Access to instance attribute on a class object |
| `literals_literalstring` | `LiteralString` and `Literal` assignment incompatibilities |
| `tuples_index_2` | Tuple index out of range |
| `generics_defaults_referential_2` | ```TypeVar``` default referential violations |
| `literals_semantics_2` | Literal value assignment incompatibility |
| `generics_variance_inference` | `TypeVar` scoping violation |
| `annotations_generators_2` | Generator yield/send/return type mismatch |
| `generics_base_class_2` | Inconsistent `TypeVar` ordering across base classes |
| `protocols_variance_2` | Protocol `TypeVar` variance mismatch |
| `generics_base_class_3` | Invariant generic type mismatch at call site |
| `callables_subtyping` | Callable subtyping violations (covariance / contravariance) |
| `protocols_generic` | Generic protocol violations |
| `dataclasses_transform_meta` | `dataclass_transform` metaclass violations |
| `generics_typevartuple_specialization_2` | Invalid `TypeVarTuple` specialization of generic alias |
| `callables_protocol_2` | Callable and Protocol assignment compatibility |
| `callables_kwargs` | Unpack[`TypedDict`] kwargs violations |
| `dataclasses_transform_class` | `dataclass_transform` violations when the transform is applied via a base class |
| `namedtuples_usage` | `NamedTuple` usage violations |
| `constructors_call_type` | Invalid constructor call via `type[T]` parameter |
| `specialtypes_type` | Invalid `type[X]` usage violations |
| `protocols_class_objects_2` | Protocol class object violations |
| `tuples_type_compat` | Tuple starred-unpack type compatibility violation |
| `generics_basic_3` | Generic type argument violations |
| `generics_syntax_scoping` | PEP 695 generic type parameter scoping violations |
| `directives_version_platform` | Variable defined only in dead version/platform branch |
| `aliases_typealiastype` | Invalid `TypeAliasType(...)` call |
| `BSK-E0152` | Missing type stubs for installed package |
| `constructors_callable` | Invalid call to a constructor-derived callable ([CHKARCH-DIAG-CTOR-CALLABLE](#CHKARCH-DIAG-CTOR-CALLABLE)) |
| `imports_module_attribute` | Access to a module attribute a local stub does not declare ([CHKARCH-DIAG-STUB-MEMBER](#CHKARCH-DIAG-STUB-MEMBER)) |
| `version_target_syntax` | PEP 695 syntax used below the configured target version ([CHKARCH-VERSION-TARGET](#CHKARCH-VERSION-TARGET)) |
| `typeddicts_extra_items` | TypedDict `extra_items` / `closed` (PEP 728) violations ([CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS](#CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS)) |
| `dataclasses_inheritance` | Dataclass field without a default after one with a default ([CHKARCH-DIAG-OWNERSHIP](#chkarch-diag-ownership)) |
| `overloads_consistency_2` | Inconsistent decorators across an `@overload` group — `@staticmethod`/`@classmethod` not uniform, or `@final`/`@override` on an overload signature ([CHKARCH-DIAG-OWNERSHIP](#chkarch-diag-ownership)) |
| `classes_override_3` | `@override` on a method with no matching ancestor method (PEP 698) ([CHKARCH-DIAG-OWNERSHIP](#chkarch-diag-ownership)) |
| `overloads_consistency_3` | Overload implementation inconsistent with its signatures (overload return not assignable to impl return, or impl parameter cannot accept an overload's) ([CHKARCH-DIAG-TYPESAFETY](#chkarch-diag-typesafety)) |
| `BSK-W0011` | Undeclared dependency import |
| `BSK-W0012` | Unused dependency |
| `BSK-W0013` | Stale uv lock file |
| `BSK-W0014` | Explicit `Any` annotation — prefer a concrete type (style nudge; split from `returns_compatibility`, see [CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)) |
| `BSK-W0015` | Test runner `pytest` not installed in the uv project ([LSPTEST-UV-INTEGRATION-TEST-DEPENDENCY-VERIFICATION](LSP-TEST-INTEGRATION-SPEC.md#LSPTEST-UV-INTEGRATION-TEST-DEPENDENCY-VERIFICATION)) |
| `BSK-W0040` | Lambda function missing type annotations |
| `BSK-W0050` | Redundant type annotation warning |

#### Constructor-to-callable conversion {#CHKARCH-DIAG-CTOR-CALLABLE}

`constructors_callable` implements the typing-spec rule
["Converting a constructor to callable"](https://typing.readthedocs.io/en/latest/spec/constructors.html#converting-a-constructor-to-callable).
When a class object flows through an identity-over-callable function
(`def f(cb: Callable[P, R]) -> Callable[P, R]`), the returned value gains the
class's constructor-to-callable signature, and calls to a variable bound that way
are validated against it.

Synthesized signature, in priority order:

1. The metaclass `__call__` (when declared) — e.g. `__call__(*args, **kwargs)` accepts any call.
2. `__new__` when its return type is neither `Self` nor the class itself (e.g. `-> int`, `-> Proxy`, `-> Any`); `__init__` is then ignored.
3. Otherwise `__init__` (or `__new__` when no `__init__`); a class with neither synthesizes a zero-argument callable returning the instance.

Fires when a call supplies too few/many positional arguments, names a non-parameter keyword, or binds a function-scoped `TypeVar` inconsistently (`list[T]` filled by both `list[int]` and `list[str]`). Conservative: starred positional args and `**kwargs` unpacking suppress arity checks. Implemented in `crates/basilisk-checker/src/rules/e0153.rs`; tests in `crates/basilisk-checker/tests/e0153_tests.rs`.

#### Strict local-stub member access {#CHKARCH-DIAG-STUB-MEMBER}

`imports_module_attribute` makes a **user/local stub authoritative**: when `import X` resolves
to a `.pyi` under a configured `stub-paths` directory (including the
auto-discovered `.basilisk/stubs/` the "Create local type stub" quick fix writes —
see [STUBRES-CREATE-LOCAL](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CREATE-LOCAL)),
accessing `X.attr` where the stub declares neither `attr` nor a module-level
`def __getattr__` is a hard error.

The `def __getattr__(name: str) -> Any: ...` in the create-local skeleton is the
**explicit opt-out**: keep it and every attribute is permitted (module stays `Any`);
remove it and declare specific symbols to opt into checked member access.

Scope (Phase 1): only plain, single-segment `import X` backed by a user stub. The
member API is captured during import resolution
(`crates/basilisk-lsp/src/import_resolver.rs`, CLI and LSP paths) and carried on
`ResolvedModule.imported_modules`. That map is populated *only* for user stubs, so
the rule is a no-op for code without local stubs (conformance suite, first-party
code) — false-positive surface is zero by construction. Third-party typeshed /
`py.typed` packages, instance/class attribute access, and dotted/aliased imports
are deferred. Implemented in `crates/basilisk-checker/src/rules/e0154/`; tests in
`crates/basilisk-checker/src/rules/e0154/tests.rs`.

#### TypedDict `extra_items` / `closed` (PEP 728) {#CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS}

`typeddicts_extra_items` implements [PEP 728](https://peps.python.org/pep-0728/) — the
`extra_items=` and `closed=` class keywords on `TypedDict`. `extra_items=T` defines an
infinite set of non-required (read-only when `T` is `ReadOnly[...]`) extra items of value
type `T`; `closed=True` forbids extra items. The rule validates four families, operating
directly on the module AST (independent of resolver state):

1. **Class-definition legality.** `closed=` must be a literal `True`/`False`;
   `extra_items=` may not wrap `Required[...]`/`NotRequired[...]`; a subclass may
   not set `closed=False` when a superclass is `closed=True` or sets
   `extra_items`; a subclass may not set `closed=True` when a superclass has a
   *non-read-only* `extra_items`; and a subclass may not redeclare `extra_items`
   unless the nearest superclass that declares it does so as `ReadOnly[...]` (a
   plain TypedDict carries the implicit read-only `extra_items=ReadOnly[object]`,
   so overriding it is always permitted).
2. **Dict-literal construction.** When a dict literal is assigned to a TypedDict
   with `extra_items=T`, every key outside the declared schema must carry a value
   type assignable to `T`.
3. **TypedDict-to-TypedDict assignability.** When both sides resolve to
   TypedDicts, each source field outside the target schema, and the source's
   effective `extra_items` pseudo-item, must satisfy the target's `extra_items`
   (covariant when the target is read-only, consistent — and non-required — when
   it is not). A plain TypedDict contributes the implicit `ReadOnly[object]`.
4. **Constructor calls.** Calling the class object with a keyword outside the
   declared schema is rejected unless the TypedDict declares a non-read-only
   `extra_items=T` whose type the argument matches.

Implemented in `crates/basilisk-checker/src/rules/e0156/`; conformance fixture is
`conformance/tests/typeddicts_extra_items.py`.

#### `ReadOnly` `TypedDict` inheritance {#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE}

Implements the typing-spec
[read-only `TypedDict` items](https://typing.readthedocs.io/en/latest/spec/typeddict.html#read-only-items)
rules (PEP 705). The foundation is **transitive `TypedDict` recognition**:
`ClassInfo::is_typed_dict` was `true` only for classes naming `TypedDict`
*directly*, so a subclass (`class Album(NamedDict): ...`) was invisible to every
`TypedDict` rule. Shared helpers in
`crates/basilisk-resolver/src/scope/typeddict_meta.rs`
(`is_transitive_typeddict`, `has_extra_items_transitive`,
`transitive_typeddict_names`, `strip_typeddict_qualifiers`) and the field-merge in
`crates/basilisk-resolver/src/visitor/typeddict_schema.rs` (`effective_fields`)
compute each `TypedDict`'s full schema (own + inherited fields, most-derived
declaration winning, carrying `ReadOnly` qualifier and required-ness).

The qualifier rules are enforced across four codes:

- **`typeddicts_inheritance`** (`crates/basilisk-checker/src/rules/e0038.rs`) — redeclaration
  legality. A writable item may not be redeclared `ReadOnly`; a required item
  may not be redeclared not-required; a writable item's value type is invariant
  while a `ReadOnly` item's may be narrowed to a subtype (a different container
  head is a legal narrowing, the same *invariant* container — `list`/`dict`/`set`
  — with different arguments is not). Multiple inheritance with two bases
  declaring a field with conflicting core type, required-ness, or read-only-ness
  is rejected. The decision functions (`parse_field_qualifiers`,
  `redeclaration_violation`, `value_type_incompatible`, `type_head`,
  `is_invariant_container`, `bases_conflict`) are pure and mutation-tested
  (`crates/basilisk-checker/tests/mutation_kill_tests.rs`, every viable mutant
  killed).
- **`typeddicts_readonly`** — writes to an *inherited* `ReadOnly` field that the subclass
  did not redeclare as writable.
- **`typeddicts_operations`** — wrong value type / missing required key against the merged
  schema, including plain reassignment of an already-typed variable.
- **`assignment_compatibility`** — skips dict-literal assignments to transitive `TypedDict`
  subclasses (field-level checking belongs to E0093).

Conformance: flips `typeddicts_readonly_inheritance.py`. Benchmark fixture:
`benchmarks/fixtures/typeddict_readonly_inheritance.py`.

#### Planned analyses {#CHKARCH-DIAG-PLANNED}

The following anchors are retained for historical/spec-ID continuity. The
"ownership", "immutability", and "coercion" categories below describe a
**planned** Mojo-inspired analysis layer (the `basilisk-mojo` crate, targeted
for a future phase). They are **not yet shipping** — the codes that currently
occupy these numeric ranges implement standard PEP-typing rules, listed in the
[complete reference](#CHKARCH-DIAG-REFERENCE) above.

- Ownership safety {#CHKARCH-DIAG-OWNERSHIP} — planned: `Borrowed` / `InOut` / `Owned` reference tracking, use-after-move.
- Immutability {#CHKARCH-DIAG-IMMUTABILITY} — planned: mutation-of-immutable and `Final` enforcement beyond the shipping `Final` checks (qualifiers_final_annotation, qualifiers_final_annotation_2).
- Structural discipline {#CHKARCH-DIAG-STRUCTURAL} — shipping codes in this range cover NewType, `Literal`, frozen-dataclass and related structural rules.
- Coercion safety {#CHKARCH-DIAG-COERCION} — planned: implicit numeric / `bytes`↔`str` coercion detection.
- Optional safety {#CHKARCH-DIAG-OPTIONAL} — narrowing and `Never`/`Optional` rules ship today (e.g. specialtypes_never_2); a dedicated optional-access pass is planned.

---

## Architecture {#CHKARCH-ARCH}

### High-Level Pipeline {#CHKARCH-ARCH-PIPELINE}

```
Source Files (.py)
       |
       v
+------------------+     +-----------+
| basilisk-parser  | <-- | ruff_python_parser (MIT crate, evaluate)
+------------------+
       |  AST
       v
+------------------+
| basilisk-resolver|  Name resolution, scope analysis, imports
+------------------+
       |  Symbol Table + Resolved AST
       v
+------------------+
| basilisk-checker |  Type checking, inference, narrowing, PEP conformance
+------------------+
       |  Typed AST + Diagnostics
       v
+------------------+     +------------------+
| basilisk-lsp     |     | basilisk-cli     |
| (IDE server)     |     | (CI/terminal)    |
+------------------+     +------------------+
       |                         |
       v                         v
  VS Code / Cursor /      Terminal / CI
  Windsurf / Zed /
  Neovim

  (planned: basilisk-mojo — Mojo-inspired ownership / immutability / coercion
   analysis — is not yet wired into the pipeline.)
```

All stages are backed by:
```
+------------------+
| basilisk-db      |  Salsa incremental computation database
+------------------+
```

### Parse Nesting-Depth Guard {#CHKARCH-ARCH-PARSEDEPTH}

`ruff_python_parser` is recursive-descent, and the resolver and checker walk the
AST recursively (as does its `Drop`). All three overflow the thread stack on
pathologically nested input: a bracket expression past ~4 000 levels aborts with
`SIGABRT`, which produced an LSP crash-restart loop. To stay crash-safe — and match
CPython, which rejects this at the *tokenizer* — `parse_source` (the single entry
point into `ruff_python_parser`) runs a nesting-depth guard **before** parsing:

- Measures depth with ruff's **linear lexer** (`lex` + `next_token`), a flat byte
  scan that never recurses and short-circuits at the first violating token.
- Rejects **bracket nesting** (`(`, `[`, `{`, cumulative) deeper than **200**,
  matching CPython's `MAXLEVEL`; message is CPython's verbatim `too many nested parentheses`.
- Rejects **indentation** deeper than **99 levels**, matching CPython's `MAXINDENT`;
  message is verbatim `too many levels of indentation`.
- Rejects **operator chains** longer than **50 000** depth-building tokens in one
  uninterrupted expression context (per bracket level; reset at `,` `;` `=` and
  logical newlines); message `expression too deeply nested`. A flat token stream
  can still build an arbitrarily deep AST — `total = 1 + 1 + …` in generated code
  nests one `BinOp` per term with zero bracket nesting, and the recursive visitors
  abort even the 64 MiB analysis stacks of [LSPARCH-ARCH-STACK] at ~150 000 levels
  (GitHub #278, the LSP crash-restart loop). Counted tokens are the ones that
  deepen the tree (binary/unary operators, `.`, ternary `if`/`else`, `lambda`);
  flat-by-construction operators (`and`/`or` → one `BoolOp` list, chained
  comparisons → one `Compare` list, `,` → one tuple/call node) are deliberately
  exempt so giant flat generated literals stay analysable.

All limits sit well below their overflow floors and above any real source
(~15 brackets / ~10 indents / chains measured safe past 100 000 on the analysis
stacks), so the guard is crash-proof without false positives.
Rejection surfaces as `ParseError::Syntax` (`BSK-PARSE` in the LSP).

Implemented in `crates/basilisk-parser/src/depth.rs` and `…/src/lib.rs`
(`parse_source`); boundary tests in `crates/basilisk-parser/tests/parse_tests.rs`,
the propagation test in `crates/basilisk-checker/tests/checker_tests.rs`, and the
real-binary crash-safety tests in
`crates/basilisk-cli/tests/e2e_deep_expressions.rs`.

### Rust Crate Structure {#CHKARCH-ARCH-CRATES}

```
basilisk/
  crates/
    basilisk-parser/       # Python AST parsing (wraps or extends ruff_python_parser)
    basilisk-resolver/     # Name resolution, scope analysis, import resolution
    basilisk-checker/      # Core type checking engine
    basilisk-mojo/         # Mojo-inspired safety analysis passes
    basilisk-lsp/          # Language Server Protocol implementation
    basilisk-cli/          # Command-line interface
    basilisk-stubs/        # Stub generation, loading, registry client
    basilisk-plugin/       # WASM-based plugin host
    basilisk-db/           # Salsa incremental computation database
    basilisk-safety/       # Python package: Borrowed, Owned, InOut annotations
  editors/
    vscode/                # VS Code extension (VSIX)
    neovim/                # Neovim configuration / plugin
    helix/                 # Helix language config
```

### Crate Dependencies {#CHKARCH-ARCH-DEPGRAPH}

```
basilisk-db (foundation)
  <- basilisk-parser
       <- basilisk-resolver
            <- basilisk-checker
                 <- basilisk-mojo
                      <- basilisk-lsp (leaf: IDE)
                      <- basilisk-cli (leaf: terminal)

basilisk-stubs (standalone, used by basilisk-resolver)
basilisk-plugin (standalone, used by basilisk-checker)
```

### Build System {#CHKARCH-ARCH-BUILD}

- **Cargo workspace** with all crates
- Cross-compilation targets: `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, `aarch64-darwin`, `x86_64-windows`
- CI: `cargo clippy`, `cargo test`, conformance suite, benchmarks, fuzzing (nightly)
- Release: pre-compiled binaries for all platforms (no build dependencies for users)

#### Shared build-info emitter {#CHKARCH-ARCH-BUILD-VERSIONINFO}

Every binary crate exposing a Shipwright `--version` payload (`basilisk-cli`,
`basilisk-profiler-helper`) stamps the same `SHIPWRIGHT_*` env vars (git SHA,
`SHIPWRIGHT_GIT_DIRTY`, build time, target, toolchain) at compile time. The logic
lives once in `basilisk-buildinfo` (`emit_version_env`), so each `build.rs` is a
one-line delegation. `SHIPWRIGHT_BUILD_TIME` uses the same RFC 3339 formatter the
profiler uses for sample timestamps —
`basilisk_common::datetime::rfc3339_from_secs` — so the Howard Hinnant
`civil_from_days` algorithm exists in exactly one place.

---

## Incremental Computation {#CHKARCH-INCREMENTAL}

### Salsa Architecture {#CHKARCH-INCREMENTAL-SALSA}

Basilisk uses the [Salsa](https://crates.io/crates/salsa) incremental computation
framework (the same system powering rust-analyzer) for **in-session** incremental
checking.

- **Input queries**: a file's source text — `SourceFile::text`
  (`crates/basilisk-db/src/db.rs`) — and the effective configuration —
  `ConfigInput::value`, a `ConfigValue(BasiliskConfig)`
  (`crates/basilisk-checker/src/incremental.rs`). The database
  (`BasiliskDatabase`) and the shared `Db` trait live in `basilisk-db`, the
  dependency-graph foundation, so the derived queries are defined in the crates
  that own the work. `ConfigInput` lives in `basilisk-checker` (beside its only
  consumer) so the salsa `Update` wrapper never reaches the salsa-free
  `basilisk-config` leaf crate. The **resolution environment** is likewise a
  tracked input — `SearchPathsInput::value`, an `ImportSearchPaths` (workspace
  roots, `extraPaths`, stub dirs, venv site-packages, and the `uv.lock`-derived
  `PackageRegistry`); `ImportSearchPaths` lives in `basilisk-checker` and derives
  `salsa::Update` directly (its `Arc<PackageRegistry>` compares by value via
  `PartialEq`, so no salsa dependency reaches `basilisk-uv`).
- **Derived queries**: the per-file diagnostics
  (`crates/basilisk-checker/src/incremental.rs`). `checked_file` runs `parse →
  resolve → check_with_config`, keyed on `(file, config)` — the **pure**,
  import-free pipeline. `checked_file_resolved` additionally runs
  `resolve_module_imports` between resolve and check, keyed on `(file, config,
  search_paths)` — the **full** pipeline the batch CLI runs. Two further
  queries carry the cross-module view: `module_exports(file)` derives a
  workspace file's exported symbols from its tracked text (its `PartialEq`
  value enables **backdating** — a body-only edit re-derives an equal export
  set and every importer's memo stays valid), and `cross_resolved_module` /
  `checked_file_cross` layer `imported_symbols` population
  (`crates/basilisk-checker/src/exports.rs`) over `resolved_module`, resolving
  workspace-tracked imports through `module_exports` and external `.pyi` /
  PEP 561 `py.typed` sources from disk. Granularity is **module-level**: each
  pipeline is fused into one tracked query per file, matching the
  `Module-level` granularity row in [CHKARCH-MATRIX]. Editing one file — or the
  configuration, or the search paths — re-executes only the affected queries;
  unrelated files are served from their memos.

The value type is the owned `CachedDiagnostic` (it satisfies salsa's `Update`
bound), so the engine adds **no** salsa dependency to `basilisk-resolver` or
`basilisk-stubs`.

**Equivalence guarantee.** Each query is a pure memoization wrapper over the
[check pipeline](#CHKARCH-ARCH-PIPELINE). For any file that parses and resolves,
`file_diagnostics(db, file, config)` equals `check_with_config(&resolved, cfg)`
byte-for-byte (and, with the default config, `check(&resolved)`);
`file_diagnostics_resolved(db, file, config, search_paths, workspace)` equals
`{ resolve_module_imports; check_with_config }` byte-for-byte — i.e. the batch
CLI's `process_file` core — **when `workspace` (the `WorkspaceFiles` registry) is
empty or every tracked file's `SourceFile` matches disk.** With a non-empty
registry the user-stub re-capture intentionally reads a tracked `.pyi`'s
in-memory text instead of disk, so it *diverges* from `process_file` for an
edited-but-unsaved stub (correct editor behaviour, but no longer byte-identical
to disk). Both equalities are asserted directly with an empty registry
(`crates/basilisk-checker/tests/incremental_tests.rs`
`checked_file_is_equivalent_to_direct_check` +
`checked_file_honours_strict_annotations`;
`incremental_resolved_tests.rs`
`resolved_query_equivalent_to_direct_import_pipeline` +
`resolved_query_applies_import_resolution`), so salsa memoization can never
corrupt a result.

**Cross-file invalidation + filesystem-impurity boundary.** `resolved_module`
takes a `WorkspaceFiles` input (a path → `SourceFile` map) and, after
resolution, records a **content edge on exactly the imports whose output
depends on content**: workspace-tracked **user-stub `.pyi`** imports, whose
member API is re-derived from the tracked text
(`recapture_user_stub_from_source`) — so editing the stub's content updates
the importer's `imports_module_attribute` diagnostics, an edge that changes
*output*, not just triggers a re-run
(`editing_a_user_stub_updates_the_importer_diagnostics` at the checker level;
`editing_open_stub_refreshes_importer_via_salsa` proves it end-to-end through
the LSP with the disk left stale). A non-stub import records **no** text edge
— the importer's resolved module is identical for any content of the imported
file (`editing_a_non_stub_imported_file_does_not_reparse_the_importer`), and a
coarse text edge would re-parse every importer on any dependency keystroke.
Sibling `.py` **type/symbol** sharing instead flows through
`cross_resolved_module`: workspace imports depend on the imported file's
`module_exports`, so an export edit updates the importers' diagnostics from
tracked (possibly unsaved) content while a body-only edit backdates and
re-checks nothing
(`body_edit_backdates_exports_and_export_edit_propagates`,
`crates/basilisk-checker/tests/incremental_cross_tests.rs`). What remains
**untracked** (mirroring
[CHKCACHE-LIMITS](CHECKER-CACHE-SPEC.md#CHKCACHE-LIMITS)):
`resolve_module_imports`' existence probes and the content of files *outside*
the workspace (third-party packages, venv site-packages, external stubs) —
those invalidate only on a re-set `SearchPathsInput` / `WorkspaceFiles`.

**Input writes compare-before-set.** Salsa 0.27 treats *every* input `set` as
a new revision — a same-value write still re-executes dependents (pinned by
`crates/basilisk-checker/tests/salsa_set_semantics.rs`). The LSP engine
therefore compares each input (source text, config, search paths) against the
stored value and writes only on a real change; without the guard, syncing
inputs on every analysis would silently discard the database's memos and turn
every workspace sweep into a full recompute.

**LSP adoption.** The engine is the LSP's analysis path once the workspace scan
has built the search paths. `basilisk-lsp`'s `SalsaAnalysisEngine`
(`salsa_engine.rs`) holds a persistent [`BasiliskDatabase`] plus the input
handles (one `SourceFile` per file, one `ConfigInput` per root, one
`SearchPathsInput`, one `WorkspaceFiles` registry), sets them to the current
values on each analysis, and reads the resolved-module query (for navigation —
hover / references / go-to-definition) and its diagnostics projection; in
`crossModule` mode these are the cross-module variants (`cross_resolved_module`
/ `file_diagnostics_cross`), elsewhere the plain CLI-parity pair.
`WorkspaceIndex::analyse_and_resolve` routes the `didOpen`/`didChange` path
through it, and `WorkspaceIndex::reresolve_imports_and_recheck` — the
post-scan re-check, the config/`uv.lock` refresh, and the dependent refresh
when an edited file's exports change — is a **salsa sweep**: it primes the
engine with every indexed file's current text (open buffers included) and
re-analyses each file through the memoized queries, so only files whose
dependencies actually changed recompute. The pre-scan import-free path is
unchanged. `ResolvedModule` (and its transitively-contained types) derive
`PartialEq` so `Arc<ResolvedModule>` satisfies salsa's `Update` bound via the
fallback, keeping `basilisk-resolver` salsa-free.

**What the engine replaced, and what remains outside it.** The former LSP-side
cross-module machinery is gone: `cross_module.rs` (two-pass
`populate_cross_module_symbols`) and `resolve_workspace_imports` were retired
in favour of the queries above, and `import_graph.rs` is reduced to the
navigation handlers' reverse lookups ([ANALYSIS-GRAPH]) — invalidation no
longer walks the graph. The startup scan itself analyses through the engine:
search paths are built first, the engine is primed with every collected
file's text, and each file runs the memoized queries exactly once
([ANALYSIS-STARTUP-WHOLE]) — there is no separate pre-salsa analysis pass.
Still outside salsa: the `FileEntry` index itself (the LSP-side store —
`FileEntry.resolved` shares the salsa memo's `Arc<ResolvedModule>`, no
duplication) and the no-search-paths degrade path (`recheck_all_files`, plus
the per-file import-free fallback used before configuration is known).
Engine `SourceFile`/registry bookkeeping is dropped on file deletion
(`SalsaAnalysisEngine::remove`), though salsa 0.27 cannot reclaim an input's
internal memo, so a deleted file's memo lingers until the database is dropped;
the database's memory footprint scales with the workspace (every analysed
file's inputs and memos stay resident for the session — the standard
incremental-engine trade).

**Scope — the CLI/conformance path is deliberately unchanged.** The batch CLI
(`process_file`) still runs the direct pipeline, so this work **cannot affect the
conformance score**. Routing the CLI (and the LSP's bulk scan) through the engine
is future work — the CLI is the conformance path (must prove byte-for-byte parity
first) and, being one-shot, reuses no memos. The engine is a public API
(`basilisk_checker::{BasiliskDatabase, SourceFile, ConfigInput, ConfigValue,
SearchPathsInput, WorkspaceFiles, ModuleExports, checked_file,
file_diagnostics, resolved_module, module_exports, cross_resolved_module,
checked_file_resolved, checked_file_cross, file_diagnostics_resolved,
file_diagnostics_cross}`).

Incremental behaviour is proven by `crates/basilisk-db/tests/db_tests.rs`
(memoization, invalidation, cross-file isolation) and the checker tests above,
plus `crates/basilisk-checker/tests/incremental_cross_tests.rs` (cross-module
population semantics, PEP 561 gating, backdating, in-memory export
propagation).

### Cancellation {#CHKARCH-INCREMENTAL-CANCEL}

When a new keystroke arrives while a check is in progress, the in-flight
computation must be abandoned rather than run to completion and waste work — this
is what keeps an editor responsive under fast typing. Salsa provides this: a write
raises the revision's cancellation flag, and the next query checkpoint unwinds
with the `Cancelled` sentinel. Verified deterministically by
`crates/basilisk-db/tests/db_tests.rs::cancellation_unwinds_in_flight_work`.

### Persistent Cache {#CHKARCH-INCREMENTAL-CACHE}

Cross-session persistence is the **content-addressed result cache**
([CHKCACHE](CHECKER-CACHE-SPEC.md), `crates/basilisk-db/src/cache.rs`), not salsa:
a fresh process loads cached diagnostics and recomputes only files whose recorded
read-set changed on disk, eliminating cold-start cost. The two layers are
complementary — salsa makes an *editing session* incremental; the result cache
makes *repeat invocations* incremental — and a hit in either is sound by
construction (salsa via tracked dependencies, the result cache by re-verifying
every recorded file).

### Performance Targets {#CHKARCH-INCREMENTAL-PERF}

These are design targets, not yet measured against the salsa path (the benchmark
harness in [ROADMAP-NEXT-STEPS-PLAN](../plans/ROADMAP-NEXT-STEPS-PLAN.md) is the
vehicle for validating them); they are not a claim of achieved numbers.

| Scenario | Target |
|---|---|
| Cold start, 100K LOC | < 5 seconds |
| Cold start, 1M LOC | < 30 seconds |
| Incremental (single file edit) | < 10ms |
| Memory, 1M LOC | < 2 GB |

---

## Language Server Protocol {#CHKARCH-LSP}

The LSP server and the CLI are two front-ends over the same engine, so interactive and CI results are always consistent. For the complete LSP specification — features, custom commands, configuration settings, binary resolution, and DAP integration — see **[LSP-ARCHITECTURE-SPEC.md](LSP-ARCHITECTURE-SPEC.md)**.

### Supported LSP Methods {#CHKARCH-LSP-METHODS}

See [LSP-ARCHITECTURE-SPEC.md §LSPARCH-FEATURES](LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES) for the complete specification. Summary:

| Method | Description |
|---|---|
| `textDocument/diagnostic` | Push diagnostics (all BSK rules) |
| `textDocument/hover` | Type information, docstrings, ownership annotations |
| `textDocument/completion` | Type-aware completions with auto-import |
| `textDocument/definition` | Go to definition |
| `textDocument/references` | Find all references |
| `textDocument/rename` | Symbol rename across workspace |
| `textDocument/codeAction` | Quick fixes for every diagnostic |
| `textDocument/signatureHelp` | Parameter hints |
| `textDocument/inlayHint` | Inferred types, parameter names, ownership |
| `textDocument/semanticTokens` | Semantic highlighting |
| `callHierarchy/incomingCalls` | Incoming call hierarchy |
| `callHierarchy/outgoingCalls` | Outgoing call hierarchy |
| `typeHierarchy` | Type inheritance navigation |

### Custom LSP Commands {#CHKARCH-LSP-COMMANDS}

See [LSP-ARCHITECTURE-SPEC.md §LSPARCH-CMDS](LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDS) for the complete specification.

---

## Editor Integrations {#CHKARCH-EDITORS}

Each editor has a dedicated specification document:

| Editor | Spec | Status |
|---|---|---|
| **VS Code** | [`VSIX-SPEC.md`](VSIX-SPEC.md) | Primary integration |
| **Zed** | [`ZED-SPEC.md`](ZED-SPEC.md) | First-class Zed extension |
| **Neovim** | [`NEOVIM-SPEC.md`](NEOVIM-SPEC.md) | basilisk.nvim plugin |
| **Helix** | Built-in LSP support. Language configuration provided. | Config only |
| **Emacs** | `eglot` / `lsp-mode` configuration. | Config only |

All editors connect to the same `basilisk lsp` binary via stdio; extensions are thin integration layers over the single LSP backend.

---

## Command-Line Interface {#CHKARCH-CLI}

### Core Commands {#CHKARCH-CLI-COMMANDS}

```bash
basilisk check [paths...]         # Type check files/directories
basilisk check --watch            # Watch mode with incremental rechecks
basilisk stats [paths...]         # Type coverage report
basilisk stubs generate <package> # Generate stubs for installed package
basilisk migrate --from mypy      # Import mypy configuration
basilisk migrate --from pyright   # Import pyright configuration
basilisk init                     # Generate starter pyproject.toml config
```

### Output Formats {#CHKARCH-CLI-OUTPUT}

| Format | Flag | Use Case |
|---|---|---|
| Human-readable | Default | Terminal (color, source context, fix suggestions) |
| JSON | `--output-format json` | Programmatic consumption |
| SARIF | `--output-format sarif` | GitHub Code Scanning, Azure DevOps |
| JUnit XML | `--output-format junit` | CI test result dashboards |

### Exit Codes {#CHKARCH-CLI-EXITCODES}

| Code | Meaning |
|---|---|
| 0 | Clean -- no errors |
| 1 | Type errors found |
| 2 | Configuration error |
| 3 | Internal error |

### CI Integration {#CHKARCH-CLI-CI}

**GitHub Actions**:
```yaml
- uses: MelbourneDeveloper/setup-basilisk@v1
- run: basilisk check --output-format sarif > results.sarif
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

**pre-commit**:
```yaml
- repo: https://github.com/Nimblesite/Basilisk
  rev: v0.1.0
  hooks:
    - id: basilisk-check
```

---

## Stub System {#CHKARCH-STUBS}

### Auto-Stub Generation {#CHKARCH-STUBS-AUTOGEN}

Stub generation engine with three modes:

1. **Runtime introspection**: import the package, inspect objects, generate `.pyi`
2. **AST-based inference**: parse package source, infer signatures without importing
3. **Hybrid**: both, preferring runtime data with AST fallback

### Stub Quality Tiers {#CHKARCH-STUBS-TIERS}

| Tier | Source | Trust Level | Diagnostic Behavior |
|---|---|---|---|
| Tier 1 | Hand-written, verified, typeshed | High | No warnings |
| Tier 2 | Auto-generated, community reviewed | Medium | Info notes on potential inaccuracies |
| Tier 3 | Best-effort inference | Low | Warnings that types may be incomplete |

### typeshed Compatibility {#CHKARCH-STUBS-TYPESHED}

Basilisk bundles typeshed as the Tier 1 baseline for standard-library stubs
(import-resolution step 3 — [STUBRES-PEP561](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561)).
Per the typing spec, "type checkers SHOULD provide an option for users to
provide a path to a directory containing a custom or modified version of
typeshed; if this option is provided, type checkers SHOULD use this as the
canonical source for standard-library types in this step"
([import resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering)).
Basilisk therefore honours `typeshed-path` to replace the bundled stdlib
typeshed wholesale as the canonical stdlib source, distinct from `stub-paths`
(resolution step 1), which *prepends* additional `.pyi` stub directories. The
canonical resolution order and override semantics live in
[STUBRES-CUSTOM-TYPESHED](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED).

---

## Plugin and Extension System {#CHKARCH-PLUGINS}

### Architecture {#CHKARCH-PLUGINS-ARCH}

**WASM-based** for security and portability: plugins compile to WebAssembly, run sandboxed (no filesystem, no network), receive AST nodes and type information, and return diagnostics and code actions.

### Extension Points {#CHKARCH-PLUGINS-EXTENSIONS}

| Extension Point | Example |
|---|---|
| Custom diagnostic rules | Flag Django `QuerySet` misuse |
| Custom type providers | Infer SQLAlchemy model field types |
| Custom code actions | Generate Pydantic validator stubs |
| Custom type narrowing | Django `get_object_or_404` narrows to model type |

### Distribution {#CHKARCH-PLUGINS-DIST}

Plugins declared in `pyproject.toml`:
```toml
[tool.basilisk.plugins]
django = "basilisk-plugin-django >= 0.1"
pydantic = "basilisk-plugin-pydantic >= 0.1"
```

---

## Configuration {#CHKARCH-CONFIG}

### Configuration File {#CHKARCH-CONFIG-FILE}

All configuration lives in `pyproject.toml`:

```toml
[tool.basilisk]
python-version = "3.12"
python-platform = "All"          # Default: check for all platforms
stub-paths = ["stubs/"]          # resolution step 1: prepend extra .pyi stub dirs
# typeshed-path = "typeshed-x"   # resolution step 3: replace the bundled stdlib typeshed
include = ["src/", "tests/"]
exclude = ["**/migrations/**"]

[tool.basilisk.mojo-safety]
ownership = true                 # Enable ownership tracking (default: true)
immutability = true              # Parameters immutable by default (default: true)
no-implicit-coercion = true      # Flag implicit type coercion (default: true)

[tool.basilisk.per-path-overrides."legacy/**"]
disabled = ["returns_compatibility"]
rules."imports_unresolved" = "warning"
```

### Include Semantics {#CHKARCH-CONFIG-INCLUDE}

`include` lists the roots scanned when no CLI paths are given. Explicit CLI paths
override it; `exclude` applies within the include roots. When `include` is absent
or empty, the current directory is scanned. Entries resolve relative to the config
file's directory (issue #37).

The **LSP** honors the same `include` roots on both paths, so the editor analyses
exactly the files `basilisk check` would. The bulk scan walks only the include
roots (`WorkspaceIndex::scan_dirs_for`); the per-file/open path suppresses
diagnostics for files outside them (`WorkspaceIndex::is_outside_include_roots`, in
`analyse_and_resolve` and `recheck_all_files`) — so a generated-tree file shows no
diagnostics even when opened, like an `exclude`d file.

### Exclude Semantics {#CHKARCH-CONFIG-EXCLUDE}

`exclude` (and `per-path-overrides` keys) use **gitignore-style globs**, matched
against the path relative to the workspace root:

- a bare name with no `/` matches that segment at **any** depth — `build` excludes
  every `build` dir, `*.pb.py` every generated file;
- `**` matches zero or more directory segments (`**/bundled/**`); `*` / `?` match
  within a single segment only;
- an anchored pattern (containing `/`) matches the full path or any ancestor
  directory, so `vendor/**` or `src/generated` also excludes everything beneath it.

A baseline set of vendored/cache directories is **always** excluded
(`node_modules`, `site-packages`, `.venv`, `__pycache__`, `build`, `dist`, the
extension's `bundled` / `_vendored` trees); user `exclude` entries extend it.
Hidden directories (`.`-prefixed) are always skipped. The single canonical matcher
`basilisk_config::path_matches_pattern` is shared by every entry point so they
exclude identically:

- LSP **workspace scan** (`workspace_scan::is_excluded`),
- CLI **walk** for `check`/`fix`/`adopt` (`is_excluded_path`), and
- LSP **incremental per-file path** (`WorkspaceIndex::is_path_excluded`, in
  `analyse_and_resolve`) — a vendored file *opened* or *edited* is parsed for
  navigation but publishes **no** diagnostics, matching the bulk scan.

### Migration from Existing Tools {#CHKARCH-CONFIG-MIGRATION}

```bash
basilisk migrate --from pyright   # Reads pyrightconfig.json -> pyproject.toml
basilisk migrate --from mypy      # Reads mypy.ini / setup.cfg -> pyproject.toml
```

Semantic mapping:
- Pyright `strict` / mypy `--strict` -> Basilisk with house-style rules enabled in configuration (require-annotation, explicit-`Any`, …), Mojo safety disabled
- Pyright `standard` -> Basilisk's PEP-only default plus selected house rules, softened in `per-path-overrides` where needed

---

## Diagnostics Experience {#CHKARCH-DIAGEXP}

### Quality Standard {#CHKARCH-DIAGEXP-QUALITY}

Diagnostics follow the rustc format:

```
error[BSK-E0001]: Missing parameter type annotation
  --> src/utils.py:14:5
   |
14 | def process(data):
   |             ^^^^ parameter `data` has no type annotation
   |
   = help: Add a type annotation: `data: <type>`
   = note: In Basilisk, all function parameters require explicit types
   = see: https://www.basilisk-python.dev/errors/BSK-E0001
```

### Quick Fixes {#CHKARCH-DIAGEXP-QUICKFIXES}

Every error has at least one associated code action:

| Error | Quick Fix |
|---|---|
| BSK-E0001 (missing param type) | Insert `: <inferred_type>` |
| BSK-E0002 (missing return type) | Insert `-> <inferred_type>` |
| enums_behaviors (mutation of immutable param) | Add `InOut` annotation |
| dataclasses_order (implicit coercion) | Wrap in explicit conversion |

---

## Performance Engineering {#CHKARCH-PERF}

### Parallelism {#CHKARCH-PERF-PARALLEL}

- File-level parallelism using Rayon (work-stealing)
- Module dependency graph partitioned into independent subgraphs
- Cross-module dependencies resolved first in dependency-ordered pass

### Memory {#CHKARCH-PERF-MEMORY}

- Arena allocation for AST nodes
- Interned strings for identifiers and paths
- Memory-mapped file I/O

### Benchmarks {#CHKARCH-PERF-BENCHMARKS}

The suite that exists today is `benchmarks/` — single-construct typing-spec
stress fixtures timed cold across Basilisk, Pyright, mypy, ty, Pyrefly, and
zuban by `benchmarks/run.sh` ([CHKARCH-TESTING-BENCH-RATCHET]).

**Planned, not yet built:** a real-world-codebase suite — **PyTorch** (~600K
LOC), **Django** (~250K LOC), **FastAPI** (~30K LOC), **Python standard
library** (~500K LOC) — with the same comparison baselines. This paragraph is
a design target, not a claim of existing measurement.

---

## Testing Strategy {#CHKARCH-TESTING}

| Layer | Method | Purpose |
|---|---|---|
| Unit tests | `cargo test` per crate | Crate-level correctness |
| Integration tests | Multi-file scenarios | Cross-module type checking |
| Conformance tests | Python typing test suite | PEP compliance (target: 100%) |
| Golden file tests | Expected diagnostic output | Diagnostic regression |
| Fuzzing | `cargo-fuzz` | Crash resistance, soundness |
| Property tests | `proptest` crate | Type system invariants |
| Benchmarks | `make bench` (hyperfine, `benchmarks/run.sh`) vs Pyright/mypy/ty/Pyrefly/Zuban | Performance tracking + regression gate (fails if basilisk regresses >25% vs the committed per-machine `benchmarks/status/<machine>.csv`) |

### PEP Conformance Scoring {#CHKARCH-CONFORMANCE}

The conformance score is produced by **RUNNING the real `python/typing`
conformance harness** — the suite's own `conformance/src/main.py` driving its
built-in `BasiliskTypeChecker` — against the compiled binary on **every run**,
never a Basilisk reimplementation. It is the exact tooling the reference checkers
(pyright, mypy, pyrefly, ty, zuban, pycroscope) are graded with. **A build in which
that official check did not run against a freshly cloned suite is a BUILD FAILURE.**

> ⛔️ **DISABLING, DELETING, OR UNREGISTERING ANY CONFORMANCE RULE IS FORBIDDEN.**
> The binary is scored in its **full, default configuration with EVERY rule
> enabled** — no `basilisk.json`, no per-rule override, no "spec-conformance mode",
> no skipped fixtures, no deleting rule source (`src/rules/*.rs`), no removing rules
> from `all_rules()`. The binary is scored over a **fresh `python/typing` clone**
> whose tree holds no `basilisk.json`, so nothing of ours can silence a rule;
> deleting the rules themselves is the **same crime by another route** and equally
> forbidden — as is hand-editing `conformance/conformance_status.csv` or loosening
> the `coverage-thresholds.json` gate (`threshold` / `max_false_positives`). A
> strict default firing on valid code is a **real conformance gap to FIX in the
> checker**, never to hide. Gaming the number is a punishable offence.

- **Runner — the real harness, nothing else**:
  [`conformance/run_conformance.py`](../../conformance/run_conformance.py) is the
  ONE conformance path. Every run it clones the
  **latest [`python/typing@main`](https://github.com/python/typing/tree/main/conformance)**
  FRESH (we shoot for the current spec suite, not a frozen commit) and runs the
  suite's **OWN unmodified `conformance/src/main.py --only-run basilisk`** against
  the real compiled binary (via `BASILISK_BIN`). The suite already ships the
  official Basilisk adapter — `BasiliskTypeChecker` in
  [`conformance/src/type_checker.py`](https://github.com/python/typing/blob/main/conformance/src/type_checker.py) —
  so **nothing of ours is injected, vendored, adapted, or reimplemented**. The
  harness writes `results/basilisk/*.toml`; every `Pass`/`Fail` verdict and every
  `errors_diff` is the harness's OWN, produced by the same code that grades pyright,
  mypy, pyrefly, ty, zuban and pycroscope. From those real results the runner only
  *reports*: it writes `conformance/conformance_status.csv` and **records the exact
  graded commit hash** in
  [`website/src/_data/conformance_report.json`](../../website/src/_data/conformance_report.json),
  so every published number is pinned *by hash* on the website. There is **NO
  vendored calculator and NO cached-fixtures fallback** — a build in which the real
  harness could not be cloned and run is a **BUILD FAILURE**, by design. (The only
  auxiliary number not in the toml, `caught` = required errors matched, is taken
  from upstream's own `get_expected_errors` imported live from the fresh clone — the
  official function on the official tests, never a copy.)
- **Pass rule** (upstream's, verbatim): a file passes iff the upstream
  `errors_diff` is empty — every `# E` line gets an error, every `# E[tag]`
  group is satisfied, and **no error lands on a line the suite does not mark**.
  `conformance_automated = "Fail" if errors_diff.strip() else "Pass"`.
- **Nothing excluded.** The harness counts **every** diagnostic — errors **and**
  warnings (strictest grading, as pyright is graded); no looser mode, no opt-out.
  The binary runs with **every rule enabled** in its default mode over a fresh
  `python/typing` clone whose tree holds no `basilisk.json`, so nothing of ours can
  silence a rule ([CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)).
- **Gate**: `make test` (via [`scripts/test-rust.sh`](../../scripts/test-rust.sh))
  builds the `basilisk` binary, then runs
  `python3 conformance/run_conformance.py --gate` on it — which runs the REAL
  harness and delegates the 100 %-pass / 0-false-positive check to
  [`conformance/assert_wheel_conformance.py`](../../conformance/assert_wheel_conformance.py)
  over the harness's OWN `results/basilisk/*.toml`. There is **no Rust conformance
  test** and **no in-repo scorer**: the score is the real suite's own verdict on the
  compiled binary. The pass-percentage floor and false-positive ceiling live in
  `coverage-thresholds.json` (`conformance.threshold`,
  `conformance.max_false_positives`); the former ratchets **up**, the latter
  **down**. Per-file results are written to `conformance/conformance_status.csv`.
- **Current score** — measured against `python/typing@main` at the exact graded
  commit recorded in `conformance_report.json`, currently
  [`<!--g:short-->f4f2952<!--/g:short-->`](https://github.com/python/typing/tree/f4f2952f3ac94d7af819c5c71b60a50a100370e0/conformance):
  **<!--g:pass-->141<!--/g:pass--> / <!--g:total-->141<!--/g:total--> = <!--g:score-->100.0%<!--/g:score-->**, **<!--g:fp-->0<!--/g:fp--> false positives**, **<!--g:missed-->0<!--/g:missed--> missed required errors**, with
  **<!--g:caught-->970<!--/g:caught-->** required errors caught. The binary runs in its default configuration — the
  PEP conformance set — over a fresh `python/typing` clone whose tree holds no
  `basilisk.json`, so nothing can silence a rule; Basilisk's opt-in house-style rules never run during scoring,
  so they can neither pad nor sink the number. The gate
  ratchets the pass-percentage **up** and the false-positive ceiling **down**
  (`coverage-thresholds.json` → `conformance.threshold` /
  `conformance.max_false_positives`), driven only by genuinely fixing the checker,
  **never** by disabling a rule. (History: a **baseline reset on 2026-06-26**
  corrected a gamed *fake 100%* that had disabled six house-style rules before
  scoring; conformance has been measured honestly in the default config ever since.)
  Target: **100%**.

#### No "spec-conformance mode" — the scorer runs the genuine default config {#CHKARCH-CONFORMANCE-MODE}

There is **no** conformance mode, and there never will be. The scorer runs the binary
in exactly the configuration a user gets out of the box — the **default config, which
is the pure PEP conformance set** ([CHKARCH-CONFIGURATION-ONLY](#CHKARCH-CONFIGURATION-ONLY))
— with no `basilisk.json`, no per-rule override, and no special scoring path. Basilisk's
opinionated *house-style* rules (require-annotations `BSK-E0001`/`BSK-E0002`/`BSK-E0004`,
require-`@override` `BSK-E0025`, redundant-annotation `BSK-W0050`, the explicit-`Any`
nudge `BSK-W0014`) are **opt-in and off by default**, so they never run during scoring
and can neither pad nor sink the number. The figure is the genuine out-of-the-box
conformance result — currently <!--g:score-->100.0%<!--/g:score-->. Any shortfall would be a real
checker bug to fix (a missing spec feature, or a false positive from an over-strict
*conformance* rule), never something to paper over by silencing a rule.

⛔️ **Disabling, deleting, or unregistering a conformance (PEP) rule to move the number
is forbidden** — as is hand-editing `conformance_status.csv` or loosening the
`coverage-thresholds.json` gate (`threshold` / `max_false_positives`) to match a faked
run. This has been attempted twice, back when the house rules still ran by default and
counted toward the score. First, a revision wrote a `basilisk.json` that turned six
rules off before scoring and reported a **fake 100%**; that was removed, and the
scorer now runs the binary over a **fresh `python/typing` clone** whose tree contains
no `basilisk.json`, so no config can silence a rule. Second — when config-disabling
was blocked — a revision tried to
*delete the offending rule source files outright* and unregister them from
`all_rules()`, then re-report a **fake 100%**: the same lie by another route. **Deleting
a rule to dodge the `basilisk.json` guard is the identical offence.**

The path to 100% is to make the checker **correct**, never to silence a rule at score
time: implement the spec features it still misses, and teach its conformance rules to
stop firing on spec-valid code (recognising inferred return types, honouring `# E`-free
lines) so the false positives fall on their own merits. Anyone may relax rules *in their
own project* via config; the **conformance scorer never does**.

### Mutation Testing Ratchet {#CHKARCH-TESTING-MUTATION-RATCHET}

Mutation testing proves the test suite actually asserts behaviour. Scope only ever **grows** toward all Rust code:

- **Scope is test-driven.** `#[mutation_safe(rule = "<rule-slug>", fns = "fn_a|fn_b")]`
  attributes on e2e tests drive the `cargo mutants` examine regex
  (`scripts/mutation_examine_re.py`). `<rule-slug>` is the rule's path stem under
  `crates/basilisk-checker/src/rules/` (file like `aliases_implicit` or directory
  like `assignment_compatibility`); omitting `fns` scopes the whole file. Adding
  these tests is the only way to widen scope.
- **Baseline is ratcheted.** `mutation_testing/mutation_scores.json` is the committed
  baseline; `mutation_testing/mutants_report.py` fails the build when the **viable
  mutant pool shrinks**, `caught` drops, `missed`/`timeout` rises, or `kill_rate`
  drops. (`unviable` mutants don't compile and are excluded.) Both `make
  mutation-test` and the CI shard merge enforce the same function.
- **Direction.** End state is the full workspace under mutation
  (`make mutation-test ALL=1`); until then each checker-logic PR leaves the viable
  pool the same size or larger.

### Benchmark Non-Regression {#CHKARCH-TESTING-BENCH-RATCHET}

Performance and conformance ratchet **together** — neither traded for the other:

- `make bench` (`benchmarks/run.sh`) fails when basilisk regresses more than
  `BENCH_REGRESS_PCT` (default 25%) on any fixture vs the committed per-machine
  baseline `benchmarks/status/<machine>.csv`.
- Run it whenever checker hot paths change (resolver visitors, rule `check` loops,
  conformance-driven additions). Conformance logic that blows the gate must be
  optimised or restructured.
- `BENCH_NO_GATE=1` (baseline reset) is reserved for fixture-set changes and must
  be justified in the PR description.

### CI Artifact Storage Policy {#GITHUB-NO-ARTIFACTS}

Basilisk is **public**: GitHub-hosted runner compute (all `ubuntu-24.04`) is free and unlimited; GitHub bills for **stored Actions artifacts** (GB-days). Therefore:

- **CI stores no artifacts.** No `actions/upload-artifact` for coverage HTML,
  mutation reports, logs, screenshots. Gates enforce in-job (coverage, mutation
  merge, benchmarks) and reports reproduce locally (`make test`, `make
  mutation-test`). Codecov consumes `lcov.info` directly without GitHub storage.
- **The only permanent store is the GitHub Release.** Release assets (binaries,
  per-platform VSIX) are free and unlimited; never retained Actions artifacts.
- **Transient cross-job hand-offs are the sole exception** (matrix jobs on separate
  runners can't share a filesystem: mutation shards → merge/score; release build
  matrix → publish). Each such upload **must** set `retention-days: 1`; the 90-day
  default is never acceptable.
- **Existing artifacts are purged, not left to expire** when this policy is
  tightened (`gh api repos/<owner>/<repo>/actions/artifacts` → `DELETE …/artifacts/{id}`).

Implemented by `.github/workflows/ci.yml` and `.github/workflows/release.yml` (every
`upload-artifact` carries `retention-days: 1` and a `[GITHUB-NO-ARTIFACTS]`
reference). The Actions **cache** (`Swatinem/rust-cache`, `actions/cache`) is
separate, free, and unaffected.

---

## Migration and Adoption {#CHKARCH-MIGRATION}

### From mypy {#CHKARCH-MIGRATION-MYPY}

1. Run `basilisk migrate --from mypy`
2. Fix BSK-E0001/E0002 errors (missing annotations) -- these are the primary diff
3. Address enums_behaviors+ (Mojo safety) or disable with `mojo-safety = false`

### From Pyright {#CHKARCH-MIGRATION-PYRIGHT}

1. Run `basilisk migrate --from pyright`
2. If you were using Pyright's strict mode: minimal changes needed for core type checking
3. Enable Mojo safety incrementally

### Gradual Adoption {#CHKARCH-MIGRATION-GRADUAL}

1. **Relax per-directory**: soften/disable high-volume rules in `legacy/**` via per-path overrides, keep `src/**` strict
2. **Relax per-file**: `# basilisk: relaxed` at the top demotes a file's errors to warnings
3. **Track progress**: `basilisk stats` shows type completeness percentage
4. **Tighten over time**: remove per-path overrides directory by directory as code is typed

---

## Governance {#CHKARCH-GOVERNANCE}

### License {#CHKARCH-GOVERNANCE-LICENSE}

MIT License. Copyright (c) 2026 NIMBLESITE PTY LTD. No CLA required. No proprietary layers.

### Contribution Model {#CHKARCH-GOVERNANCE-CONTRIB}

- Issues and PRs on GitHub
- RFC process for significant type system changes
- Monthly minor releases, quarterly major releases (semver)

### Relationship to Python Typing Council {#CHKARCH-GOVERNANCE-TYPING}

Basilisk follows the Python Typing Council's governance (PEP 729): implements the typing spec as defined by the council, participates in conformance testing, and never extends the type system in ways that contradict the spec.

---

## Roadmap {#CHKARCH-ROADMAP}

### Phase 1: Foundation {#CHKARCH-ROADMAP-P1}
- Parser (evaluate `ruff_python_parser` vs custom)
- Name resolver
- Basic type checker (50% PEP conformance)
- CLI with human-readable output
- CI pipeline

### Phase 2: LSP and Editors {#CHKARCH-ROADMAP-P2}
- Language server (diagnostics, hover, completions)
- VS Code extension (VSIX)
- Integrated Python debugging via DAP proxy over debugpy (§10.1.1)
- Neovim / Helix configuration

### Phase 3: House Rules and Gradual Adoption {#CHKARCH-ROADMAP-P3}
- All BSK-E0001 through BSK-E0025 rules
- Gradual adoption (per-path / per-file relaxation)
- `basilisk migrate` from mypy/Pyright
- 80% PEP conformance

### Phase 4: Mojo Safety {#CHKARCH-ROADMAP-P4}
- Ownership tracking (BSK-E003x)
- Immutability enforcement (BSK-E004x)
- Structural discipline (BSK-E005x)
- Coercion detection (BSK-E006x)

### Phase 5: Plugin System and Stubs {#CHKARCH-ROADMAP-P5}
- WASM plugin host
- Django, Pydantic, SQLAlchemy plugins
- Auto-stub generation engine
- Stub registry

### Phase 6: Production Hardening {#CHKARCH-ROADMAP-P6}
- 95%+ PEP conformance
- Performance optimization (meet all targets in Section 8.4)
- SARIF/JUnit output
- Enterprise migration playbook

### Phase 7: Ecosystem Growth {#CHKARCH-ROADMAP-P7}
- Plugin marketplace
- Community stub registry
- Conference talks, documentation, tutorials
- PyCharm / IntelliJ plugin maturity

---

## Appendix A: Full PEP Coverage Matrix {#CHKARCH-APPENDIX-PEPS}

| PEP | Title | Priority | Phase |
|---|---|---|---|
| 484 | Type Hints | P0 | 1 |
| 526 | Variable Annotations | P0 | 1 |
| 544 | Protocols | P0 | 1 |
| 585 | Generics in Standard Collections | P0 | 1 |
| 586 | Literal Types | P0 | 3 |
| 589 | TypedDict | P0 | 3 |
| 591 | Final Qualifier | P0 | 3 |
| 604 | Union X \| Y | P0 | 1 |
| 612 | ParamSpec | P1 | 3 |
| 613 | TypeAlias | P0 | 1 |
| 634 | Structural Pattern Matching | P1 | 3 |
| 646 | TypeVarTuple | P1 | 3 |
| 647 | TypeGuard | P0 | 3 |
| 673 | Self Type | P0 | 3 |
| 675 | LiteralString | P1 | 5 |
| 681 | Data Class Transforms | P1 | 5 |
| 692 | TypedDict **kwargs | P1 | 5 |
| 695 | Type Parameter Syntax | P0 | 3 |
| 696 | TypeVar Defaults | P1 | 5 |
| 698 | Override Decorator | P0 | 3 |
| 702 | Deprecated Decorator | P1 | 5 |
| 742 | TypeIs | P0 | 3 |

## Appendix B: Glossary {#CHKARCH-APPENDIX-GLOSSARY}

| Term | Definition |
|---|---|
| **Basilisk** | This project — a configuration-driven Python type checker in Rust; default config is pure PEP conformance, house-style rules opt-in. |
| **Borrowed** | Parameter convention: function reads but does not mutate or transfer the value (default) |
| **Owned** | Parameter convention: function takes exclusive ownership; caller must not use value afterward |
| **InOut** | Parameter convention: function may mutate the value in place |
| **Default configuration** | Basilisk has no modes (no basic/standard/strict). The default config enables every PEP typing-spec rule and nothing else; house-style rules are opt-in via configuration ([CHKARCH-CONFIGURATION-ONLY](#CHKARCH-CONFIGURATION-ONLY)) |
| **Mojo safety** | The set of ownership, immutability, and coercion rules inspired by the Mojo language |
| **Type completeness** | Percentage of symbols in a module/project with resolved (non-Any) types |
