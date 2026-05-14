# Basilisk: Complete Type Safety for Python

**Version**: 0.1.0-draft
**Status**: Specification Draft
**License**: MIT

---

## Vision and Philosophy {#CHKARCH-VISION}

### The Problem {#CHKARCH-PROBLEM}

Python has a type system. Nobody uses it properly.

73% of Python developers write type hints. Only 41% enforce them in CI. Every existing type checker defaults to gradual typing -- untyped code passes silently. The result: type annotations are documentation, not contracts. They rot. They lie. They give false confidence.

The Python ecosystem has no equivalent of TypeScript. No tool exists that says: **"This code is not typed. It does not compile."**

Basilisk is that tool.

### Design Thesis {#CHKARCH-THESIS}

Basilisk treats Python as a statically typed language. It is to Python what TypeScript is to JavaScript -- a strict, typed superset that enforces contracts at analysis time.

- Every function parameter has a type.
- Every return type is declared.
- Every variable assignment resolves to a known type.
- `Any` is an explicit escape hatch, never an implicit default.

There is no "basic" mode. There is no "standard" mode. There is no `--permissive` flag. The type system is the product. Escape hatches exist for pragmatism, but the burden is on the developer to justify the exception, not to remember to enable the rule.

Rust does not have a flag that disables the borrow checker. TypeScript's `strict: true` is the expected default. Basilisk takes the same stance for Python.

### Mojo: The North Star {#CHKARCH-MOJO}

Mojo demonstrated that Python-family syntax can support ownership semantics, immutability by default, and zero implicit coercion. Basilisk adapts these concepts as static analysis rules over standard Python -- no Mojo dependency required.

### Project Principles {#CHKARCH-PRINCIPLES}

1. **Strict by default, escape hatches by choice** -- The safe path is the default path
2. **Every error must teach** -- Diagnostics explain why, not just what
3. **Don't reinvent wheels** -- Depend on quality open-source tools (Ruff, ty, typeshed) for everything we can
4. **Performance is a feature** -- Sub-10ms incremental checks or it's broken
5. **Open source means open governance** -- No proprietary layers, no vendor lock-in
6. **Mojo-compatible, not Mojo-dependent** -- Honor the concepts, own the implementation
7. **First-class developer experience** -- VS Code extensions, LSP, CLI -- everything works out of the box

---

## Ecosystem Gap Analysis {#CHKARCH-GAP}

See the project README for competitive analysis.

### Capability Matrix {#CHKARCH-MATRIX}

| Capability | Pyright | mypy | ty | Pyrefly | Zuban | Ruff | **Basilisk** |
|---|---|---|---|---|---|---|---|
| Implementation | TypeScript | Python/C | Rust | Rust | Rust | Rust | **Rust** |
| License | MIT | MIT | MIT | MIT | AGPL | MIT | **MIT** |
| Default strictness | Gradual | Gradual | Gradual | Gradual | Gradual | N/A | **Strict only** |
| PEP conformance target | ~95% | ~85% | ~15% | ~58% | ~69% | N/A | **100%** |
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
| ty | MIT Rust, but we build our own checker with different philosophy (strict-by-default). We may contribute upstream or share crates where sensible. |
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

#### Strict Is the Only Mode {#CHKARCH-STRICTNESS-ONLY}

Basilisk has one mode. It is strict.

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

There is no `--basic`, `--standard`, or `--permissive` flag. Every function parameter must be annotated. Every function must declare its return type. Every variable assigned from an untyped source must have an explicit annotation.

#### `Any` Is Explicit, Never Implicit {#CHKARCH-STRICTNESS-ANY}

```python
from typing import Any

# ERROR: Implicit Any -- untyped import [BSK-E0010]
from untyped_lib import do_stuff

# OK: Explicit Any with reason
result: Any = do_stuff()  # basilisk: allow[BSK-E0010] -- untyped dependency, tracking in #1234

# ERROR: Bare Any without justification in strict mode
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
from fastmcp import FastMCP  # type: ignore[BSK-E0010]
```

**Per-line: severity override (demote or promote)**
```python
from fastmcp import FastMCP  # type: warning[BSK-E0010]
from fastmcp import FastMCP  # type: info[BSK-E0010]
from fastmcp import FastMCP  # type: disabled[BSK-E0010]
```

**Per-line: override all rules on this line**
```python
data = unsafe_cast(value)  # type: warning
data = unsafe_cast(value)  # type: disabled
```

**Per-block: override severity for a range of lines**
```python
# type: disabled[BSK-E0010]
from fastmcp import FastMCP
from result import Result, Ok, Err
from errors import AutomatorError, ErrorCode
from models import Platform, Credentials
# type: end-disabled[BSK-E0010]
```

Block directives work with all modes: `# type: warning[CODE]` / `# type: end-warning[CODE]`, `# type: info[CODE]` / `# type: end-info[CODE]`, `# type: disabled[CODE]` / `# type: end-disabled[CODE]`. Omitting the code applies to all rules.

**Per-file: file-level mode at the top of the file**
```python
# basilisk: relaxed
# All errors become warnings in this file
```

```python
# basilisk: file-disabled[BSK-E0010]
# Disable E0010 for the entire file
```

```python
# basilisk: file-warning[BSK-E0010, BSK-E0011]
# Demote E0010 and E0011 to warnings for the entire file
```

**Per-directory configuration** in `pyproject.toml`:
```toml
[tool.basilisk]
strict = true  # default, cannot be set to false globally

[tool.basilisk.per-path-overrides."legacy/**"]
strict = false  # gradual typing for legacy code
deadline = "2025-12-31"  # enforcement deadline -- becomes strict after this date

[tool.basilisk.per-path-overrides."vendor/**"]
rules.disabled = ["BSK-E0010"]
rules.warning = ["BSK-E0001", "BSK-E0002"]
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
"BSK-E0010" = "warning"    # demote globally
"BSK-W0050" = "error"      # promote globally
"BSK-E0060" = "disabled"   # disable globally
```

**Migration mode** (project-wide, time-boxed):
```toml
[tool.basilisk.migration]
enabled = true
started = "2025-06-01"
enforce_after = "2025-12-01"  # all errors become warnings until this date
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
| `# type: ignore[BSK-E0010]` | Suppress specific code (Basilisk extension, mypy-compatible syntax) |
| `# type: warning` | Demote all diagnostics to warnings (Basilisk-specific) |
| `# type: warning[BSK-E0010]` | Demote specific code to warning (Basilisk-specific) |
| `# type: info` | Demote all diagnostics to info (Basilisk-specific) |
| `# type: info[BSK-E0010]` | Demote specific code to info (Basilisk-specific) |
| `# type: disabled` | Disable all diagnostics on this line (Basilisk-specific) |
| `# type: disabled[BSK-E0010]` | Disable specific code on this line (Basilisk-specific) |
| `# basilisk: relaxed` | Per-file: all errors become warnings |
| `# basilisk: file-disabled[CODE]` | Per-file: disable specific rules |
| `# basilisk: file-warning[CODE]` | Per-file: demote specific rules to warnings |

The `# type:` prefix ensures compatibility with editors and tools that already recognize `# type: ignore`. Other type checkers will treat `# type: warning` as an unknown directive and ignore it gracefully.

### Python Typing PEP Coverage {#CHKARCH-PEPS}

Basilisk targets **100% conformance** with the Python typing specification. We run the official conformance test suite (`python/typing` repository) in CI.

#### Foundation PEPs

| PEP | Title | Status |
|---|---|---|
| 484 | Type Hints | Required |
| 526 | Variable Annotations | Required |
| 544 | Protocols (Structural Subtyping) | Required |
| 585 | Generics in Standard Collections | Required |
| 604 | Union `X \| Y` Syntax | Required |

#### Advanced PEPs

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
- **Cross-module inference**: does NOT cross module boundaries for public symbols. Imports from typed modules resolve to declared types. Imports from untyped modules produce `BSK-E0010`.

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

---

## Mojo-Inspired Safety Analysis {#CHKARCH-MOJO-SAFETY}

Basilisk adapts Mojo's ownership, immutability, and coercion concepts as static analysis rules over standard Python using `typing.Annotated`, decorators, and `dataclass(frozen=True)`. No Mojo code or runtime is required.

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
    data.append(1)            # ERROR: mutation of Borrowed parameter [BSK-E0030]
    return consumed           # OK: owned value returned

items = [1, 2, 3]
temp = [4, 5]
buf: list[int] = []

result = process(data=items, buffer=buf, consumed=temp)
print(temp)  # ERROR: use after ownership transfer [BSK-E0031]
print(buf)   # OK: InOut reference still valid
```

**Static analysis rules**:
- `BSK-E0030`: Mutation of `Borrowed` parameter
- `BSK-E0031`: Use-after-move (value used after `Owned` transfer)
- `BSK-E0032`: Implicit copy of large structure (suggest explicit `.copy()`)
- `BSK-W0033`: Missing ownership annotation on mutable parameter (suggestion)

### Immutability by Default {#CHKARCH-MOJO-IMMUTABLE}

Function parameters are treated as immutable by default. Mutation of a parameter produces a diagnostic unless annotated with `InOut`:

```python
def bad(items: list[int]) -> None:
    items.append(1)  # ERROR: mutation of parameter [BSK-E0040]
    items = [1, 2]   # ERROR: reassignment of parameter [BSK-E0041]

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
c.timeout = 30  # ERROR: dynamic attribute on typed structure [BSK-E0050]
```

**Rules**:
- `BSK-E0050`: Dynamic attribute assignment on typed class
- `BSK-E0051`: Missing `__init__` on class with type annotations
- `BSK-E0052`: Missing `__del__` on class managing resources (when detectable)
- `BSK-W0053`: Class should use `__slots__` for performance (suggestion)

### No Implicit Type Coercion {#CHKARCH-MOJO-COERCION}

```python
x: float = 1        # ERROR: implicit int-to-float coercion [BSK-E0060]
x: float = float(1)  # OK: explicit conversion

y: int = True        # ERROR: implicit bool-to-int coercion [BSK-E0061]
y: int = int(True)   # OK: explicit conversion

z: str = b"hello"    # ERROR: implicit bytes-to-str [BSK-E0062]
```

### Mojo Compatibility Matrix {#CHKARCH-MOJO-COMPAT}

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

#### Missing Annotations (BSK-E0001 -- BSK-E0009)

| Code | Description |
|---|---|
| BSK-E0001 | Missing parameter type annotation |
| BSK-E0002 | Missing return type annotation |
| BSK-E0003 | Missing variable type (unresolvable inference) |
| BSK-E0004 | Missing `*args` / `**kwargs` type annotation |
| BSK-E0005 | Missing class attribute type annotation |

#### Type Safety (BSK-E0010 -- BSK-E0029)

| Code | Description |
|---|---|
| BSK-E0010 | Import from untyped module without stub |
| BSK-E0011 | Implicit `Any` (type resolves to `Any` without explicit annotation) |
| BSK-E0012 | Argument type mismatch |
| BSK-E0013 | Return type mismatch |
| BSK-E0014 | Assignment type incompatibility |
| BSK-E0015 | Invalid type argument |
| BSK-E0016 | Incompatible method override |
| BSK-E0017 | Incompatible variable override |
| BSK-E0018 | Undefined variable |
| BSK-E0019 | Unbound variable (some code paths) |
| BSK-E0020 | Missing overload implementation |
| BSK-E0021 | Overlapping overloads with incompatible returns |
| BSK-E0022 | Unhashable type in hash-requiring context |
| BSK-E0023 | Non-exhaustive pattern match |
| BSK-E0024 | Invalid type form in annotation |
| BSK-E0025 | Missing `@override` decorator |

#### Ownership Safety (BSK-E0030 -- BSK-E0039)

| Code | Description |
|---|---|
| BSK-E0030 | Mutation of `Borrowed` parameter |
| BSK-E0031 | Use after ownership transfer (`Owned`) |
| BSK-E0032 | Implicit copy of large structure |
| BSK-W0033 | Missing ownership annotation on mutable parameter |
| BSK-E0034 | Owned value not consumed or returned |
| BSK-E0035 | Multiple mutable references to same value |

#### Immutability (BSK-E0040 -- BSK-E0049)

| Code | Description |
|---|---|
| BSK-E0040 | Mutation of immutable parameter |
| BSK-E0041 | Reassignment of immutable parameter |
| BSK-W0042 | Mutable dataclass (prefer `frozen=True`) |
| BSK-E0043 | Mutation of `Final` variable |

#### Structural Discipline (BSK-E0050 -- BSK-E0059)

| Code | Description |
|---|---|
| BSK-E0050 | Dynamic attribute on typed structure |
| BSK-E0051 | Missing `__init__` on annotated class |
| BSK-E0052 | Missing resource cleanup (`__del__` / context manager) |
| BSK-W0053 | Missing `__slots__` (performance suggestion) |
| BSK-E0054 | Sealed class (`@final`) subclassed |

#### Coercion Safety (BSK-E0060 -- BSK-E0069)

| Code | Description |
|---|---|
| BSK-E0060 | Implicit `int`-to-`float` coercion |
| BSK-E0061 | Implicit `bool`-to-`int` coercion |
| BSK-E0062 | Implicit `bytes`-to-`str` coercion |
| BSK-E0063 | Implicit numeric widening |

#### Optional Safety (BSK-E0070 -- BSK-E0079)

| Code | Description |
|---|---|
| BSK-E0070 | Subscript on `Optional` type |
| BSK-E0071 | Member access on `Optional` type |
| BSK-E0072 | Call on `Optional` type |
| BSK-E0073 | Iteration over `Optional` type |

#### Unused Code (BSK-W0080 -- BSK-W0089)

| Code | Description |
|---|---|
| BSK-W0080 | Unused import |
| BSK-W0081 | Unused variable |
| BSK-W0082 | Unused function (private) |
| BSK-W0083 | Unused class (private) |
| BSK-W0084 | Unused call result (non-None return) |
| BSK-W0085 | Unreachable code |

#### Code Quality (BSK-W0090 -- BSK-W0099)

| Code | Description |
|---|---|
| BSK-W0090 | Unnecessary `isinstance` (always true/false) |
| BSK-W0091 | Unnecessary cast |
| BSK-W0092 | Unnecessary comparison (always true/false) |
| BSK-W0093 | Deprecated API usage |
| BSK-W0094 | Type comment usage (use annotation syntax) |
| BSK-W0095 | Assert with side effects |

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
+------------------+
| basilisk-mojo    |  Ownership, immutability, coercion analysis
+------------------+
       |  Full Diagnostics
       v
+------------------+     +------------------+
| basilisk-lsp     |     | basilisk-cli     |
| (IDE server)     |     | (CI/terminal)    |
+------------------+     +------------------+
       |                         |
       v                         v
  VS Code / Neovim /      Terminal / CI / SARIF
  PyCharm / Helix / Zed
```

All stages are backed by:
```
+------------------+
| basilisk-db      |  Salsa incremental computation database
+------------------+
```

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
strict = false
deadline = "2025-12-31"

[tool.basilisk.migration]
enabled = false
enforce_after = "2026-01-01"
```

### Migration from Existing Tools {#CHKARCH-CONFIG-MIGRATION}

```bash
basilisk migrate --from pyright   # Reads pyrightconfig.json -> pyproject.toml
basilisk migrate --from mypy      # Reads mypy.ini / setup.cfg -> pyproject.toml
```

Semantic mapping:
- Pyright `strict` mode -> Basilisk default (strict) with Mojo safety disabled
- Pyright `standard` mode -> Basilisk `per-path-overrides` with `strict = false`
- mypy `--strict` -> Basilisk default with Mojo safety disabled

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
| BSK-E0040 (mutation of immutable param) | Add `InOut` annotation |
| BSK-E0060 (implicit coercion) | Wrap in explicit conversion |

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
| Benchmarks | Criterion + real codebases | Performance regression gates |

---

## Migration and Adoption {#CHKARCH-MIGRATION}

### From mypy {#CHKARCH-MIGRATION-MYPY}

1. Run `basilisk migrate --from mypy`
2. Fix BSK-E0001/E0002 errors (missing annotations) -- these are the primary diff
3. Address BSK-E0040+ (Mojo safety) or disable with `mojo-safety = false`

### From Pyright {#CHKARCH-MIGRATION-PYRIGHT}

1. Run `basilisk migrate --from pyright`
2. If using strict mode: minimal changes needed for core type checking
3. Enable Mojo safety incrementally

### Gradual Adoption {#CHKARCH-MIGRATION-GRADUAL}

1. **Start**: Enable Basilisk in migration mode (errors -> warnings)
2. **Adopt per-directory**: Mark `src/` as strict, leave `legacy/` relaxed
3. **Track progress**: `basilisk stats` shows type completeness percentage
4. **Set deadline**: `deadline = "2025-12-31"` in per-path overrides
5. **Enforce**: After deadline, relaxed paths become strict

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

### Phase 1: Foundation
- Parser (evaluate `ruff_python_parser` vs custom)
- Name resolver
- Basic type checker (50% PEP conformance)
- CLI with human-readable output
- CI pipeline

### Phase 2: LSP and Editors
- Language server (diagnostics, hover, completions)
- VS Code extension (VSIX)
- Integrated Python debugging via DAP proxy over debugpy (§10.1.1)
- Neovim / Helix configuration

### Phase 3: Strict-by-Default
- All BSK-E0001 through BSK-E0025 rules
- Migration mode
- `basilisk migrate` from mypy/Pyright
- 80% PEP conformance

### Phase 4: Mojo Safety
- Ownership tracking (BSK-E003x)
- Immutability enforcement (BSK-E004x)
- Structural discipline (BSK-E005x)
- Coercion detection (BSK-E006x)

### Phase 5: Plugin System and Stubs
- WASM plugin host
- Django, Pydantic, SQLAlchemy plugins
- Auto-stub generation engine
- Stub registry

### Phase 6: Production Hardening
- 95%+ PEP conformance
- Performance optimization (meet all targets in Section 8.4)
- SARIF/JUnit output
- Enterprise migration playbook

### Phase 7: Ecosystem Growth
- Plugin marketplace
- Community stub registry
- Conference talks, documentation, tutorials
- PyCharm / IntelliJ plugin maturity

---

## Appendix A: Full PEP Coverage Matrix

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

## Appendix B: Glossary

| Term | Definition |
|---|---|
| **Basilisk** | This project — a strict-by-default Python type checker built in Rust. No escape hatches. |
| **Borrowed** | Parameter convention: function reads but does not mutate or transfer the value (default) |
| **Owned** | Parameter convention: function takes exclusive ownership; caller must not use value afterward |
| **InOut** | Parameter convention: function may mutate the value in place |
| **Strict mode** | Basilisk's only mode -- all types must be declared or inferable |
| **Migration mode** | Temporary mode where errors become warnings until an enforcement deadline |
| **Mojo safety** | The set of ownership, immutability, and coercion rules inspired by the Mojo language |
| **Type completeness** | Percentage of symbols in a module/project with resolved (non-Any) types |
