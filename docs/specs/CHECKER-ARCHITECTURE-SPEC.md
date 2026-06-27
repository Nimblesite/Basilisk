# Basilisk: Complete Type Safety for Python {#CHKARCH}

**Version**: 0.1.0-draft
**Status**: Specification Draft
**License**: MIT

---

## Vision and Philosophy {#CHKARCH-VISION}

### The Problem {#CHKARCH-PROBLEM}

Python has a type system. Nobody uses it properly.

Type hints are widespread, but most projects never enforce them. Every mainstream type checker defaults to gradual typing -- untyped code passes silently. The result: type annotations are documentation, not contracts. They rot. They lie. They give false confidence.

The Python ecosystem has no equivalent of TypeScript. No tool exists that says: **"This code is not typed. It does not compile."**

Basilisk is that tool.

### Design Thesis {#CHKARCH-THESIS}

Basilisk treats Python as a statically typed language. It is to Python what TypeScript is to JavaScript -- a strict, typed superset that enforces contracts at analysis time.

- Every function parameter has a type.
- Every return type is declared.
- Every variable assignment resolves to a known type.
- `Any` is an explicit escape hatch, never an implicit default.

There is no "basic" mode. There is no "standard" mode. There is no "strict" mode either. There is no `--permissive` flag and no `--strict` flag. The type system is the product, and **everything Basilisk reports is decided by configuration alone** — a flat set of per-rule severities, not a dial you switch between. Escape hatches exist for pragmatism, but the burden is on the developer to justify the exception.

Rust does not have a flag that disables the borrow checker, and Basilisk does not have a "strictness" dial. What it has is configuration: the **default configuration enables every PEP typing-spec rule and nothing else**, so a fresh project is measured as pure PEP conformance, and the opinionated house-style rules are opt-in from there.

### No "strict mode" — behaviour is configuration only {#CHKARCH-CONFIGURATION-ONLY}

Basilisk has **no modes**. There is no "strict mode", no "basic" or "standard" mode, no `--strict`, no `--permissive`. Other checkers ship a discrete dial — pyright's `off` / `basic` / `standard` / `strict`; Basilisk deliberately does not. Everything Basilisk reports is decided by **configuration alone**: a flat set of per-rule severities a project sets globally, per path, or per file.

Two consequences follow, and both are load-bearing:

1. **The default configuration is pure PEP conformance.** With no config file, Basilisk enables **every rule that implements the Python typing specification, and nothing else**. A fresh project is therefore measured purely against the PEP typing spec. This genuine, unconfigured default is exactly what the conformance scorer runs — no `basilisk.json`, no special "conformance mode" ([CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)). Every PEP rule is on; the score is the real out-of-the-box experience.

2. **Everything beyond the spec is opt-in configuration.** Basilisk's opinionated house-style rules — require-an-annotation (`BSK-E0001`/`BSK-E0002`/`BSK-E0004`), require-`@override` (`BSK-E0025`), redundant-annotation (`BSK-W0050`), the explicit-`Any` nudge (`BSK-W0014`), uv dependency hygiene, and stub suggestions — are **off by default**. A project that wants them turns them on in configuration (`strict_annotations = true`, `uv_dependency_diagnostics = true`, …). They are never enabled implicitly and never by a "mode".

Basilisk's *opinion* is still that you should type everything — the house rules encode that recommendation — but acting on it is a **configuration a project chooses**, not a baked-in mode and never a precondition of the conformance score. "Strict" is a property of a chosen configuration, not a switch in the product. The anti-gaming rule is unchanged: no PEP rule may be disabled, deleted, or unregistered to move the conformance number ([CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)).

### Project Principles {#CHKARCH-PRINCIPLES}

1. **Configuration over modes** -- behaviour is per-rule configuration, never a mode; the default is pure PEP conformance, strictness is opt-in, escape hatches by choice ([CHKARCH-CONFIGURATION-ONLY](#CHKARCH-CONFIGURATION-ONLY))
2. **Every error must teach** -- Diagnostics explain why, not just what
3. **Don't reinvent wheels** -- Depend on quality open-source tools (Ruff, ty, typeshed) for everything we can
4. **Performance is a feature** -- Sub-10ms incremental checks or it's broken
5. **Open source means open governance** -- No proprietary layers, no vendor lock-in
6. **First-class developer experience** -- VS Code extensions, LSP, CLI -- everything works out of the box

---

## Ecosystem Gap Analysis {#CHKARCH-GAP}

See the project README for competitive analysis.

### Capability Matrix {#CHKARCH-MATRIX}

| Capability | Pyright | mypy | ty | Pyrefly | Zuban | Ruff | **Basilisk** |
|---|---|---|---|---|---|---|---|
| Implementation | TypeScript | Python/C | Rust | Rust | Rust | Rust | **Rust** |
| License | MIT | MIT | MIT | MIT | AGPL | MIT | **MIT** |
| Default strictness | Gradual | Gradual | Gradual | Gradual | Gradual | N/A | **PEP by default; strict opt-in** |
| PEP conformance (current) | [live results][cf] | [cf] | [cf] | [cf] | [cf] | N/A | **46.6%** (self-measured) |
| PEP conformance target | — | — | — | — | — | N/A | **100%** |
| LSP server | Yes | No | Yes | Yes | Yes | No | **Yes** |
| Incremental computation | Lazy eval | Daemon | Salsa | Module-level | No | N/A | **Salsa** |
| Ownership analysis | No | No | No | No | No | No | **Yes** |
| Immutability enforcement | No | No | No | No | No | No | **Yes** |
| Implicit coercion detection | No | No | No | No | No | No | **Yes** |
| Linting | No | No | No | No | No | **Yes** | Delegates to Ruff |
| Formatting | No | No | No | No | No | **Yes** | Delegates to Ruff |
| Plugin system | No | Python hooks | Planned | No | No | No | **WASM plugins** |
| Auto-stub generation | No | stubgen (basic) | No | Inference | No | No | **Tiered stubs** |
| CI output (SARIF/JUnit) | Limited | No | No | No | No | No | **SARIF + JUnit** |
| Multi-threaded | No | No | Yes | Yes | No | Yes | **Yes** |
| Migration tooling | N/A | N/A | No | No | No | N/A | **mypy + Pyright import** |
| VS Code extension | Pylance (proprietary) | No | Yes | Yes | Yes | Yes | **Yes (open source)** |
| No Microsoft dependency | No (Node.js) | Yes | Yes | Yes | Yes | Yes | **Yes** |

> Rival conformance figures move as those tools evolve, so rather than freeze (and inevitably misstate) them here, the rival cells link to the official, continuously-updated scoreboard. Basilisk's **46.6%** is self-measured by that same suite's calculator run over the unmodified binary in its default config ([CHKARCH-CONFORMANCE](#CHKARCH-CONFORMANCE)); it is not directly comparable to numbers produced under a different methodology or grading.

[cf]: https://github.com/python/typing/blob/main/conformance/results/results.html

---

## Dependency Strategy {#CHKARCH-DEPS}

Basilisk does not reinvent wheels. We depend on quality open-source tools for everything we can.

### Direct Dependencies {#CHKARCH-DEPS-DIRECT}

| Dependency | Purpose | License | Rationale |
|---|---|---|---|
| **Ruff** (`ruff` CLI) | Linting + formatting | MIT | Best-in-class. 700+ rules. We don't recreate lint or format. |
| **`ruff_python_parser`** | Python AST parsing | MIT | Battle-tested Rust crate. Powers Ruff. Evaluate as our parser. |
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
| **Ruff** | Basilisk invokes `ruff check` and `ruff format` as subprocesses or links the Ruff crates directly. Configuration unified in `pyproject.toml`. |
| **typeshed** | Bundled copy of typeshed stubs, updated with each Basilisk release. Users can override with custom stubs. |
| **mypy config** | `basilisk migrate --from mypy` reads `mypy.ini` / `setup.cfg` and produces `[tool.basilisk]` config. |
| **Pyright config** | `basilisk migrate --from pyright` reads `pyrightconfig.json` and produces `[tool.basilisk]` config. |
| **PEP 561** | Full support for `py.typed` packages, inline type annotations, and stub-only packages. |

---

## Core Type System {#CHKARCH-TYPESYS}

### Strictness Model {#CHKARCH-STRICTNESS}

#### No Modes — Configuration Decides Everything {#CHKARCH-STRICTNESS-ONLY}

Basilisk has **no modes** — no "strict mode", no basic/standard dial. Behaviour is configuration. The default configuration is pure PEP conformance; the example below shows the require-annotation house rules (`BSK-E0001`/`BSK-E0002`) that fire **only once a project enables them in configuration** ([CHKARCH-CONFIGURATION-ONLY](#CHKARCH-CONFIGURATION-ONLY)). Under the default config these snippets are accepted.

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

There is no `--basic`, `--standard`, `--strict`, or `--permissive` flag — Basilisk has no modes. The behaviour above is configuration: enable the require-annotation house rules and every function parameter must be annotated, every function must declare its return type, and every variable assigned from an untyped source must carry an explicit annotation. Leave them off — the default — and the same code is accepted as pure PEP conformance.

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

The default mode for each rule is determined by its code prefix (`E` = error, `W` = warning). All modes can be overridden at every level: per-line, per-block, per-file, and per-project.

#### Inline Suppression and Mode Override {#CHKARCH-STRICTNESS-SUPPRESSION}

Basilisk supports both standard `# type: ignore` (for compatibility with mypy/Pyright) and its own ergonomic comment directives.

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
# Basilisk has no "strict"/"mode" switch. The default configuration is pure PEP
# conformance; opt into house-style rules explicitly, by name:
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

Basilisk recognizes these comment formats for maximum interop:

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

The `# type:` prefix ensures compatibility with editors and tools that already recognize `# type: ignore`. Other type checkers will treat `# type: warning` as an unknown directive and ignore it gracefully.

### Python Typing PEP Coverage {#CHKARCH-PEPS}

Basilisk's **target** is 100% conformance with the Python typing specification. Today the official `python/typing` conformance scorer (pinned commit, run unmodified in CI, **with every rule enabled** — no spec-conformance mode, see [CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)) reports **68 of 146 files passing (46.6%, counting errors and warnings — the strictest grading)**, with **265 false positives** and **0 missed required errors**. The checker catches every required error; the gap is entirely strict-by-default house rules firing on spec-valid code. We run that suite in CI on every change; the gate ratchets the pass-percentage **up** and the false-positive ceiling **down** — closed only by fixing the checker, never by disabling a rule.

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

Basilisk enforces annotations on public APIs but infers types for local variables:

```python
def process(items: list[str]) -> int:
    count = 0              # inferred: int (from literal)
    filtered = [x for x in items if x.startswith("a")]  # inferred: list[str]
    count = len(filtered)  # OK: int = int
    return count
```

**Rules**:
- **Public APIs** (module-level functions, class methods, module-level variables): explicit annotations required
- **Local variables**: types inferred from assignments, comprehensions, and control flow
- **Cross-module inference**: does NOT cross module boundaries for public symbols. Imports from typed modules resolve to declared types. Imports from untyped modules produce `imports_unresolved`.

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
> This section is a forward-looking design for the `basilisk-mojo` crate, which is
> a stub and is **not wired into the analysis pipeline**. The `generics_defaults`–`specialtypes_never`
> codes referenced below are **illustrative of the planned design only** — those same
> numeric codes are currently used by shipping PEP-typing rules (see the
> [complete diagnostic reference](#CHKARCH-DIAG-REFERENCE) for what each code actually
> does today). Do not treat the examples in this section as current behaviour.

When implemented, these are **opt-in** rules in the `basilisk-mojo` crate — off by default like every non-PEP house rule, and enabled only when a project turns them on in configuration ([CHKARCH-CONFIGURATION-ONLY](#CHKARCH-CONFIGURATION-ONLY)). They adapt Mojo's ownership, immutability, and coercion concepts as static analysis over standard Python using `typing.Annotated`, decorators, and `dataclass(frozen=True)`; no Mojo code or runtime is required.

### Ownership and Lifetime Tracking {#CHKARCH-MOJO-OWNERSHIP}

Basilisk introduces optional ownership annotations using Python's existing `typing.Annotated` mechanism:

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
    return consumed           # OK: owned value returned

items = [1, 2, 3]
temp = [4, 5]
buf: list[int] = []

result = process(data=items, buffer=buf, consumed=temp)
print(temp)  # ERROR: use after ownership transfer [directives_cast]
print(buf)   # OK: InOut reference still valid
```

**Static analysis rules**:
- `generics_defaults`: Mutation of `Borrowed` parameter
- `directives_cast`: Use-after-move (value used after `Owned` transfer)
- `typeddicts_class_syntax_2`: Implicit copy of large structure (suggest explicit `.copy()`)
- `BSK-W0033`: Missing ownership annotation on mutable parameter (suggestion)

### Immutability by Default {#CHKARCH-MOJO-IMMUTABLE}

Function parameters are treated as immutable by default. Mutation of a parameter produces a diagnostic unless annotated with `InOut`:

```python
def bad(items: list[int]) -> None:
    items.append(1)  # ERROR: mutation of parameter [enums_behaviors]
    items = [1, 2]   # ERROR: reassignment of parameter [calls_argument_count]

def good(items: Annotated[list[int], InOut]) -> None:
    items.append(1)  # OK: explicitly mutable
```

**Interaction with dataclasses**:
```python
from dataclasses import dataclass

@dataclass  # WARNING: prefer frozen=True [BSK-W0042]
class Point:
    x: float
    y: float

@dataclass(frozen=True)  # OK: immutable by default
class Point:
    x: float
    y: float
```

### Structural Discipline {#CHKARCH-MOJO-STRUCTURAL}

```python
class Config:
    host: str
    port: int

    def __init__(self, host: str, port: int) -> None:
        self.host = host
        self.port = port

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
x: float = float(1)  # OK: explicit conversion

y: int = True        # ERROR: implicit bool-to-int coercion [enums_expansion]
y: int = int(True)   # OK: explicit conversion

z: str = b"hello"    # ERROR: implicit bytes-to-str [specialtypes_never]
```

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

Inspired by `rustc`'s diagnostic system and ty's approach.

### Error Code System {#CHKARCH-DIAG-CODES}

Format: `BSK-Xnnnn` where X = default severity class:
- `E` = Error (blocks CI by default)
- `W` = Warning (does not block by default)
- `I` = Info (suggestion by default)

The prefix determines the **default** severity. Every rule can be overridden to any of the four modes (`error`, `warning`, `info`, `disabled`) at every scope level (line, block, file, path, global). See Section 4.1.3 for the mode system and Section 4.1.4 for override syntax.

### Rule Categories {#CHKARCH-DIAG-CATEGORIES}

> **Classification is by tags, not categories.** The authoritative way Basilisk
> classifies rules is the tagging system — provenance tags (`pep`/`basilisk`),
> PEP-category tags (PEP rules only), and free-form tags. The code-range groupings
> below are a coarse legacy convenience for the reference table; the source of
> truth is [Rule Tagging](CHECKER-RULE-TAGGING-SPEC.md#CHKTAG) ([CHKTAG]).

#### Missing Annotations (BSK-E0001 -- BSK-E0009) {#CHKARCH-DIAG-MISSING}

| Code | Description |
|---|---|
| BSK-E0001 | Missing parameter type annotation |
| BSK-E0002 | Missing return type annotation |
| BSK-E0003 | Missing variable type (unresolvable inference) |
| BSK-E0004 | Missing `*args` / `**kwargs` type annotation |
| BSK-E0005 | Missing class attribute type annotation |

#### Type Safety (imports_unresolved -- typeddicts_class_syntax) {#CHKARCH-DIAG-TYPESAFETY}

| Code | Description |
|---|---|
| imports_unresolved | Unresolved import |
| returns_compatibility | Return type mismatch |
| calls_argument_type | Argument type mismatch |
| returns_compatibility_2 | Return type mismatch |
| assignment_compatibility | Assignment type incompatibility |
| callables_annotation | Invalid type argument |
| classes_override | Incompatible method override |
| classes_override_2 | Incompatible variable override |
| names_undefined | Undefined variable |
| names_unbound | Unbound variable (some code paths) |
| overloads_definitions | Missing overload implementation |
| overloads_consistency | Overlapping overloads with incompatible returns |
| dict_key_hashable | Unhashable type in hash-requiring context |
| match_exhaustiveness | Non-exhaustive pattern match |
| annotations_typeexpr | Invalid type form in annotation |
| BSK-E0025 | Missing `@override` decorator |
| generics_basic | `TypeVar` declared with a single constraint |
| generics_base_class | Duplicate `TypeVar` in a `Generic[...]` base |
| typeddicts_class_syntax | Method defined inside a `TypedDict` class |

#### Complete diagnostic reference {#CHKARCH-DIAG-REFERENCE}

The full set of codes the checker currently emits. This table is generated from
the rule source by `scripts/gen_rules_reference.py` and is the authoritative
list — keep it in sync after adding or renaming a rule.

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
| `BSK-W0040` | Lambda function missing type annotations |
| `BSK-W0050` | Redundant type annotation warning |

#### Constructor-to-callable conversion {#CHKARCH-DIAG-CTOR-CALLABLE}

`constructors_callable` implements the typing-spec rule
["Converting a constructor to callable"](https://typing.readthedocs.io/en/latest/spec/constructors.html#converting-a-constructor-to-callable).
When a class object flows through an identity-over-callable function
(`def f(cb: Callable[P, R]) -> Callable[P, R]`), the value it returns gains the
class's *constructor-to-callable* signature. Calls to a variable bound that way
are validated against the synthesized signature.

The synthesized signature is derived in priority order:

1. The metaclass `__call__` (when the class declares a metaclass that defines
   `__call__`) — e.g. a `__call__` taking `*args, **kwargs` accepts any call.
2. `__new__` when its return type is neither `Self` nor the class itself
   (e.g. `-> int`, `-> Proxy`, `-> Any`); `__init__` is then ignored.
3. Otherwise `__init__` (or `__new__` when no `__init__` exists); a class with
   neither synthesizes a zero-argument callable returning the instance.

`constructors_callable` fires when a call to such a variable supplies too few or too many
positional arguments, names a keyword that is not a parameter, or binds a
function-scoped `TypeVar` inconsistently (e.g. `list[T]` filled by both
`list[int]` and `list[str]`). The analysis is conservative: starred positional
arguments and `**kwargs` unpacking suppress arity checks to avoid false
positives. Implemented in `crates/basilisk-checker/src/rules/e0153.rs`; tests in
`crates/basilisk-checker/tests/e0153_tests.rs`.

#### Strict local-stub member access {#CHKARCH-DIAG-STUB-MEMBER}

`imports_module_attribute` makes a **user/local stub authoritative**: when `import X` resolves
to a `.pyi` under a configured `stub-paths` directory (including the
auto-discovered `.basilisk/stubs/` that the "Create local type stub" quick fix
writes — see [STUBRES-CREATE-LOCAL](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CREATE-LOCAL)),
accessing `X.attr` where the stub declares neither `attr` nor a module-level
`def __getattr__` is a hard error. This is the counterpart
that makes a hand-written or generated stub *mean something* — declare what you
use, or it is flagged.

The `def __getattr__(name: str) -> Any: ...` that the create-local skeleton ships
by default is the **explicit opt-out**: keep it and every attribute is permitted
(the module stays `Any`); remove it and declare specific symbols to opt into
checked member access.

Scope (Phase 1): only plain, single-segment `import X` backed by a user stub.
The member API is captured during import resolution
(`crates/basilisk-lsp/src/import_resolver.rs`, on both the CLI and LSP paths) and
carried on `ResolvedModule.imported_modules`. Because that map is populated *only*
for user stubs, the rule is a complete no-op for code without local stubs (the
conformance suite, first-party code) — the false-positive surface is zero by
construction. Third-party typeshed / `py.typed` packages, instance/class
attribute access, and dotted/aliased imports are deferred follow-ups. Implemented
in `crates/basilisk-checker/src/rules/e0154/`; tests in
`crates/basilisk-checker/src/rules/e0154/tests.rs`.

#### TypedDict `extra_items` / `closed` (PEP 728) {#CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS}

`typeddicts_extra_items` implements [PEP 728](https://peps.python.org/pep-0728/) — the
`extra_items=` and `closed=` class keywords on `TypedDict`. A TypedDict that
specifies `extra_items=T` defines an infinite set of non-required (or, when `T`
is `ReadOnly[...]`, read-only) extra items whose value type is `T`; `closed=True`
forbids any extra items at all. The rule validates four families of usage,
operating directly on the module AST so it is independent of resolver state:

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
`ClassInfo::is_typed_dict` is only `true` for classes that name `TypedDict`
*directly*, so a subclass (`class Album(NamedDict): ...`) was invisible to every
`TypedDict` rule. The shared helpers in
`crates/basilisk-resolver/src/scope/typeddict_meta.rs`
(`is_transitive_typeddict`, `has_extra_items_transitive`,
`transitive_typeddict_names`, `strip_typeddict_qualifiers`) and the effective
field-merge in `crates/basilisk-resolver/src/visitor/typeddict_schema.rs`
(`effective_fields`) compute each `TypedDict`'s full schema (own + inherited
fields, most-derived declaration winning, carrying the field's `ReadOnly`
qualifier and required-ness). Recognising transitive subclasses also cleared the
E0014 dict-literal false positives across the read-only suite.

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
`benchmarks/fixtures/e0038_typeddict_readonly_inheritance.py`.

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

`ruff_python_parser` is a recursive-descent parser, and the resolver and checker
walk the resulting AST recursively (as does the AST's own `Drop`). All three
overflow the thread stack on pathologically nested input: a bracket expression
nested past roughly 4 000 levels aborts the process with `SIGABRT`. On the
language server this manifested as a crash-restart loop the moment a workspace
containing such a file was scanned. (A single 20 000-deep parenthesised
expression in a generated file is the canonical trigger.)

To stay crash-safe — and to match CPython, which rejects this input at the
*tokenizer* rather than crashing — `parse_source` (the workspace's single entry
point into `ruff_python_parser`) runs a nesting-depth guard **before** handing
the source to the recursive parser. The guard:

- Measures depth with ruff's **linear lexer** (`lex` + `next_token`), a flat byte
  scan that never recurses, so the measurement itself can never overflow. It
  short-circuits at the first violating token, so a pathological file is only
  lexed up to the offending bracket/indent.
- Rejects **bracket nesting** (`(`, `[`, `{`, cumulative) deeper than **200**,
  matching CPython's tokenizer `MAXLEVEL`; the message is CPython's verbatim
  `too many nested parentheses`.
- Rejects **indentation** deeper than **99 levels**, matching CPython's
  `MAXINDENT`; the message is CPython's verbatim `too many levels of
  indentation`.

Both limits sit one to two orders of magnitude below the ~4 000 stack-overflow
floor and far above any non-pathological source (real code rarely nests beyond
~15 brackets / ~10 indents), so the guard is crash-proof without false
positives. The rejection surfaces as a `ParseError::Syntax` and follows the
existing parse-error path (`BSK-PARSE` in the LSP).

**Known residual (out of scope for this tokenizer-level guard):** an extremely
long *un-bracketed* expression — e.g. a 30 000-term `1+1+...` operator chain or a
deeply nested ternary/`lambda` — parses successfully in ruff but produces an AST
deep enough to overflow on any later recursive traversal (resolver, checker, or
`Drop`). This is the class CPython itself bounds only at its *parser* C-stack
guard (a build-dependent `MemoryError`), not the tokenizer, and a complete fix
would require iterative AST traversal/teardown rather than a parse-time check.
Such input does not occur in real or generated code (deep generated data is
bracketed, and is covered above).

Implemented in `crates/basilisk-parser/src/depth.rs` and `…/src/lib.rs`
(`parse_source`); covered by `crates/basilisk-parser/tests/parse_tests.rs` and
the propagation test in `crates/basilisk-checker/tests/checker_tests.rs`.

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

Every binary crate that exposes a Shipwright `--version` payload
(`basilisk-cli`, `basilisk-profiler-helper`) must stamp the same
`SHIPWRIGHT_*` env vars (git SHA, a guaranteed `SHIPWRIGHT_GIT_DIRTY`, build
time, target, toolchain) at compile time. That logic lives once in the
`basilisk-buildinfo` crate (`emit_version_env`), so each crate's `build.rs` is
a one-line delegation rather than a copy. The calendar arithmetic that formats
`SHIPWRIGHT_BUILD_TIME` is the same RFC 3339 formatter the profiler uses for
sample timestamps; it lives in `basilisk_common::datetime::rfc3339_from_secs`
so the Howard Hinnant `civil_from_days` algorithm exists in exactly one place.

---

## Incremental Computation {#CHKARCH-INCREMENTAL}

### Salsa Architecture {#CHKARCH-INCREMENTAL-SALSA}

Basilisk uses the Salsa incremental computation framework (the same system powering rust-analyzer).

**Input queries**: Source file contents, configuration, stub files
**Derived queries**: Parsed ASTs, resolved names, type assignments, diagnostics

When a source file changes, only queries that depend on the changed input are recomputed. The dependency graph is tracked automatically by Salsa.

### Cancellation {#CHKARCH-INCREMENTAL-CANCEL}

When a new keystroke arrives while a check is in progress, the current computation is cancelled and restarted with the new input. This is critical for responsive IDE experience.

### Persistent Cache {#CHKARCH-INCREMENTAL-CACHE}

Disk-backed cache between sessions. On startup, Basilisk loads the cache and only recomputes files that changed since last run. This eliminates cold-start latency for repeat sessions.

### Performance Targets {#CHKARCH-INCREMENTAL-PERF}

| Scenario | Target |
|---|---|
| Cold start, 100K LOC | < 5 seconds |
| Cold start, 1M LOC | < 30 seconds |
| Incremental (single file edit) | < 10ms |
| Memory, 1M LOC | < 2 GB |

---

## Language Server Protocol {#CHKARCH-LSP}

### LSP-First Design {#CHKARCH-LSP-FIRST}

Basilisk is an LSP server first, CLI tool second. The LSP server is the primary product. The CLI is a batch-mode wrapper around the same engine. This ensures interactive and CI experiences are always consistent.

> For the complete LSP specification — all 21 features, custom commands, configuration settings, binary resolution, DAP integration, and DapTcpProxy — see **[LSP-ARCHITECTURE-SPEC.md](LSP-ARCHITECTURE-SPEC.md)**.

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

All editors connect to the same `basilisk lsp` binary via stdio. The LSP server is the single backend — editor extensions are thin integration layers.

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

Basilisk includes a stub generation engine with three modes:

1. **Runtime introspection**: Import the package, inspect objects, generate `.pyi` files
2. **AST-based inference**: Parse package source, infer signatures without importing
3. **Hybrid**: Combine both, preferring runtime data with AST fallback

### Stub Quality Tiers {#CHKARCH-STUBS-TIERS}

| Tier | Source | Trust Level | Diagnostic Behavior |
|---|---|---|---|
| Tier 1 | Hand-written, verified, typeshed | High | No warnings |
| Tier 2 | Auto-generated, community reviewed | Medium | Info notes on potential inaccuracies |
| Tier 3 | Best-effort inference | Low | Warnings that types may be incomplete |

### typeshed Compatibility {#CHKARCH-STUBS-TYPESHED}

Basilisk bundles a copy of typeshed and uses it as the Tier 1 baseline for standard library stubs. Users can override with custom stubs via `stubPaths` configuration.

---

## Plugin and Extension System {#CHKARCH-PLUGINS}

### Architecture {#CHKARCH-PLUGINS-ARCH}

**WASM-based** for security and portability:
- Plugins compiled to WebAssembly
- Sandboxed execution (no filesystem, no network)
- Receive AST nodes and type information
- Return diagnostics and code actions

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
stub-paths = ["stubs/"]
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

`include` lists the roots scanned when no paths are given on the CLI
(`basilisk check` with no arguments). Explicit CLI paths always override it;
`exclude` applies within the include roots. When `include` is absent or empty,
the current directory is scanned. Entries are resolved relative to the
directory of the configuration file. This keeps vendored or generated trees
the user excluded by omission out of the walk entirely (issue #37).

The **LSP** honors the same `include` roots on both paths, so the editor
analyses exactly the files `basilisk check` would. The bulk scan walks only the
include roots (`WorkspaceIndex::scan_dirs_for`), and the per-file/open path
suppresses diagnostics for any file outside them
(`WorkspaceIndex::is_outside_include_roots`, applied in `analyse_and_resolve` and
`recheck_all_files`) — so a file in a generated tree shows no diagnostics even
when opened, exactly like an `exclude`d file.

### Exclude Semantics {#CHKARCH-CONFIG-EXCLUDE}

`exclude` (and the `per-path-overrides` keys) use **gitignore-style globs**,
matched against the path relative to the workspace root:

- a bare name with no `/` matches that segment at **any** depth — `build`
  excludes every `build` directory in the tree, `*.pb.py` every generated file;
- `**` matches zero or more directory segments, so `**/bundled/**` matches a
  `bundled` directory anywhere; `*` / `?` match within a single segment only;
- an anchored pattern (one containing `/`) matches the full path or any of its
  ancestor directories, so a directory pattern (`vendor/**`, `src/generated`)
  also excludes everything beneath it.

A baseline set of vendored / cache directories is **always** excluded (e.g.
`node_modules`, `site-packages`, `.venv`, `__pycache__`, `build`, `dist`, and
the extension's vendored `bundled` / `_vendored` trees); user `exclude` entries
extend this set. Hidden directories (names starting with `.`) are always
skipped. The single canonical matcher is `basilisk_config::path_matches_pattern`,
shared by every entry point so they all exclude exactly the same files:

- the LSP **workspace scan** (`workspace_scan::is_excluded`),
- the `basilisk check`/`fix`/`adopt` **CLI walk** (`is_excluded_path`), and
- the LSP **incremental per-file path** (`WorkspaceIndex::is_path_excluded`,
  applied in `analyse_and_resolve`) — so a vendored file that is *opened* or
  *edited* in the editor is parsed for navigation but publishes **no**
  diagnostics, matching the bulk scan rather than squiggling every line.

### Migration from Existing Tools {#CHKARCH-CONFIG-MIGRATION}

```bash
basilisk migrate --from pyright   # Reads pyrightconfig.json -> pyproject.toml
basilisk migrate --from mypy      # Reads mypy.ini / setup.cfg -> pyproject.toml
```

Semantic mapping:
- Pyright `strict` mode -> Basilisk with the house-style rules enabled in configuration (require-annotation, explicit-`Any`, …), Mojo safety disabled
- Pyright `standard` mode -> Basilisk's PEP-only default plus selected house rules, softened in `per-path-overrides` where needed
- mypy `--strict` -> Basilisk with the house-style rules enabled in configuration, Mojo safety disabled

---

## Diagnostics Experience {#CHKARCH-DIAGEXP}

### Quality Standard {#CHKARCH-DIAGEXP-QUALITY}

Every diagnostic follows the rustc standard:

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

Benchmark suite against real-world codebases:
- **PyTorch** (~600K LOC)
- **Django** (~250K LOC)
- **FastAPI** (~30K LOC)
- **Python standard library** (~500K LOC)

Comparison baselines: Pyright, ty, Pyrefly, Zuban.

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
| Benchmarks | `make bench` (hyperfine, `benchmarks/run.sh`) vs Pyright/mypy/ty/Pyrefly | Performance tracking + regression gate (fails if basilisk regresses >25% vs the committed per-machine `benchmarks/status/<machine>.csv`) |

### PEP Conformance Scoring {#CHKARCH-CONFORMANCE}

The conformance score is computed by the **real `python/typing` conformance
calculator**, not a Basilisk reimplementation. This is non-negotiable: the
number must be one anyone can reproduce with the same tooling the reference
checkers (pyright, mypy, pyrefly, ty, zuban, pycroscope) are graded with.

> ⛔️ **DISABLING, DELETING, OR UNREGISTERING ANY CONFORMANCE RULE IS ABSOLUTELY
> FORBIDDEN.** The binary is scored in its **full, default, strict-by-default
> configuration with EVERY rule enabled** — no `basilisk.json`, no per-rule
> override, no "spec-conformance mode", no skipped fixtures, **no deleting rule
> source files (`src/rules/*.rs`), no removing rules from `all_rules()`**, no
> exceptions, no matter what. `score.py` deletes any `basilisk.json` from the
> fixtures directory before scoring so config cannot silence a rule — but note
> that guard does **not** stop someone from deleting the rules themselves, which
> is the **same crime by another route** and is forbidden just as absolutely.
> Equally forbidden: hand-editing `conformance/conformance_status.csv` or
> loosening the `coverage-thresholds.json` gate (`threshold` /
> `max_false_positives`) to match a faked run. The number is exactly what a real
> user gets out of the box. If a strict default fires on valid type-system code,
> that is a **real conformance gap to FIX in the checker** — never to hide by
> turning a rule off, deleting it, or editing the scoreboard. Gaming the number
> in any of these ways is a punishable offence.

- **Scorer**: [`conformance/score.py`](../../conformance/score.py) **imports the
  committed [`conformance/upstream_main.py`](../../conformance/upstream_main.py)** —
  a byte-identical, sha256-verified copy of `python/typing`'s
  `conformance/src/main.py`, pinned to the same commit the fixtures come from
  (`score.py` → `PINNED_TYPING_REF`, currently `268d0c4e`, sha256
  `b4e3bd08…0fc6a2`) — and calls its own `get_expected_errors` +
  `diff_expected_errors` functions **unmodified**. Nothing is downloaded at score
  time; the verbatim upstream file lives in the repo and `score.py` refuses to run
  if its hash drifts. Refresh it only when bumping the ref:
  `python3 conformance/score.py --refresh-upstream`. The only Basilisk-specific
  code is a checker *adapter* that runs the real `basilisk` binary and turns its
  JSON output into the `{line: [errors]}` mapping the upstream algorithm consumes —
  exactly the role of upstream's per-checker adapters in `type_checker.py`.
- **Pass rule** (upstream's, verbatim): a file passes iff the upstream
  `errors_diff` is empty — every `# E` line gets an error, every `# E[tag]`
  group is satisfied, and **no error lands on a line the suite does not mark**.
  `conformance_automated = "Fail" if errors_diff.strip() else "Pass"`.
- **Nothing excluded from scoring.** The scorer counts **every** diagnostic the
  binary emits — errors **and** warnings, the strictest grading and how pyright is
  graded. `score.py` applies this single grading on every run; there is no looser
  mode and no opt-out flag, so every run produces the same canonical figure.
  One firing on an unannotated line is a real false positive and fails the file —
  same as for any other checker. **Nothing is configured on the binary either:**
  it runs with **every rule enabled** in its default strict-by-default mode, and
  `score.py` deletes any stale `basilisk.json` before scoring so no rule can be
  silenced ([CHKARCH-CONFORMANCE-MODE](#CHKARCH-CONFORMANCE-MODE)).
- **Gate**: `make test` (via [`scripts/test-rust.sh`](../../scripts/test-rust.sh))
  builds the `basilisk` binary, then runs `python3 conformance/score.py --gate`
  on it — there is **no Rust conformance test**; the whole conformance system is
  the two committed Python files plus the git-ignored downloaded fixtures under
  `conformance/tests/`. The pass-percentage floor and false-positive ceiling live
  in `coverage-thresholds.json` (`conformance.threshold`,
  `conformance.max_false_positives`); the former ratchets **up**, the latter
  **down**. Per-file results are written to `conformance/conformance_status.csv`.
- **Current score**: **68 / 146 = 46.6%** (strictest grading: every diagnostic,
  errors AND warnings, counted — as pyright is graded), **265 false positives**, **0
  missed required errors**, binary run with **every rule enabled**. The checker still
  catches all **955** required errors; every failing file fails on a *false positive*
  — a strict-by-default house rule firing on valid type-system code — never on a
  missed error. **Baseline reset (2026-06-26):** a prior version disabled six
  house-style rules (E0001/E0002/E0004/E0025/W0014/W0050) before scoring and
  reported a **fake 100%**. Running the binary the way a real user does — all rules
  on — the honest figure is 46.6%. This is the one-time correction of that gamed
  baseline; from here the pass-percentage ratchets **up** and the FP ceiling **down**,
  driven only by genuinely fixing the checker, **never** by disabling a rule again.
  Target: **100%**.

#### No "spec-conformance mode" — every rule runs {#CHKARCH-CONFORMANCE-MODE}

There is **no** conformance mode, and there never will be. Basilisk is
**strict-by-default**: on top of the type system it ships opinionated *house-style*
rules the typing spec does not define (require-annotations `BSK-E0001`/`BSK-E0002`/
`BSK-E0004`, require-`@override` `BSK-E0025`, redundant-annotation `BSK-W0050`, the
explicit-`Any` nudge `BSK-W0014`). On the PEP suite these fire on valid type-system
code, so they cost us conformance points.

⛔️ **We pay that cost honestly. Disabling, deleting, or unregistering any rule for
conformance is forbidden.** This has been attempted twice. First, a revision wrote a
`basilisk.json` that turned six of these rules off before scoring and reported a
**fake 100%**; that was removed, and `score.py` now *deletes* any `basilisk.json`
from the fixtures directory before scoring (`purge_rule_config`). Second — when
config-disabling was blocked — a revision tried to *delete the offending rule source
files outright* and unregister them from `all_rules()`, then re-report a **fake
100%**: the same lie, dressed up as a "milestone," because "every rule is enabled"
reads as true once the rules no longer exist. **Deleting a rule to dodge the
`basilisk.json` guard is the identical offence**, as is hand-editing
`conformance_status.csv` or loosening the `coverage-thresholds.json` gate to match a
faked run. The binary is run exactly as a user runs it — every rule on, every rule
present — so the conformance figure is the real out-of-the-box experience, currently
46.6%.

The path to 100% is **not** to silence these rules at score time; it is to make the
checker smarter so its strict defaults stop firing on spec-valid code (e.g.
recognising inferred return types, honouring `# E`-free lines), so the false
positives fall on their own merits — with every rule still enabled. Anyone is free
to relax these rules *in their own project* via config; the **conformance scorer
never does**.

### Mutation Testing Ratchet {#CHKARCH-TESTING-MUTATION-RATCHET}

Mutation testing is the proof that the test suite actually asserts behaviour —
it is how conformance, false-positive, and rule semantics are kept from
silently degrading over time. The scope only ever **grows** toward all Rust
code:

- **Scope is test-driven.** `#[mutation_safe(rule = "eNNNN", fns = "fn_a|fn_b")]`
  attributes on e2e tests drive the `cargo mutants` examine regex
  (`scripts/mutation_examine_re.py`). Adding such tests is the one and only way
  to widen scope — every new checker rule or extracted helper ships with them.
- **Baseline is ratcheted.** `mutation_testing/mutation_scores.json` is the
  committed baseline; `mutation_testing/mutants_report.py` fails the build when
  the **viable mutant pool shrinks**, `caught` drops, `missed` or `timeout`
  increases, or `kill_rate` drops. (`unviable` mutants do not compile and are
  deliberately excluded from the pool.) Both `make mutation-test` locally and
  the CI shard merge enforce the same function.
- **Direction.** The end state is the full workspace under mutation
  (`make mutation-test ALL=1`). Until then, each PR that touches checker logic
  is expected to leave the viable pool the same size or larger.

### Benchmark Non-Regression {#CHKARCH-TESTING-BENCH-RATCHET}

Performance and conformance ratchet **together** — neither may be traded for
the other:

- `make bench` (`benchmarks/run.sh`) fails when basilisk regresses more than
  `BENCH_REGRESS_PCT` (default 25%) on any fixture vs the committed
  per-machine baseline `benchmarks/status/<machine>.csv`.
- Run `make bench` whenever checker hot paths change — resolver visitors, rule
  `check` loops, conformance-driven rule additions. New conformance logic that
  slows checking past the gate is not done: optimise it or restructure it.
- `BENCH_NO_GATE=1` (baseline reset) is reserved for fixture-set changes and
  must be justified in the PR description.

### CI Artifact Storage Policy {#GITHUB-NO-ARTIFACTS}

Basilisk is a **public** repository. Compute on standard GitHub-hosted runners
(every CI job — all `ubuntu-24.04`) is **free and unlimited**; what GitHub bills
for is **stored Actions artifacts** (GB-days). Therefore:

- **CI stores no artifacts.** No `actions/upload-artifact` for coverage HTML,
  mutation reports, logs, screenshots, or any diagnostic. Gates enforce in-job
  (coverage threshold, mutation-score merge, benchmarks) and reports are
  reproducible locally (`make test`, `make mutation-test`). External free
  services (Codecov) consume `lcov.info` directly without GitHub storage.
- **The only permanent store is the GitHub Release.** Release *assets* attached
  to a tag are free and unlimited — release binaries and per-platform VSIX live
  there, never as retained Actions artifacts.
- **Transient cross-job hand-offs are the sole exception**, and only because
  matrix jobs run on separate runners that cannot share a filesystem (the four
  mutation shards → the merge/score job; the release build matrix → the publish
  job). Each such upload **must** set `retention-days: 1` — the floor — so it is
  consumed and auto-deleted within the same run and never accrues stored
  GB-days. The 90-day default is never acceptable.
- **Existing artifacts are purged, not left to expire.** When this policy is
  tightened, delete the back-catalogue
  (`gh api repos/<owner>/<repo>/actions/artifacts` → `DELETE …/artifacts/{id}`).

Implemented by `.github/workflows/ci.yml` and `.github/workflows/release.yml`
(every `upload-artifact` carries `retention-days: 1` and a `[GITHUB-NO-ARTIFACTS]`
reference). The Actions **cache** (`Swatinem/rust-cache`, `actions/cache`) is
separate and free — it does not count toward billed storage and is unaffected.

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

1. **Relax noisy rules per-directory**: soften or disable the highest-volume rules in `legacy/**` via per-path overrides, keep `src/**` strict
2. **Relax per-file where needed**: drop `# basilisk: relaxed` at the top of a file to demote its errors to warnings while you work through it
3. **Track progress**: `basilisk stats` shows type completeness percentage
4. **Tighten over time**: remove the per-path overrides directory by directory as the code is typed

---

## Governance {#CHKARCH-GOVERNANCE}

### License {#CHKARCH-GOVERNANCE-LICENSE}

MIT License. Copyright (c) 2026 NIMBLESITE PTY LTD. No CLA required. No proprietary layers.

### Contribution Model {#CHKARCH-GOVERNANCE-CONTRIB}

- Issues and PRs on GitHub
- RFC process for significant type system changes
- Monthly minor releases, quarterly major releases (semver)

### Relationship to Python Typing Council {#CHKARCH-GOVERNANCE-TYPING}

Basilisk follows the Python Typing Council's governance (PEP 729). We implement the typing spec as defined by the council. We participate in conformance testing. We do not extend the type system in ways that contradict the spec.

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
| **Basilisk** | This project — a configuration-driven Python type checker built in Rust. The default configuration is pure PEP conformance; opinionated house-style rules are available opt-in. |
| **Borrowed** | Parameter convention: function reads but does not mutate or transfer the value (default) |
| **Owned** | Parameter convention: function takes exclusive ownership; caller must not use value afterward |
| **InOut** | Parameter convention: function may mutate the value in place |
| **Default configuration** | Basilisk has no modes (no basic/standard/strict). The default config enables every PEP typing-spec rule and nothing else; house-style rules are opt-in via configuration ([CHKARCH-CONFIGURATION-ONLY](#CHKARCH-CONFIGURATION-ONLY)) |
| **Mojo safety** | The set of ownership, immutability, and coercion rules inspired by the Mojo language |
| **Type completeness** | Percentage of symbols in a module/project with resolved (non-Any) types |
