# Basilisk: Complete Type Safety for Python

**Version**: 0.1.0-draft
**Status**: Specification Draft
**License**: Apache-2.0 OR MIT (dual-license)

---

## 1. Vision and Philosophy

### 1.1 The Problem

Python has a type system. Nobody uses it properly.

73% of Python developers write type hints. Only 41% enforce them in CI. Every existing type checker defaults to gradual typing -- untyped code passes silently. The result: type annotations are documentation, not contracts. They rot. They lie. They give false confidence.

The Python ecosystem has no equivalent of TypeScript. No tool exists that says: **"This code is not typed. It does not compile."**

Basilisk is that tool.

### 1.2 Design Thesis

Basilisk treats Python as a statically typed language. It is to Python what TypeScript is to JavaScript -- a strict, typed superset that enforces contracts at analysis time.

- Every function parameter has a type.
- Every return type is declared.
- Every variable assignment resolves to a known type.
- `Any` is an explicit escape hatch, never an implicit default.

There is no "basic" mode. There is no "standard" mode. There is no `--permissive` flag. The type system is the product. Escape hatches exist for pragmatism, but the burden is on the developer to justify the exception, not to remember to enable the rule.

Rust does not have a flag that disables the borrow checker. TypeScript's `strict: true` is the expected default. Basilisk takes the same stance for Python.

### 1.3 Mojo: The North Star

Mojo demonstrated that Python-family syntax can support ownership semantics, immutability by default, and zero implicit coercion -- concepts previously associated only with systems languages like Rust and C++.

Basilisk draws direct inspiration from Mojo's type discipline:

- **Mojo's `fn` vs `def`**: In Basilisk, all `def` functions are strict by default. No keyword distinction needed because there is no permissive mode.
- **Mojo's ownership model**: Basilisk adapts `borrowed`, `owned`, and `inout` as static analysis annotations over standard Python.
- **Mojo's structural immutability**: Basilisk enforces immutability by default for function parameters and typed structures.

Basilisk serves as a bridge. Until the Mojo compiler can do everything CPython does, Basilisk brings Mojo's type discipline to every Python codebase today -- without requiring a new compiler, a new runtime, or any dependency on Mojo itself. Code that passes Basilisk's checks should be compatible with Mojo's type expectations.

### 1.4 What Basilisk Is

- A statically typed Python dialect (like TypeScript is to JavaScript)
- A static analyzer, language server, and CI tool
- Strongly typed by default -- you opt OUT of strictness, not in
- Compatible with standard CPython
- Compatible with Mojo's type discipline (no dependency on Mojo tooling)
- Built on existing open-source tools wherever possible
- 100% open source with open governance

### 1.5 What Basilisk Is Not

- Not a Python compiler or runtime
- Not a fork of any existing tool
- Not dependent on any Microsoft proprietary technology
- Not a Mojo dependency -- it references Mojo's concepts, not its code
- Not a gradual type checker -- gradual typing is what we're replacing

### 1.6 Project Principles

1. **Strict by default, escape hatches by choice** -- The safe path is the default path
2. **Every error must teach** -- Diagnostics explain why, not just what
3. **Don't reinvent wheels** -- Depend on quality open-source tools (Ruff, ty, typeshed) for everything we can
4. **Performance is a feature** -- Sub-10ms incremental checks or it's broken
5. **Open source means open governance** -- No proprietary layers, no vendor lock-in
6. **Mojo-compatible, not Mojo-dependent** -- Honor the concepts, own the implementation
7. **First-class developer experience** -- VS Code extensions, LSP, CLI -- everything works out of the box

---

## 2. Ecosystem Gap Analysis

### 2.1 Current Landscape (2025-2026)

The Python type checking landscape is in a generational shift. Three Rust-based type checkers launched in 2025 (ty, Pyrefly, Zuban), challenging the TypeScript incumbent (Pyright) and the aging Python-based tools (mypy, Pytype). Google has deprecated Pytype. Meta is replacing Pyre with Pyrefly.

Yet every single tool -- new and old -- defaults to gradual typing. None enforce complete type safety. None adopt Mojo-inspired ownership semantics. The ecosystem has fast tools, but no tool that treats Python as a typed language.

### 2.2 Tool-by-Tool Assessment

#### 2.2.1 Pyright

| Attribute | Value |
|---|---|
| Language | TypeScript |
| License | MIT |
| Strictness | Gradual (4 modes: off/basic/standard/strict) |
| PEP Conformance | ~95% (best in class) |
| LSP | Yes |
| Incremental | Yes (lazy evaluation) |
| Plugin System | No |

**Strengths**: Best PEP conformance. Fast incremental analysis. Powers Pylance (the de facto VS Code experience). 81 diagnostic rules. Excellent type narrowing and flow analysis.

**Weaknesses**: Strict mode is opt-in, not default. Pylance's IDE features (semantic highlighting, IntelliCode, refactoring code actions, auto-import) are proprietary and locked to Microsoft's VS Code. Requires Node.js runtime. No ownership analysis. No plugin system (Microsoft explicitly rejected this). Configuration cannot decrease severity below mode defaults.

**What we reuse**: Nothing directly. Pyright is TypeScript -- we can't link to it. But its diagnostic rule catalog (81 rules) is the benchmark for our own rule set.

#### 2.2.2 mypy

| Attribute | Value |
|---|---|
| Language | Python/C |
| License | MIT |
| Strictness | Gradual |
| PEP Conformance | ~85% |
| LSP | No (third-party pylsp, unrecommended) |
| Incremental | Daemon mode (fragile) |
| Plugin System | Yes (Python hooks) |

**Strengths**: Original type checker. Plugin system enables framework support (Django, SQLAlchemy). Large user base. Mature.

**Weaknesses**: Slow -- 10-100x slower than Rust-based tools. No first-class LSP. Daemon mode is fragile. Declining relevance as Rust-based tools emerge. PEP adoption velocity is low.

**What we reuse**: mypy's plugin API design is a reference for our own extension system. The mypy stubs ecosystem (`types-*` packages) feeds into typeshed which we consume.

#### 2.2.3 ty (Astral)

| Attribute | Value |
|---|---|
| Language | Rust |
| License | MIT |
| Strictness | Gradual |
| PEP Conformance | ~15% (beta, Dec 2025) |
| LSP | Yes |
| Incremental | Salsa-based (500x faster than Pyright) |
| Plugin System | Planned |

**Strengths**: 500x faster incremental updates than Pyright (4.7ms vs 2.38s on PyTorch). Built by the Ruff team (Astral). Salsa-based incremental architecture is production-proven (rust-analyzer). rustc-quality diagnostics. MIT license.

**Weaknesses**: Only ~15% PEP conformance (early beta). Years from full coverage. No ownership analysis. Plugin system not yet designed.

**What we reuse**: ty's Salsa-based architecture is the model for our incremental computation. Astral also maintains `ruff_python_parser` (MIT) which we should evaluate as our parser. The Ruff linter itself handles formatting and linting -- we should depend on it rather than recreating those features.

#### 2.2.4 Pyrefly (Meta)

| Attribute | Value |
|---|---|
| Language | Rust |
| License | MIT |
| Strictness | Gradual |
| PEP Conformance | ~58% (alpha, May 2025) |
| LSP | Yes |
| Incremental | Yes (module-level) |
| Plugin System | No |

**Strengths**: 1.8 million LOC/sec throughput. Auto-infers types for unannotated code. Built and battle-tested on Instagram's codebase. Good documentation.

**Weaknesses**: ~58% conformance. Single-org focus (Meta's internal needs drive priorities). No strict-by-default mode. No ownership analysis. No plugin system.

**What we reuse**: Pyrefly's type inference engine design is a reference. Their approach to inferring return types and local variable types from unannotated code is relevant for our inference engine (which must infer locals even in strict mode).

#### 2.2.5 Zuban

| Attribute | Value |
|---|---|
| Language | Rust |
| License | AGPL (commercial license available) |
| Strictness | Gradual |
| PEP Conformance | ~69% (best among Rust tools) |
| LSP | Yes |
| Incremental | No (single-threaded) |
| Plugin System | No |

**Strengths**: Best PEP conformance of any Rust-based checker (69%). Dual-mode support (Pyright-compatible and mypy-compatible). Built by the author of Jedi. Uses ~50% less memory/CPU than competitors.

**Weaknesses**: AGPL license may deter corporate adoption. Single-threaded. No plugin system. No ownership analysis.

**What we reuse**: Zuban's conformance test results are a benchmark. Its dual-mode configuration approach (supporting both mypy and Pyright config formats) informs our migration tooling.

#### 2.2.6 Ruff (Astral)

| Attribute | Value |
|---|---|
| Language | Rust |
| License | MIT |
| Scope | Linting + Formatting (NOT type checking) |

**Strengths**: Lightning-fast Rust-based linter and formatter. 700+ lint rules. Drop-in replacement for flake8, isort, Black. Massive adoption.

**What we reuse**: Ruff is a direct dependency. Basilisk does NOT recreate linting or formatting. We delegate to Ruff for all lint rules and code formatting. We also evaluate `ruff_python_parser` as our Python parser crate.

### 2.3 Capability Matrix

| Capability | Pyright | mypy | ty | Pyrefly | Zuban | Ruff | **Basilisk** |
|---|---|---|---|---|---|---|---|
| Implementation | TypeScript | Python/C | Rust | Rust | Rust | Rust | **Rust** |
| License | MIT | MIT | MIT | MIT | AGPL | MIT | **Apache-2.0/MIT** |
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

### 2.4 Structural Gaps No Existing Tool Addresses

1. **Strict-by-default with no permissive mode** -- Every tool treats strictness as opt-in. No tool treats untyped code as an error by default.
2. **Ownership/borrowing as static analysis** -- No Python type checker tracks value ownership or flags use-after-move.
3. **Immutability-by-default for parameters** -- No tool flags mutation of function parameters without explicit annotation.
4. **Implicit copy detection** -- No tool warns when large structures are implicitly copied.
5. **No implicit type coercion** -- No tool flags `int`-to-`float` promotion or `bool`-as-`int` usage.
6. **First-class open-source VS Code experience** -- The best Python IDE experience (Pylance) is proprietary. The open-source alternatives are fragmented.
7. **Unified toolchain** -- No single tool provides type checking + linting + formatting + LSP + VSIX as one coherent experience. (Basilisk achieves this by integrating with Ruff, not by reimplementing.)
8. **Mojo compatibility** -- No tool validates code against Mojo's type discipline.

### 2.5 Opportunity

The technology is ready (Rust-based type checkers are proven). The philosophy is unoccupied (no tool is strict-by-default). The ecosystem needs it (Python is the world's most popular language with the weakest type enforcement). Mojo proved the concepts are sound. Basilisk brings them to every Python codebase today.

---

## 3. Dependency Strategy: Standing on Shoulders

Basilisk does not reinvent wheels. We depend on quality open-source tools for everything we can.

### 3.1 Direct Dependencies

| Dependency | Purpose | License | Rationale |
|---|---|---|---|
| **Ruff** (`ruff` CLI) | Linting + formatting | MIT | Best-in-class. 700+ rules. We don't recreate lint or format. |
| **`ruff_python_parser`** | Python AST parsing | MIT | Battle-tested Rust crate. Powers Ruff. Evaluate as our parser. |
| **typeshed** | Standard library type stubs | Apache-2.0 | Community standard. We bundle it and extend it. |
| **Salsa** | Incremental computation framework | Apache-2.0/MIT | Powers rust-analyzer. Proven at scale. |
| **`lsp-server`** / **`tower-lsp`** | LSP implementation | MIT | Standard Rust LSP crates. |

### 3.2 Tools We Do NOT Depend On

| Tool | Why Not |
|---|---|
| Pyright/Pylance | TypeScript, Microsoft ecosystem. Cannot link. Cannot extend. |
| mypy | Python, too slow for our architecture. Reference only. |
| ty | MIT Rust, but we build our own checker with different philosophy (strict-by-default). We may contribute upstream or share crates where sensible. |
| Pyrefly | MIT Rust, same reasoning as ty. Different design goals. |
| Node.js | No JavaScript runtime dependency anywhere in the stack. |

### 3.3 Interoperability

| Tool | Interop Strategy |
|---|---|
| **Ruff** | Basilisk invokes `ruff check` and `ruff format` as subprocesses or links the Ruff crates directly. Configuration unified in `pyproject.toml`. |
| **typeshed** | Bundled copy of typeshed stubs, updated with each Basilisk release. Users can override with custom stubs. |
| **mypy config** | `basilisk migrate --from mypy` reads `mypy.ini` / `setup.cfg` and produces `[tool.basilisk]` config. |
| **Pyright config** | `basilisk migrate --from pyright` reads `pyrightconfig.json` and produces `[tool.basilisk]` config. |
| **PEP 561** | Full support for `py.typed` packages, inline type annotations, and stub-only packages. |

---

## 4. Core Type System

### 4.1 Strictness Model

#### 4.1.1 Strict Is the Only Mode

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

#### 4.1.2 `Any` Is Explicit, Never Implicit

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

#### 4.1.3 Diagnostic Severity Modes

Every rule has four severity modes:

| Mode | Behavior | Blocks CI | LSP Indicator |
|---|---|---|---|
| `error` | Full diagnostic with fix suggestions | Yes | Red squiggly |
| `warning` | Diagnostic shown but does not block | No | Yellow squiggly |
| `info` | Informational hint only | No | Blue hint |
| `disabled` | Rule is not checked at all (zero cost) | No | Nothing |

The default mode for each rule is determined by its code prefix (`E` = error, `W` = warning). All modes can be overridden at every level: per-line, per-block, per-file, and per-project.

#### 4.1.4 Inline Suppression and Mode Override

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

#### 4.1.5 Suppression Precedence

When multiple overrides apply, the most specific wins:

1. **Per-line comment** (highest priority)
2. **Per-block comment**
3. **Per-file directive**
4. **Per-path override** in pyproject.toml
5. **Per-module override** in pyproject.toml
6. **Global rule override** in pyproject.toml
7. **Rule default** (lowest priority)

#### 4.1.6 Compatibility

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

### 4.2 Python Typing PEP Coverage

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

### 4.3 Type Inference Engine

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

### 4.4 Type Narrowing and Flow Analysis

Full support for:
- `isinstance()` / `issubclass()` guards with bidirectional narrowing
- Truthiness narrowing (`if x:` narrows `Optional[T]` to `T`)
- Pattern matching exhaustiveness (PEP 634)
- Sentinel / `None` narrowing
- Custom type guards (`TypeGuard`, `TypeIs` per PEP 742)
- Negative narrowing in `else` branches
- Assignment-based narrowing

### 4.5 Reachability Analysis

- Dead code detection after narrowing
- Unreachable branch elimination
- `NoReturn` propagation from `sys.exit()`, `raise`, and custom `NoReturn` functions
- `assert_never()` for exhaustiveness checking
- Platform-aware reachability (default: assume code may run on any platform)

---

## 5. Mojo-Inspired Safety Analysis

### 5.1 Design Philosophy

Mojo proved that Python-family syntax can enforce ownership, immutability, and coercion safety. Basilisk adapts these concepts as static analysis rules over standard Python using existing annotation mechanisms (`typing.Annotated`, decorators, `dataclass(frozen=True)`).

No Mojo code is used. No Mojo runtime is required. The analysis is additive -- it runs as additional passes alongside standard type checking.

Code that passes Basilisk's Mojo-safety checks should be structurally compatible with Mojo's type expectations when Mojo achieves full Python compatibility.

### 5.2 Function Strictness

All functions require complete type annotations. This is not a separate mode -- it is the default behavior (Section 4.1).

```python
# In Mojo, `fn` enforces strict typing. In Basilisk, all `def` is strict.
# No new keyword needed.

def add(a: int, b: int) -> int:  # OK in both Basilisk and Mojo
    return a + b
```

**Difference from Mojo**: Mojo has `fn` (strict) vs `def` (dynamic). Basilisk has only `def`, and it is always strict. There is no dynamic mode to escape to.

### 5.3 Ownership and Lifetime Tracking

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

**Difference from Mojo**: Mojo enforces ownership at the compiler level with the `^` transfer operator. Basilisk enforces it via static analysis of annotation-decorated parameters. The `^` operator does not exist in Python -- Basilisk uses `Owned` annotation + use-after-move tracking instead.

### 5.4 Immutability by Default

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

**Difference from Mojo**: Mojo's `borrowed` is the default parameter convention. Basilisk mirrors this -- parameters are immutable by default in both systems.

### 5.5 Structural Discipline

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

**Difference from Mojo**: Mojo's `struct` is static by definition -- no dynamic attributes. Basilisk enforces this via analysis of classes with type annotations.

### 5.6 No Implicit Type Coercion

```python
x: float = 1        # ERROR: implicit int-to-float coercion [BSK-E0060]
x: float = float(1)  # OK: explicit conversion

y: int = True        # ERROR: implicit bool-to-int coercion [BSK-E0061]
y: int = int(True)   # OK: explicit conversion

z: str = b"hello"    # ERROR: implicit bytes-to-str [BSK-E0062]
```

**Difference from Mojo**: Mojo forbids implicit conversions entirely. Basilisk mirrors this philosophy -- all type conversions must be explicit.

### 5.7 Mojo Compatibility Matrix

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

## 6. Diagnostic Rules

### 6.1 Design Philosophy

Every diagnostic must be:
1. **Precise** -- exact location (file, line, column, span)
2. **Clear** -- explains what is wrong and why
3. **Actionable** -- suggests at least one fix
4. **Stable** -- error codes are never renumbered or reused

Inspired by `rustc`'s diagnostic system and ty's approach.

### 6.2 Error Code System

Format: `BSK-Xnnnn` where X = default severity class:
- `E` = Error (blocks CI by default)
- `W` = Warning (does not block by default)
- `I` = Info (suggestion by default)

The prefix determines the **default** severity. Every rule can be overridden to any of the four modes (`error`, `warning`, `info`, `disabled`) at every scope level (line, block, file, path, global). See Section 4.1.3 for the mode system and Section 4.1.4 for override syntax.

### 6.3 Rule Categories

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

## 7. Architecture

### 7.1 High-Level Pipeline

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

### 7.2 Rust Crate Structure

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

### 7.3 Crate Dependencies (Acyclic)

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

### 7.4 Build System

- **Cargo workspace** with all crates
- Cross-compilation targets: `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, `aarch64-darwin`, `x86_64-windows`
- CI: `cargo clippy`, `cargo test`, conformance suite, benchmarks, fuzzing (nightly)
- Release: pre-compiled binaries for all platforms (no build dependencies for users)

---

## 8. Incremental Computation

### 8.1 Salsa Architecture

Basilisk uses the Salsa incremental computation framework (the same system powering rust-analyzer).

**Input queries**: Source file contents, configuration, stub files
**Derived queries**: Parsed ASTs, resolved names, type assignments, diagnostics

When a source file changes, only queries that depend on the changed input are recomputed. The dependency graph is tracked automatically by Salsa.

### 8.2 Cancellation

When a new keystroke arrives while a check is in progress, the current computation is cancelled and restarted with the new input. This is critical for responsive IDE experience.

### 8.3 Persistent Cache

Disk-backed cache between sessions. On startup, Basilisk loads the cache and only recomputes files that changed since last run. This eliminates cold-start latency for repeat sessions.

### 8.4 Performance Targets

| Scenario | Target |
|---|---|
| Cold start, 100K LOC | < 5 seconds |
| Cold start, 1M LOC | < 30 seconds |
| Incremental (single file edit) | < 10ms |
| Memory, 1M LOC | < 2 GB |

---

## 9. Language Server Protocol (LSP)

### 9.1 LSP-First Design

Basilisk is an LSP server first, CLI tool second. The LSP server is the primary product. The CLI is a batch-mode wrapper around the same engine. This ensures interactive and CI experiences are always consistent.

### 9.2 Supported LSP Methods

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

### 9.3 Custom LSP Extensions

| Extension | Description |
|---|---|
| `basilisk/ownershipOverlay` | Visual ownership annotations (borrowed/owned/inout) in gutter |
| `basilisk/typeCompleteness` | Per-file/module type completeness score (% typed) |
| `basilisk/migrationProgress` | Project-wide migration dashboard |

---

## 10. Editor Integrations

### 10.1 VS Code Extension (VSIX)

The primary integration. Open source. No Microsoft proprietary dependencies.

**Architecture**:
- VSIX bundles a pre-compiled LSP server binary per platform
- No Node.js dependency for the server (the extension activation layer uses VS Code's extension API)
- Configuration exposed via VS Code settings with JSON schema validation

**Features**:
- All LSP features (diagnostics, completions, hover, navigation, refactoring)
- Semantic highlighting (token-level coloring by type)
- Inlay hints (inferred types, parameter names, ownership annotations)
- Ownership visualization (gutter icons: borrowed/owned/inout)
- Type completeness indicator (status bar: "87% typed")
- Migration dashboard (sidebar panel)
- Integrated Ruff linting/formatting (delegates to Ruff extension or bundled Ruff)
- Code actions for every diagnostic (add annotation, add return type, fix coercion, etc.)

### 10.2 Other Editors

| Editor | Integration |
|---|---|
| **Neovim** | Native LSP client (`nvim-lspconfig`). Configuration example provided. |
| **Helix** | Built-in LSP support. Language configuration provided. |
| **Zed** | Native LSP extension. |
| **PyCharm / IntelliJ** | LSP plugin (via IntelliJ LSP support). |
| **Emacs** | `eglot` / `lsp-mode` configuration. |

---

## 11. Command-Line Interface

### 11.1 Core Commands

```bash
basilisk check [paths...]         # Type check files/directories
basilisk check --watch            # Watch mode with incremental rechecks
basilisk stats [paths...]         # Type coverage report
basilisk stubs generate <package> # Generate stubs for installed package
basilisk migrate --from mypy      # Import mypy configuration
basilisk migrate --from pyright   # Import pyright configuration
basilisk init                     # Generate starter pyproject.toml config
```

### 11.2 Output Formats

| Format | Flag | Use Case |
|---|---|---|
| Human-readable | Default | Terminal (color, source context, fix suggestions) |
| JSON | `--output-format json` | Programmatic consumption |
| SARIF | `--output-format sarif` | GitHub Code Scanning, Azure DevOps |
| JUnit XML | `--output-format junit` | CI test result dashboards |

### 11.3 Exit Codes

| Code | Meaning |
|---|---|
| 0 | Clean -- no errors |
| 1 | Type errors found |
| 2 | Configuration error |
| 3 | Internal error |

### 11.4 CI Integration

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
- repo: https://github.com/MelbourneDeveloper/Basilisk
  rev: v0.1.0
  hooks:
    - id: basilisk-check
```

---

## 12. Stub System

### 12.1 Problem

typeshed is a centralized repository of stubs maintained by volunteers. It does not scale. Only ~70% of popular packages have any type coverage. Average coverage per package is ~35%. Maintaining stubs for thousands of packages is unsustainable.

### 12.2 Auto-Stub Generation

Basilisk includes a stub generation engine with three modes:

1. **Runtime introspection**: Import the package, inspect objects, generate `.pyi` files
2. **AST-based inference**: Parse package source, infer signatures without importing
3. **Hybrid**: Combine both, preferring runtime data with AST fallback

### 12.3 Stub Quality Tiers

| Tier | Source | Trust Level | Diagnostic Behavior |
|---|---|---|---|
| Tier 1 | Hand-written, verified, typeshed | High | No warnings |
| Tier 2 | Auto-generated, community reviewed | Medium | Info notes on potential inaccuracies |
| Tier 3 | Best-effort inference | Low | Warnings that types may be incomplete |

### 12.4 typeshed Compatibility

Basilisk bundles a copy of typeshed and uses it as the Tier 1 baseline for standard library stubs. Users can override with custom stubs via `stubPaths` configuration.

---

## 13. Plugin and Extension System

### 13.1 Motivation

Framework-specific type checking (Django ORM, SQLAlchemy, Pydantic, FastAPI) cannot be built into the core. A plugin system allows community extensions without forking.

### 13.2 Architecture

**WASM-based** for security and portability:
- Plugins compiled to WebAssembly
- Sandboxed execution (no filesystem, no network)
- Receive AST nodes and type information
- Return diagnostics and code actions

### 13.3 Extension Points

| Extension Point | Example |
|---|---|
| Custom diagnostic rules | Flag Django `QuerySet` misuse |
| Custom type providers | Infer SQLAlchemy model field types |
| Custom code actions | Generate Pydantic validator stubs |
| Custom type narrowing | Django `get_object_or_404` narrows to model type |

### 13.4 Distribution

Plugins declared in `pyproject.toml`:
```toml
[tool.basilisk.plugins]
django = "basilisk-plugin-django >= 0.1"
pydantic = "basilisk-plugin-pydantic >= 0.1"
```

---

## 14. Configuration

### 14.1 Configuration File

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

### 14.2 Migration from Existing Tools

```bash
basilisk migrate --from pyright   # Reads pyrightconfig.json -> pyproject.toml
basilisk migrate --from mypy      # Reads mypy.ini / setup.cfg -> pyproject.toml
```

Semantic mapping:
- Pyright `strict` mode -> Basilisk default (strict) with Mojo safety disabled
- Pyright `standard` mode -> Basilisk `per-path-overrides` with `strict = false`
- mypy `--strict` -> Basilisk default with Mojo safety disabled

---

## 15. Diagnostics Experience

### 15.1 Quality Standard

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

### 15.2 Quick Fixes

Every error has at least one associated code action:

| Error | Quick Fix |
|---|---|
| BSK-E0001 (missing param type) | Insert `: <inferred_type>` |
| BSK-E0002 (missing return type) | Insert `-> <inferred_type>` |
| BSK-E0040 (mutation of immutable param) | Add `InOut` annotation |
| BSK-E0060 (implicit coercion) | Wrap in explicit conversion |

---

## 16. Performance Engineering

### 16.1 Parallelism

- File-level parallelism using Rayon (work-stealing)
- Module dependency graph partitioned into independent subgraphs
- Cross-module dependencies resolved first in dependency-ordered pass

### 16.2 Memory

- Arena allocation for AST nodes
- Interned strings for identifiers and paths
- Memory-mapped file I/O

### 16.3 Benchmarks

Benchmark suite against real-world codebases:
- **PyTorch** (~600K LOC)
- **Django** (~250K LOC)
- **FastAPI** (~30K LOC)
- **Python standard library** (~500K LOC)

Comparison baselines: Pyright, ty, Pyrefly, Zuban.

---

## 17. Testing Strategy

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

## 18. Migration and Adoption

### 18.1 From mypy

1. Run `basilisk migrate --from mypy`
2. Fix BSK-E0001/E0002 errors (missing annotations) -- these are the primary diff
3. Address BSK-E0040+ (Mojo safety) or disable with `mojo-safety = false`

### 18.2 From Pyright

1. Run `basilisk migrate --from pyright`
2. If using strict mode: minimal changes needed for core type checking
3. Enable Mojo safety incrementally

### 18.3 Gradual Adoption

1. **Start**: Enable Basilisk in migration mode (errors -> warnings)
2. **Adopt per-directory**: Mark `src/` as strict, leave `legacy/` relaxed
3. **Track progress**: `basilisk stats` shows type completeness percentage
4. **Set deadline**: `deadline = "2025-12-31"` in per-path overrides
5. **Enforce**: After deadline, relaxed paths become strict

---

## 19. Governance

### 19.1 License

Apache-2.0 OR MIT dual-license for maximum corporate adoption. No CLA required. No proprietary layers.

### 19.2 Contribution Model

- Issues and PRs on GitHub
- RFC process for significant type system changes
- Monthly minor releases, quarterly major releases (semver)

### 19.3 Relationship to Python Typing Council

Basilisk follows the Python Typing Council's governance (PEP 729). We implement the typing spec as defined by the council. We participate in conformance testing. We do not extend the type system in ways that contradict the spec.

---

## 20. Roadmap

### Phase 1: Foundation
- Parser (evaluate `ruff_python_parser` vs custom)
- Name resolver
- Basic type checker (50% PEP conformance)
- CLI with human-readable output
- CI pipeline

### Phase 2: LSP and Editors
- Language server (diagnostics, hover, completions)
- VS Code extension (VSIX)
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
