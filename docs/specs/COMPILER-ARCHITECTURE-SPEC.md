# Basilisk Compiler Specification {#COMPARCH}

**Version**: 0.1.0-draft
**Status**: Specification Draft
**License**: Apache-2.0 OR MIT (dual-license)

---

## Vision {#COMPILER-VISION}

Basilisk is a compiled subset of Python. Every Basilisk program is a valid Python 3.12 program. Not every Python program is a valid Basilisk program.

The compiler takes `.py` files that pass Basilisk's type checker, lowers them to LLVM IR, and produces native machine code. You run a Basilisk script the same way you run any Python script -- but it compiles and executes as native code.

```bash
basilisk run script.py          # compile + execute (JIT)
basilisk build script.py        # compile to native binary (AOT)
python3 script.py               # still works -- it's valid Python
```

### What This Is {#COMPILER-WHAT}

- A **strict subset** of Python 3.12 that compiles to native code
- 100% PEP compliant for the features it supports
- LLVM-based: JIT for development, AOT for deployment
- Interoperable with the Python ecosystem via CPython embedding
- A single binary (`basilisk`) that checks, compiles, and runs

### What This Is Not {#COMPILER-WHATNOT}

- **Not Mojo.** No new syntax. No `fn` vs `def`. No `let` vs `var`. No MLIR. No hardware abstractions. Basilisk is Python -- the typed part.
- **Not Cython.** No `.pyx` files. No C type declarations. No mixed Python/C syntax.
- **Not Nuitka.** Does not attempt to compile all of CPython. Only compiles the statically typed subset.
- **Not PyPy.** No tracing JIT over the full interpreter. Compilation is driven by static type information.
- **Not a transpiler.** Does not emit Python, C, or Rust source. Emits LLVM IR directly.
- **Not a drop-in CPython replacement.** Untyped code and dynamic features go through the Python interpreter via interop.

### Design Thesis {#COMPILER-THESIS}

Python has two halves: the typed half and the dynamic half. The typed half -- annotated functions, typed classes, generics, protocols, pattern matching -- is a perfectly good statically typed language hiding inside a dynamically typed one. Basilisk compiles that half.

The dynamic half -- `eval`, `exec`, monkey-patching, runtime metaclasses -- stays in CPython where it belongs. Basilisk provides a clean interop boundary so you can call Python when you need to, but you never pay for dynamism you aren't using.

---

## The Basilisk Subset {#COMPILER-SUBSET}

Basilisk supports every Python 3.12 feature that can be statically typed and compiled. The boundary is simple: **if the type checker can verify it, the compiler can compile it.**

### Supported Features {#COMPILER-SUPPORTED}

All features listed here follow standard Python 3.12 semantics. For the definitive specification of each feature, see the [Python Language Reference](https://docs.python.org/3.12/reference/) and the [Python Typing Specification](https://typing.python.org/en/latest/spec/index.html).

**Functions and Control Flow**
- `def` with typed parameters and return types
- `if` / `elif` / `else`, `while`, `for`
- `match` / `case` (PEP 634)
- `return`, `yield`, `yield from`
- `break`, `continue`, `pass`
- `assert` (runtime check in debug mode, stripped in release)
- `raise`, `try` / `except` / `finally`
- `with` / `as` (context managers)
- `async def`, `await`, `async for`, `async with`
- Comprehensions: list, dict, set, generator
- Lambda expressions (with inferred types from context)
- `*args: T`, `**kwargs: T` (with typed signatures)
- Closures with captured variables

**Type System**
- All PEPs listed in [CHECKER-ARCHITECTURE-SPEC.md §CHKARCH-PEPS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-PEPS) (PEP 484, 526, 544, 585, 586, 589, 591, 604, 612, 613, 634, 646, 647, 673, 675, 681, 692, 695, 696, 698, 702, 742)
- `Union`, `Optional`, `Literal`, `Final`, `ClassVar`
- `TypeVar`, `TypeVarTuple`, `ParamSpec` (PEP 612)
- `Protocol` (structural subtyping, PEP 544)
- `TypeGuard`, `TypeIs` (PEP 647, 742)
- `Self` (PEP 673)
- `@overload`, `@override`, `@final`
- Type narrowing via `isinstance`, `is None`, truthiness, pattern matching, assertions

**Classes and Data**
- Classes with typed attributes
- Single and multiple inheritance (C3 linearization computed at compile time)
- `@dataclass`, `@dataclass(frozen=True)`
- `TypedDict`, `NamedTuple`
- `__slots__`
- `__init__`, `__repr__`, `__eq__`, `__hash__`, `__lt__`, etc.
- `@staticmethod`, `@classmethod`, `@property`
- `super()` (resolved statically from MRO)
- Descriptors with typed `__get__` / `__set__` / `__delete__`
- `Enum` and `IntEnum`

**Expressions and Operators**
- All arithmetic, comparison, logical, bitwise operators
- Augmented assignment (`+=`, `-=`, etc.)
- F-strings
- Walrus operator (`:=`, PEP 572)
- Unpacking (`a, b = ...`, `*rest = ...`)
- Subscript access with typed containers

**Modules**
- `import` and `from ... import ...` (statically resolved)
- Module-level typed variables
- `if __name__ == "__main__":` entry point

### Excluded Features {#COMPILER-EXCLUDED}

These features require the CPython interpreter. Basilisk does not compile them natively. To use them, go through the Python interop layer ([Python Interop](#COMPILER-INTEROP)).

| Feature | Why Excluded |
|---|---|
| `eval()`, `exec()`, `compile()` | Requires runtime parsing and interpretation |
| `globals()`, `locals()` as mutable dicts | Requires dynamic frame introspection |
| `setattr()` / `getattr()` with dynamic names | Cannot be statically resolved |
| `__getattr__`, `__setattr__` for dynamic dispatch | Prevents static field layout |
| Metaclasses (beyond `type`) | Runtime code execution during class creation |
| `type("Name", bases, dict)` dynamic class creation | Cannot be statically analyzed |
| Monkey-patching (adding attributes at runtime) | Violates static class layout |
| `importlib` dynamic imports | Import graph must be known at compile time |
| `sys._getframe()`, `inspect.stack()` | No Python frames in compiled code |
| `ctypes` | Use Basilisk's FFI via interop layer instead |
| Untyped code | The type system IS the compilation contract |

### Boundary Cases {#COMPILER-BOUNDARY}

| Feature | Status | Notes |
|---|---|---|
| `__init_subclass__` | Supported | Evaluated at compile time as a static hook |
| Multiple inheritance | Supported | MRO computed at compile time via C3 linearization |
| `isinstance(x, T)` | Supported | Compiled to type-tag comparison (O(1)) |
| `type(x)` | Supported | Returns compile-time type tag, not a dynamic `type` object |
| `id(x)` | Supported | Returns memory address of the object |
| Generators | Supported | Compiled to state machine structs |
| Decorators | Supported | For decorators with known semantics; custom decorators must be typed |
| `*args: T` / `**kwargs: T` | Supported | Must have typed signatures |
| `property` | Supported | Getter/setter types inferred from annotations |

---

## Compilation Pipeline {#COMPILER-PIPELINE}

The compiler extends Basilisk's existing analysis pipeline. The type checker is a hard gate -- code that fails any rule does not enter codegen.

```mermaid
graph TD
    A[".py source file"] --> B["basilisk-parser<br/>→ ParsedModule (Ruff AST)"]
    B --> C["basilisk-resolver<br/>→ ResolvedModule (symbol table)"]
    C --> D["basilisk-checker<br/>→ Vec&lt;Diagnostic&gt;"]
    D -->|"GATE: any Error stops compilation"| E["basilisk-hir<br/>→ HirModule (typed IR)"]
    E --> F["basilisk-codegen<br/>→ LLVM Module"]
    F --> G["LLVM<br/>→ machine code (JIT or object file)"]
    G --> H["Execution or native binary"]
```

For the existing parser, resolver, and checker stages, see:
- [CHECKER-ARCHITECTURE-SPEC.md §CHKARCH-ARCH](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH) for architecture details
- [CHECKER-TYPE-INFERENCE-SPEC.md](CHECKER-TYPE-INFERENCE-SPEC.md) for inference rules

### HIR Stage {#COMPILER-HIR}

The HIR (High-level Intermediate Representation) bridges analysis and codegen. It takes the `ResolvedModule` from the checker and produces a fully typed, monomorphized representation suitable for LLVM lowering.

**What the HIR does:**
- Resolves all `InferredType` variants to concrete `CompiledType` values with memory layout
- Monomorphizes generics: `list[int]` and `list[str]` become distinct concrete types
- Lowers `Union[T1, T2]` to tagged union structs
- Lowers `Protocol` to vtable interfaces
- Resolves method dispatch (static dispatch for concrete types, vtable for protocols)
- Eliminates dead code and unreachable branches
- Evaluates `Final` values and inlines constants
- Computes class layouts (field offsets, sizes, alignments)

### Codegen Stage {#COMPILER-CODEGEN}

The codegen stage translates HIR to LLVM IR using `inkwell` (safe Rust LLVM bindings).

**LLVM IR generation covers:**
- Function bodies → LLVM functions
- Class instances → LLVM structs
- Method dispatch → direct call (concrete) or vtable indirect call (protocol)
- Control flow → LLVM basic blocks with phi nodes
- Exception handling → LLVM landing pads (or setjmp/longjmp for simpler initial implementation)
- Pattern matching → switch + comparison chains
- Memory management → calls to pluggable runtime interface (incref/decref/alloc/dealloc)
- Python interop → calls into `libpython3.12` via pyo3-generated wrappers

---

## Type Representation {#COMPILER-TYPES}

Every `InferredType` from the checker maps to a concrete compiled representation. Types that cannot be fully resolved (`Any`, `Unknown`) are rejected by the checker and never reach codegen.

| Python Type | Compiled Representation | Stack/Heap | Notes |
|---|---|---|---|
| `int` | `i64` | Stack | Fixed 64-bit. Overflow traps in debug, wraps in release. `--big-int` flag for arbitrary precision. |
| `float` | `f64` | Stack | IEEE 754 double precision |
| `bool` | `i8` (storage) / `i1` (LLVM ops) | Stack | |
| `None` | Zero-size type | N/A | No runtime representation |
| `str` | `{ ptr: *u8, len: usize, cap: usize }` | Heap (data) | UTF-8, reference-counted |
| `bytes` | `{ ptr: *u8, len: usize, cap: usize }` | Heap (data) | Reference-counted |
| `list[T]` | `{ ptr: *T, len: usize, cap: usize }` | Heap (data) | Contiguous array, reference-counted |
| `dict[K, V]` | Hash map (SwissTable layout) | Heap | Reference-counted |
| `set[T]` | Hash set | Heap | Reference-counted |
| `tuple[T1, T2, ...]` | `struct { _0: T1, _1: T2, ... }` | Stack | Fixed layout, no indirection |
| `T1 \| T2` (Union) | `{ tag: u8, data: [u8; max(sizeof(T1), sizeof(T2))] }` | Stack | Tagged/discriminated union |
| `T \| None` (Optional) | Nullable pointer if T is heap-allocated; tagged union otherwise | Stack | Null pointer optimization |
| `Callable[[P], R]` | `{ fn_ptr: *fn, env_ptr: *void }` | Stack (ptrs) | Function pointer + optional closure environment |
| Class instance | Struct with fields | Heap | Reference-counted, type-tagged |
| `@dataclass(frozen=True)` | Struct with fields | Stack (if small) | Value type, no refcount needed if stack-allocated |
| `Never` | LLVM `unreachable` | N/A | |
| `Literal[42]` | Compile-time constant | Inlined | Narrowed to base type at codegen |
| `type[T]` | Type tag (integer constant) | Stack | For `isinstance` checks |

### Integer Semantics {#COMPILER-TYPES-INT}

By default, Basilisk integers are 64-bit signed (`i64`). This diverges from Python's arbitrary-precision integers but matches what most code actually needs.

- Arithmetic overflow traps in debug mode (like Rust), wraps in release mode
- The `--big-int` flag enables arbitrary-precision integers backed by GMP, matching Python's semantics exactly
- Integer literals that exceed `i64` range are a compile error unless `--big-int` is enabled

### String Semantics {#COMPILER-TYPES-STR}

Strings are UTF-8 encoded, matching Python 3's `str` type. Operations follow Python's string semantics:

- Indexing (`s[i]`) returns a single character (one-element `str`, not a code point integer)
- Slicing (`s[i:j]`) returns a new string (copy-on-write optimization possible)
- `len(s)` returns the number of Unicode code points (not bytes)
- Concatenation allocates a new string (or mutates in-place when refcount == 1)

---

## Memory Model {#COMPILER-MEMORY}

Basilisk's memory management is **pluggable**. The compiler emits calls to a runtime memory interface -- the implementation behind that interface is swappable. This means the same compiled code can run under different memory management strategies without recompilation of user code (only relinking against a different runtime).

The default is **CPython-style**: reference counting with a cyclic garbage collector. This is what Python developers already know. No surprises.

### Memory Interface {#COMPILER-MEMORY-IFACE}

The runtime exposes a standard interface that all memory backends implement:

```
alloc(size, type_tag) -> *void       # allocate a new object
dealloc(ptr)                         # free an object
incref(ptr)                          # increment reference count (no-op for tracing GC)
decref(ptr)                          # decrement reference count (no-op for tracing GC)
collect()                            # run cycle collection / GC pass
```

The codegen emits calls to these functions. The backend is selected at link time. User code does not change.

```bash
basilisk build --gc=refcount      # default: CPython-style (refcount + cyclic GC)
basilisk build --gc=arc           # ARC-only: no cycle collector, fastest, leaks cycles
basilisk build --gc=tracing       # tracing GC: throughput-optimized, non-deterministic __del__
basilisk build --gc=arena         # arena: bump allocator, free-all-at-once, ideal for CLI tools
```

### Default Backend {#COMPILER-MEMORY-DEFAULT}

**This is the only backend implemented initially.** All other backends are future work. The interface exists from day one so that adding new backends is a matter of implementing the trait, not restructuring the compiler.

The default backend matches CPython's memory semantics:

- Every heap object has a reference count header
- Assignment increments the reference count
- Scope exit or reassignment decrements the reference count
- When the reference count reaches zero, the object is destroyed immediately
- `__del__` methods (if present) are called deterministically at destruction time (same as CPython)

**Cyclic garbage collector:**

- Objects that **can** form cycles (contain references to other heap objects) are tracked in a generation list
- Objects that **cannot** form cycles (e.g., `list[int]`, `dict[str, float]`, primitives) are exempt -- their refcount alone is sufficient
- The cycle collector runs when the tracked object count exceeds a generation threshold
- Uses the trial-deletion algorithm (same approach as CPython's `gc` module)
- Three generations, same as CPython: generation 0 (young), generation 1, generation 2 (old)

This means Basilisk programs behave identically to CPython with respect to object lifetime and `__del__` ordering for acyclic objects. Cyclic objects are collected by the same algorithm. No behavioral surprises when porting from Python.

### Future Backends {#COMPILER-MEMORY-FUTURE}

| Backend | Flag | Trade-off | Best For |
|---|---|---|---|
| **ARC-only** | `--gc=arc` | Fastest, deterministic, but leaks reference cycles | Scripts, pipelines, acyclic data (trees, arrays, configs) |
| **Tracing GC** | `--gc=tracing` | Higher throughput, no refcount overhead per assignment, but `__del__` timing is non-deterministic | Long-running servers, many short-lived allocations |
| **Arena** | `--gc=arena` | Bump allocation, free everything at once, fastest possible allocation | CLI tools, request handlers, short-lived programs |

These are documented here for the interface design. Implementation is deferred.

### Stack Allocation {#COMPILER-MEMORY-STACK}

Regardless of backend, values that don't escape their scope are stack-allocated:

- Primitives: `int`, `float`, `bool`, `None`
- Small tuples: `tuple[int, int]`, `tuple[str, float]`
- Small frozen dataclasses
- Tagged unions where all variants are stack-sized

The compiler performs escape analysis: if a value is never stored into a heap object, passed to a function that stores it, or returned, it stays on the stack. Stack-allocated values bypass the memory interface entirely.

### Ownership Annotations {#COMPILER-MEMORY-OWNERSHIP}

The ownership annotations from [CHECKER-ARCHITECTURE-SPEC.md §CHKARCH-MOJO-OWNERSHIP](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-MOJO-OWNERSHIP) (`Borrowed`, `Owned`, `InOut`) are optimization hints that work across **all** memory backends:

| Annotation | Compiler Effect |
|---|---|
| `Borrowed` (default) | No refcount increment at call boundary. Caller guarantees the value outlives the call. |
| `Owned` | Caller transfers ownership. No increment. Callee responsible for eventual decrement. Enables move semantics. |
| `InOut` | Exclusive mutable borrow. No refcount change. Compiler can optimize in-place mutation. |
| (no annotation) | Same as `Borrowed` -- immutable parameter, refcount elided when provably safe. |

These annotations are optional. Without them, the compiler conservatively increments/decrements reference counts at call boundaries. With them, the compiler can elide unnecessary operations -- regardless of which backend is active.

---

## Object Layout and Class Compilation {#COMPILER-LAYOUT}

### Class Layout {#COMPILER-LAYOUT-CLASS}

A class compiles to a struct with:
1. **Type tag** (u64): identifies the concrete type for `isinstance` checks
2. **Refcount** (usize): reference count for ARC
3. **Vtable pointer** (optional): only for classes used through `Protocol` interfaces
4. **Fields**: in declaration order, with alignment padding

```python
class Point:
    x: float
    y: float

    def __init__(self, x: float, y: float) -> None:
        self.x = x
        self.y = y
```

Compiles to:
```
struct Point {
    _type_tag: u64,      // 8 bytes
    _refcount: usize,    // 8 bytes
    x: f64,              // 8 bytes
    y: f64,              // 8 bytes
}
// Total: 32 bytes, alignment: 8
```

### Inheritance {#COMPILER-LAYOUT-INHERIT}

**Single inheritance**: child struct embeds parent struct as first field, enabling pointer casting.

**Multiple inheritance**: fields are flattened according to C3 MRO, computed at compile time. Each base class gets a vtable for its methods. Method calls on a base type use the corresponding vtable offset.

### Protocols {#COMPILER-LAYOUT-PROTOCOLS}

`Protocol` types compile to vtable interfaces, similar to Rust's `dyn Trait`:

```python
class Printable(Protocol):
    def display(self) -> str: ...

def show(item: Printable) -> None:
    print(item.display())
```

`Printable` compiles to a vtable struct:
```
struct PrintableVtable {
    display: fn(*void) -> String,
}
```

Any class implementing `display(self) -> str` satisfies `Printable` at compile time. The vtable is constructed per concrete type and passed alongside the data pointer (fat pointer).

### isinstance {#COMPILER-LAYOUT-ISINSTANCE}

`isinstance(x, T)` compiles to a comparison of the object's type tag against `T`'s known tag constant. This is O(1). For inheritance hierarchies, each class stores its full ancestor chain as a compile-time constant, and `isinstance` checks against the chain.

---

## Python Interop {#COMPILER-INTEROP}

Basilisk interoperates with CPython in both directions. Interop is explicit and typed at the boundary.

### Project Layout {#COMPILER-INTEROP-LAYOUT}

All Basilisk source files use the `.py` extension. They are valid Python. The distinction between compiled Basilisk code and interpreted Python code is determined by **folder convention**, not file extension.

```
myproject/
  src/              # Basilisk code -- compiled to native
    main.py
    core/
      engine.py
      models.py
  interop/          # Python code -- runs via CPython interpreter
    legacy.py
    scrapers.py
  pyproject.toml
```

Configuration in `pyproject.toml`:

```toml
[tool.basilisk]
compile = ["src/"]          # compiled to native code
interop = ["interop/"]      # stays in CPython, accessed via interop layer
```

**Why not a `.bsk` extension?**
- Basilisk files ARE valid Python -- renaming them breaks `python3 script.py` compatibility
- Standard Python tooling (Ruff, IDEs, pytest, syntax highlighting) works on `.py` files out of the box
- No ecosystem fragmentation

**How it works:**
- Imports within `compile` directories are native calls (zero overhead)
- Imports from `interop` directories cross the interop boundary automatically (value conversion at the call site)
- Imports from installed third-party packages follow the stub strategy: if the package passes the Basilisk checker, it can be compiled; otherwise it goes through interop (see [stub-strategy.md](stub-strategy.md))

**Gradual migration:** move files from `interop/` to `src/` (or whichever `compile` directory) as you add type annotations and fix checker errors. The boundary moves with you.

**Per-file override** for edge cases:

```python
# basilisk: compile
# Forces this file to be compiled even if it's outside the compile directories
```

```python
# basilisk: interop
# Forces this file to go through CPython even if it's inside a compile directory
```

### Calling Python from Basilisk {#COMPILER-INTEROP-PY2BSK}

Basilisk embeds `libpython3.12` via [pyo3](https://pyo3.rs) (Rust bindings to CPython). Imports from `interop` directories or untyped third-party packages go through this layer automatically.

```python
# src/main.py (compiled)
from interop.legacy import process_data  # crosses interop boundary automatically

def run(data: list[str]) -> int:
    result = process_data(data)  # Python interop call
    return int(result)           # convert back to Basilisk int
```

**At the interop boundary:**
- Basilisk values are converted to `PyObject*` before crossing
- Python return values are converted back to Basilisk types
- The return type must be annotated -- no implicit `Any`
- If the Python call returns a value that doesn't match the annotation, a runtime error is raised

**Conversion table:**

| Basilisk Type | Python Type | Conversion |
|---|---|---|
| `int` | `int` | Direct (i64 ↔ PyLong) |
| `float` | `float` | Direct (f64 ↔ PyFloat) |
| `bool` | `bool` | Direct |
| `str` | `str` | UTF-8 ↔ PyUnicode |
| `bytes` | `bytes` | Buffer copy |
| `list[T]` | `list` | Element-wise conversion |
| `dict[K, V]` | `dict` | Key/value-wise conversion |
| `None` | `None` | Direct |

### Calling Basilisk from Python {#COMPILER-INTEROP-BSK2PY}

Compiled Basilisk modules can be exported as CPython extension modules (`.so` / `.pyd`):

```python
# In Basilisk: mark functions for export
def fibonacci(n: int) -> int:  # automatically exportable -- it's typed
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)
```

```bash
basilisk build --lib fib.py    # produces fib.cpython-312-*.so
```

```python
# In Python:
import fib
print(fib.fibonacci(40))       # calls compiled native code
```

The compiler generates CPython C API wrappers (PEP 384 stable ABI) for all public typed functions and classes. This enables gradual adoption: compile hot paths to Basilisk, import them from Python.

### Compiling Typed Python Libraries {#COMPILER-INTEROP-LIBS}

A PEP-compliant Python library that passes Basilisk's type checker can be compiled to a native Basilisk library:

```bash
basilisk build --lib /path/to/typed-library/
```

This produces a native shared library (`.bsk.so` / `.bsk.dylib`) that can be linked directly into a Basilisk program -- no CPython overhead at the call boundary.

**Requirements for native compilation:**
- The library must pass all Basilisk checker rules (no `Any` escapes, no untyped functions)
- All dependencies must either be natively compiled or available via interop
- The library must not use any excluded features ([Excluded Features](#COMPILER-EXCLUDED))

**For libraries with stubs but dynamic implementations**, see [stub-strategy.md](stub-strategy.md). Libraries that have type stubs but use dynamic features internally cannot be natively compiled -- they go through the interop layer.

---

## Runtime Library {#COMPILER-RUNTIME}

The Basilisk runtime (`basilisk-runtime`) provides the minimal foundation needed by compiled code. It is a Rust crate linked into every Basilisk binary.

### Core Runtime Components {#COMPILER-RUNTIME-CORE}

| Component | Responsibility |
|---|---|
| **Memory** | Pluggable memory interface: alloc, dealloc, incref, decref, collect. Default backend: CPython-style refcount + cyclic GC (see [Memory Model](#COMPILER-MEMORY)) |
| **Allocator** | Memory allocation interface (defaults to system allocator, swappable) |
| **Strings** | UTF-8 string operations: concatenation, slicing, formatting, f-string evaluation |
| **Collections** | Native `list`, `dict`, `set` implementations with type-specialized layouts |
| **Exceptions** | Exception type hierarchy, raise/catch machinery, stack trace construction |
| **Builtins** | `print`, `len`, `range`, `enumerate`, `zip`, `map`, `filter`, `sorted`, `reversed`, `min`, `max`, `sum`, `abs`, `round`, `hash`, `id`, `type`, `isinstance`, `issubclass`, `repr`, `str`, `int`, `float`, `bool`, `list`, `dict`, `set`, `tuple`, `iter`, `next`, `open`, `input` |
| **Assertions** | `assert` compiled to conditional trap in debug mode |
| **Panic** | Unrecoverable error handling (integer overflow in debug, failed type assertions at interop boundary) |

### No GIL {#COMPILER-RUNTIME-NOGIL}

Basilisk does not have a Global Interpreter Lock. Statically typed code does not need runtime type dispatch or reference safety guards. True parallelism is available through typed concurrency primitives.

### Exception Implementation {#COMPILER-RUNTIME-EXCEPTIONS}

Exceptions are compiled to LLVM landing pads (zero-cost when no exception is thrown):

- `raise` generates an LLVM `invoke` to a runtime raise function
- `try` / `except` generates a landing pad that catches and dispatches by exception type
- `finally` generates cleanup code in both normal and exceptional paths
- Exception types are represented as tagged structs, `isinstance` dispatch is O(1)
- Stack traces are constructed from DWARF debug info in debug builds

---

## Standard Library Strategy {#COMPILER-STDLIB}

The standard library is available in three tiers:

### Tier 1: Native Builtins {#COMPILER-STDLIB-T1}

Implemented in the Basilisk runtime (Rust) or compiled from Basilisk source. Full native speed, no CPython dependency.

`builtins`, `math`, `os.path`, `sys` (subset: `argv`, `exit`, `platform`, `version`), `collections` (`deque`, `defaultdict`, `Counter`, `OrderedDict`), `itertools`, `functools` (subset: `reduce`, `partial`, `lru_cache`), `typing`, `dataclasses`, `enum`, `json`, `re` (via Rust `regex` crate), `pathlib`, `datetime`, `hashlib`, `struct`, `io` (subset: file read/write), `string`, `textwrap`, `copy` (`copy`, `deepcopy`)

### Tier 2: Wrapper Builtins {#COMPILER-STDLIB-T2}

Thin Basilisk wrappers around OS or C libraries. Native speed for the computation, syscall overhead for OS operations.

`os`, `socket`, `threading`, `subprocess`, `csv`, `tempfile`, `shutil`, `signal`, `select`, `mmap`

### Tier 3: Python Interop {#COMPILER-STDLIB-T3}

Everything not in Tier 1 or Tier 2 goes through CPython embedding. This includes all third-party packages.

`importlib`, `inspect`, `ast`, `asyncio` (initially -- native implementation planned), `logging`, `unittest`, `argparse`, `http`, `urllib`, `email`, `xml`, `sqlite3`, `ctypes`, and all third-party packages

---

## CLI Interface {#COMPILER-CLI}

The existing `basilisk check` and `basilisk lsp` commands remain unchanged. New commands for compilation:

```bash
# Run (JIT compile and execute)
basilisk run script.py               # compile and run
basilisk run script.py -- arg1 arg2  # pass args to script

# Build (AOT compile to binary)
basilisk build script.py             # produce native binary
basilisk build -o myapp src/         # compile project to binary
basilisk build --lib src/            # compile to shared library (.so/.dylib/.dll)
basilisk build --cpython-ext src/    # compile to CPython extension module

# Existing commands
basilisk check src/                  # type check only
basilisk lsp                         # language server
```

### Flags {#COMPILER-CLI-FLAGS}

| Flag | Description |
|---|---|
| `--opt-level=0\|1\|2\|3` | LLVM optimization level (default: 0 for `run`, 2 for `build`) |
| `--debug` | Include DWARF debug info, enable assertions, integer overflow traps |
| `--release` | `-O2`, strip debug info, disable assertions, integer overflow wraps |
| `--target=<triple>` | Cross-compilation target (e.g., `x86_64-unknown-linux-gnu`) |
| `--emit=llvm-ir\|asm\|obj` | Emit intermediate artifacts instead of final binary |
| `--python-path=<path>` | Python interpreter for interop (default: `python3`) |
| `--no-interop` | Fail on any code that requires CPython (pure Basilisk mode) |
| `--big-int` | Use arbitrary-precision integers instead of i64 |
| `--gc=refcount\|arc\|tracing\|arena` | Memory management backend (default: `refcount`). See [Memory Model](#COMPILER-MEMORY). |

### Exit Codes {#COMPILER-CLI-EXIT}

| Code | Meaning |
|---|---|
| 0 | Success (check clean, or program exited 0) |
| 1 | Type errors found (check failed, compilation refused) |
| 2 | Configuration error |
| 3 | Internal compiler error |
| N | Program exit code (for `basilisk run`) |

---

## Compilation Modes {#COMPILER-MODES}

### JIT Mode {#COMPILER-MODES-JIT}

For development. Parse, check, lower to HIR, generate LLVM IR, JIT-compile, and execute immediately.

- Uses LLVM's ORC JIT engine
- Module-level code executes as soon as it's compiled
- Compiled modules are cached in `.basilisk/cache/` keyed by content hash
- Subsequent runs skip compilation for unchanged modules
- **Startup target**: < 100ms for a small script (after cache warm-up)

### AOT Mode {#COMPILER-MODES-AOT}

For deployment. Full ahead-of-time compilation to a native binary or shared library.

- Whole-program analysis: monomorphization, dead code elimination, cross-module inlining
- Links against `basilisk-runtime` (static by default)
- If interop is used, links against `libpython3.12` (dynamic)
- Output: standalone binary, shared library, or CPython extension module
- Cross-compilation supported via LLVM target triples

### Caching {#COMPILER-MODES-CACHE}

Both modes use content-addressed caching:
- Each module is hashed (source content + compiler version + flags)
- Compiled LLVM IR and object files are cached in `.basilisk/cache/`
- Cache invalidation is automatic when source changes
- `basilisk cache clear` to manually flush

---

## New Crates {#COMPILER-CRATES}

| Crate | Purpose | Key Dependencies |
|---|---|---|
| `basilisk-hir` | High-level typed IR: monomorphized types, resolved layouts, typed AST | `basilisk-checker`, `basilisk-resolver` |
| `basilisk-codegen` | LLVM IR generation from HIR | `basilisk-hir`, `inkwell` (safe LLVM bindings) |
| `basilisk-runtime` | ARC, strings, collections, builtins, exceptions | Minimal: libc, allocator |
| `basilisk-interop` | CPython embedding and value conversion | `pyo3`, `basilisk-runtime` |

### Dependency Graph {#COMPILER-CRATES-DEPS}

```
basilisk-cli
  |
  +-- basilisk-parser
  +-- basilisk-resolver  (depends on basilisk-parser)
  +-- basilisk-checker   (depends on basilisk-resolver)
  +-- basilisk-hir       (depends on basilisk-checker)   [NEW]
  +-- basilisk-codegen   (depends on basilisk-hir)       [NEW]
  +-- basilisk-runtime   (standalone)                    [NEW]
  +-- basilisk-interop   (depends on basilisk-runtime)   [NEW]
  +-- basilisk-lsp       (depends on basilisk-checker)
```

No circular dependencies. The existing `parser → resolver → checker` chain is extended with `→ hir → codegen`. The runtime and interop crates are standalone.

---

## Phased Implementation Roadmap {#COMPILER-ROADMAP}

| Phase | Milestone | What Compiles |
|---|---|---|
| **C1: Hello World** | First compiled output | `print("hello")`, integer arithmetic, string literals, `if`/`else`, `while`, `for range()` |
| **C2: Functions** | Function calls | `def` with typed params/returns, local variables, recursion, basic closures |
| **C3: Classes** | Object-oriented code | Classes, single inheritance, methods, `@dataclass`, `__init__`, `isinstance` |
| **C4: Collections** | Data structures | Native `list[T]`, `dict[K,V]`, `set[T]`, `tuple[T1,T2,...]` |
| **C5: Generics** | Generic code | Monomorphization of `TypeVar`-based functions and classes |
| **C6: Exceptions** | Error handling | `try`/`except`/`finally`, exception hierarchy, `raise` |
| **C7: Interop** | Call Python libraries | CPython embedding via pyo3, value conversion, export as extension modules |
| **C8: Stdlib** | Useful programs | Native builtins, `os`, `json`, `re`, `pathlib`, `datetime` |
| **C9: AOT** | Production binaries | Whole-program optimization, cross-compilation, release builds |
| **C10: Async** | Async code | `async`/`await` compiled to coroutine state machines |

---

## Performance Targets {#COMPILER-PERF}

| Benchmark | Target vs C/Rust | CPython Comparison | Notes |
|---|---|---|---|
| Fibonacci(40) recursive | Within 2x of C | ~100x faster than CPython | Tests function call overhead |
| String processing (100MB) | Within 3x of Rust | ~50x faster than CPython | Tests string allocation and iteration |
| JSON parsing | Within 2x of serde_json | ~20x faster than CPython json | Tests dict/list construction |
| List comprehension (1M elements) | Within 2x of Rust Vec | ~30x faster than CPython | Tests collection allocation |
| Startup time (hello world) | < 50ms | CPython ~30ms | JIT compilation + execution |
| Binary size (hello world) | < 5MB | N/A | Statically linked with runtime |
| Compilation speed | < 1s for 10K LOC | N/A | Incremental: < 100ms for single file change |

---

## Testing Strategy {#COMPILER-TESTING}

The compiler is tested end-to-end. We write a `.py` file, compile and run it, and match the output against an expected output file. This is the primary testing layer -- it proves the whole pipeline works from source to execution.

### E2E Test Convention {#COMPILER-TESTING-E2E}

Test fixtures live in `crates/basilisk-compiler/tests/e2e/`. Each test is a pair of files:

```
crates/basilisk-compiler/tests/e2e/
  hello-expectedoutput.txt        # expected stdout
  hello.py                        # input program
  arithmetic-expectedoutput.txt
  arithmetic.py
  functions-expectedoutput.txt
  functions.py
  ...
```

**The input file** is a valid Basilisk (typed Python) program:

```python
# crates/basilisk-compiler/tests/e2e/hello.py
def main() -> None:
    print("hello, world")

main()
```

**The expected output file** is the exact stdout the program should produce:

```
# crates/basilisk-compiler/tests/e2e/hello-expectedoutput.txt
hello, world
```

The test runner compiles each `.py` file, executes the resulting binary, captures stdout, and asserts it matches the corresponding `-expectedoutput.txt` file byte-for-byte.

### Examples {#COMPILER-TESTING-EXAMPLES}

**Arithmetic:**

```python
# arithmetic.py
def add(a: int, b: int) -> int:
    return a + b

def multiply(x: int, y: int) -> int:
    return x * y

print(add(2, 3))
print(multiply(4, 5))
print(add(multiply(2, 3), 4))
```

```
# arithmetic-expectedoutput.txt
5
20
10
```

**Control flow:**

```python
# controlflow.py
def fizzbuzz(n: int) -> str:
    if n % 15 == 0:
        return "FizzBuzz"
    elif n % 3 == 0:
        return "Fizz"
    elif n % 5 == 0:
        return "Buzz"
    else:
        return str(n)

for i in range(1, 16):
    print(fizzbuzz(i))
```

```
# controlflow-expectedoutput.txt
1
2
Fizz
4
Buzz
Fizz
7
8
Fizz
Buzz
11
Fizz
13
14
FizzBuzz
```

**Classes:**

```python
# classes.py
class Point:
    x: float
    y: float

    def __init__(self, x: float, y: float) -> None:
        self.x = x
        self.y = y

    def distance(self) -> float:
        return (self.x ** 2 + self.y ** 2) ** 0.5

p = Point(3.0, 4.0)
print(p.distance())
```

```
# classes-expectedoutput.txt
5.0
```

### Test Layers {#COMPILER-TESTING-LAYERS}

The compiler follows the same testing philosophy as the analyzer (see [CHECKER-ARCHITECTURE-SPEC.md §CHKARCH-TESTING](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING)):

| Layer | Location | What It Tests |
|---|---|---|
| **E2E** | `crates/basilisk-compiler/tests/e2e/*.py` | Compile + run + match output. The thing that actually matters. |
| **Integration** | `crates/basilisk-hir/tests/`, `crates/basilisk-codegen/tests/` | HIR lowering and LLVM IR generation for specific constructs |
| **Unit** | `#[cfg(test)]` modules inside crate source files | Narrow logic only -- type layout computation, ARC elision decisions |

E2E tests are the foundation. If a feature doesn't have an E2E test with expected output, it doesn't work.

### Failure Tests {#COMPILER-TESTING-FAILURES}

Tests that should **fail to compile** use a `-expectederror.txt` file instead:

```python
# untyped-param.py
def greet(name):  # missing type annotation
    print(f"hello {name}")
```

```
# untyped-param-expectederror.txt
BSK-E0001
```

The test runner asserts that compilation fails and the error output contains the expected error code.

---

## References {#COMPILER-REFERENCES}

- [CHECKER-ARCHITECTURE-SPEC.md §CHKARCH-TYPESYS](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TYPESYS) -- Basilisk type system specification
- [CHECKER-TYPE-INFERENCE-SPEC.md](CHECKER-TYPE-INFERENCE-SPEC.md) -- Type inference rules
- [CHECKER-STUB-RESOLUTION-SPEC.md](CHECKER-STUB-RESOLUTION-SPEC.md) -- Stub resolution and type provenance
- [Python Language Reference (3.12)](https://docs.python.org/3.12/reference/)
- [Python Typing Specification](https://typing.python.org/en/latest/spec/index.html)
- [PEP Conformance Suite](https://github.com/python/typing/blob/main/conformance/README.md)
- [LLVM Language Reference](https://llvm.org/docs/LangRef.html)
- [inkwell -- Safe LLVM bindings for Rust](https://github.com/TheDan64/inkwell)
- [pyo3 -- Rust bindings for CPython](https://pyo3.rs)
