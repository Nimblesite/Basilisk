# Basilisk Type Inference Specification

> **Status**: Draft — Basilisk is in the specification stage. This document defines the target behavior for the type inference engine.
>
> **Canonical Python version**: 3.12
>
> **Authoritative references**: [PEP 484](https://peps.python.org/pep-0484/), [PEP 526](https://peps.python.org/pep-0526/), [Python Typing Spec](https://typing.readthedocs.io/en/latest/), [Python Typing Conformance Suite](https://github.com/python/typing/tree/main/conformance)

---

## 0. Design Philosophy

Basilisk's type inference is **strict by default and bidirectional throughout**. Where Pyright makes inference optional or limits it to avoid false positives, Basilisk demands more from both the programmer and itself. Where Pyright falls back to `Unknown` or `Any`, Basilisk either produces a precise type or emits a diagnostic.

Key design decisions that make Basilisk **more advanced than Pyright**:

| Capability | Pyright | Basilisk |
|---|---|---|
| Unannotated parameter types | `Unknown` / infer from default | **Error** — all params must be annotated |
| Return type inference | Inferred silently | Inferred **and** validated; mismatch is an error |
| Container inference | `Unknown` for heterogeneous (loose mode) | **Union** always — `strictListInference` is always on |
| TypeVar constraint solving | Heuristic, bounded recursion | **Bidirectional constraint propagation** with exhaustive solving |
| Literal type inference | Context-sensitive | Literal-first: widen only when an annotation demands it |
| Narrowing coverage | `isinstance`, `is None`, TypeGuard, TypeIs | All of the above + **pattern matching exhaustiveness**, **dict key existence**, **attribute presence** |
| Unannotated functions | Checked via call-site inference | **Error** — every public function must be annotated |

---

## 1. Governing PEPs

The following PEPs define the ground truth for Basilisk's inference rules. All are required reading for implementors.

| PEP | Title | Relevant to inference |
|---|---|---|
| [PEP 484](https://peps.python.org/pep-0484/) | Type Hints | Core type system, TypeVar, inference rules |
| [PEP 526](https://peps.python.org/pep-0526/) | Variable Annotations | Annotated variable inference, `ClassVar`, `Final` |
| [PEP 544](https://peps.python.org/pep-0544/) | Protocols | Structural subtyping and inferred protocol satisfaction |
| [PEP 572](https://peps.python.org/pep-0572/) | Walrus Operator | Type of `:=` expression |
| [PEP 585](https://peps.python.org/pep-0585/) | Generic Built-in Types | `list[int]` etc. at runtime |
| [PEP 604](https://peps.python.org/pep-0604/) | `X \| Y` Union Syntax | Union inference from `\|` expressions |
| [PEP 612](https://peps.python.org/pep-0612/) | ParamSpec | Callable parameter list capture |
| [PEP 634](https://peps.python.org/pep-0634/) | Structural Pattern Matching | Match/case type narrowing |
| [PEP 647](https://peps.python.org/pep-0647/) | TypeGuard | User-defined type narrowing |
| [PEP 655](https://peps.python.org/pep-0655/) | `Required`/`NotRequired` | TypedDict field requiredness |
| [PEP 673](https://peps.python.org/pep-0673/) | Self Type | `Self` in methods |
| [PEP 675](https://peps.python.org/pep-0675/) | LiteralString | String literal type |
| [PEP 681](https://peps.python.org/pep-0681/) | dataclass_transform | Framework-defined dataclass semantics |
| [PEP 695](https://peps.python.org/pep-0695/) | Type Parameter Syntax | `[T]` generic syntax, `type` statement, inferred variance |
| [PEP 696](https://peps.python.org/pep-0696/) | TypeVar Defaults | `TypeVar("T", default=int)` |
| [PEP 698](https://peps.python.org/pep-0698/) | `@override` | Enforced override checking |
| [PEP 705](https://peps.python.org/pep-0705/) | `ReadOnly` for TypedDict | Immutable TypedDict fields |
| [PEP 728](https://peps.python.org/pep-0728/) | TypedDict Extra Items | `extra_items` parameter |
| [PEP 742](https://peps.python.org/pep-0742/) | TypeIs | Bidirectional narrowing (Python 3.13+) |

---

## 2. Type Inference Overview

### 2.1 What Is Inferred

Basilisk infers types for:

- **Local variable assignments** — `x = 42` → `x: int`
- **Return types** — from the union of all `return` expression types (see §5)
- **Container literals** — list, dict, set, tuple elements (see §6)
- **`self` and `cls`** — always inferred, never annotated (see §4.4)
- **Walrus operator** — `(x := expr)` has the same type as `expr`
- **Comprehensions** — element type from the expression, collection type from the form
- **Generic instantiation** — `list[int]()` → `list[int]`; `Foo(x)` → `Foo[T]` solved from `x` (see §8)
- **Narrowed types** — after guards (see §9)

### 2.2 What Is Never Inferred (Must Be Annotated)

Basilisk **requires explicit annotations** for:

- **All function parameters** (except `self`, `cls`) — E0001 if missing
- **All public function return types** — E0002 if missing
- **All class-level attributes** at the class body level — E0003 if missing
- **TypedDict fields**, `NamedTuple` fields, `Protocol` members — always explicit

This is the fundamental difference from Pyright: Basilisk does not silently fall back to `Unknown`. A missing annotation is a **diagnostic**, not an inference opportunity.

### 2.3 Inference Algorithm

Basilisk uses a **bidirectional type inference** algorithm:

```
infer_type(expr, expected: Option<Type>) -> Type
```

The `expected` parameter ("expected type" or "pushed type") flows from outer context into inner expressions. When present, it guides inference of ambiguous constructs (empty containers, lambdas, integer literals that could be `Literal[N]`).

This is the same approach as described in the [bidirectional typing literature](https://www.cl.cam.ac.uk/~nk480/bidir.pdf) (Pierce & Turner, "Local Type Inference", 2000) and implemented in Pyright's `inferTypeForExpression` with expected-type context.

---

## 3. Variable Type Inference

### 3.1 Simple Assignment

```python
x = 42          # int
y = "hello"     # str
z = 3.14        # float
b = True        # bool
n = None        # None
```

**Literal inference rule**: By default Basilisk infers the **most specific type** (literal) for constants assigned at module scope or class scope. Within function bodies, literals are widened to their base types unless the value is used in a literal-sensitive context.

```python
# Module scope — literal inference
STATUS = "active"   # Literal["active"]
MAX = 100           # Literal[100]

# Function body — widened
def f() -> None:
    x = "active"   # str  (widened — no need for Literal in local scope)
    y: Literal["active"] = "active"  # Literal["active"] (annotation drives it)
```

This is stricter than Pyright, which applies literal inference uniformly and sometimes infers `Literal` in local scopes. Basilisk reserves `Literal` inference for module/class constants.

### 3.2 Multiple Assignment (Flow Union)

When a variable is assigned in multiple branches, the inferred type is the **union** of all assigned types:

```python
def f(cond: bool) -> None:
    if cond:
        x = 1       # int
    else:
        x = "hi"    # str
    reveal_type(x)  # int | str
```

This follows [PEP 484 §Union types](https://peps.python.org/pep-0484/#union-types) and the Python typing spec on [variable type narrowing](https://typing.readthedocs.io/en/latest/spec/narrowing.html).

### 3.3 Annotated Variable

When an annotation is present, the annotation **is** the declared type. The inferred RHS type must be assignable to it:

```python
x: int = 42         # declared: int; RHS infers int ✓
y: float = 42       # declared: float; RHS infers int; int is subtype of float ✓
z: str = 42         # BSK-E0010: int is not assignable to str
```

> **Authority**: [PEP 526 §Annotated assignment statements](https://peps.python.org/pep-0526/#annotated-assignment-statements):
> "If a variable has been annotated, all assignments to that variable will be type-checked."

### 3.4 Augmented Assignment

```python
x = 1
x += 2   # still int — calls __iadd__ or __add__, return type drives x's new type
```

The type of `x` after `x op= rhs` is the return type of `type(x).__iadd__(rhs)` (or `__add__` if `__iadd__` is absent). If the return type differs from `x`'s current type, the narrowed type applies.

### 3.5 Walrus Operator (PEP 572)

```python
if (n := len(a)) > 10:
    reveal_type(n)  # int
```

The walrus operator `:=` assigns the value and the **expression type equals the assigned type**. The variable `n` is in scope for the remainder of the enclosing scope (not just the `if` block).

> **Authority**: [PEP 572](https://peps.python.org/pep-0572/): "The value of the target is the same as the value of the value expression."

---

## 4. Function Type Inference

### 4.1 Parameters

**All parameters must be explicitly annotated.** There are no exceptions for public API functions. This fires `BSK-E0001`.

```python
def process(data):          # BSK-E0001: parameter 'data' has no type annotation
    pass

def process(data: bytes):   # ✓
    pass
```

The **only parameters that are inferred rather than annotated** are:

- `self` in instance methods → inferred as `Self` (the containing class bound to `Self`)
- `cls` in class methods → inferred as `type[Self]`
- `__` (positional-only placeholder) → accepted as `Any` for compatibility

> **Authority**: [PEP 673 (Self type)](https://peps.python.org/pep-0673/) for `Self` semantics.
> Pyright docs: "The `self` parameter in instance methods is inferred as the containing class type using the `Self` type."

### 4.2 Default Parameters

When a parameter has no annotation but has a default, Basilisk **does not** infer the type from the default. The annotation is still required. This is stricter than Pyright, which infers `param: type(default)` for unannotated defaulted parameters.

```python
def connect(timeout=30):        # BSK-E0001 — annotation required even with default
    pass

def connect(timeout: int = 30): # ✓
    pass
```

### 4.3 Return Types

Return types are **inferred from the function body** but an annotation is required for all non-trivial public functions (those that are not `-> None` trivially).

The inferred return type is the **union of all `return` expression types**:

```python
def f(x: int) -> int | str:    # ✓ — annotation matches inference
    if x > 0:
        return x               # int
    return "negative"          # str
```

If the annotated return type is **narrower** than the inferred union, Basilisk emits `BSK-E0011` (return type mismatch).

If the function body has **no reachable `return` statement** and does not raise, the inferred return type is `None`. If the function always raises, the inferred return type is `Never`.

```python
def always_raises() -> Never:
    raise RuntimeError("always")    # inferred: Never ✓

def sometimes_returns(x: int) -> int | None:
    if x > 0:
        return x    # int
    # implicit return None → None
```

> **Authority**: [PEP 484 §The `NoReturn` type](https://peps.python.org/pep-0484/#the-noreturn-type).

### 4.4 `self` and `cls` Inference

| Parameter | Context | Inferred type |
|---|---|---|
| `self` | Instance method | `Self` (bound to the class) |
| `cls` | Class method (`@classmethod`) | `type[Self]` |
| `cls` | `__init_subclass__` | `type[Self]` |
| `mcs` or `cls` | Metaclass `__new__`/`__init__` | `type[Self]` |

`Self` participates in inheritance correctly: a subclass calling an inherited method infers the subclass type, not the base class type.

```python
class Builder:
    def set_name(self, name: str) -> Self:
        self._name = name
        return self

class AdvancedBuilder(Builder):
    pass

b = AdvancedBuilder().set_name("x")
reveal_type(b)  # AdvancedBuilder — not Builder
```

> **Authority**: [PEP 673](https://peps.python.org/pep-0673/).

### 4.5 Lambda Inference

Lambdas cannot have annotated parameters. Basilisk infers lambda parameter types exclusively from **bidirectional context** (the expected type pushed from the outer expression).

```python
# Expected type provides context
transform: Callable[[int], str] = lambda x: str(x)
#                                        ^ x is inferred as int from expected type

# No context — BSK-W0040: lambda parameter types unknown
f = lambda x: x + 1   # warning: x is unknown
```

Without an expected type, unannotated lambda parameters are `Unknown`. Unlike Pyright, Basilisk emits a **warning** (not silence) when a lambda's parameters cannot be inferred.

### 4.6 Overloads

Overloaded functions require full annotation on every `@overload` variant. The implementation signature (without `@overload`) must be compatible with all variants.

```python
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: int | str) -> int | str:   # implementation
    ...
```

A single `@overload` without an implementation is only valid in stub files (`.pyi`) and Protocol/ABC bodies. In regular modules, it fires `BSK-E0020`.

---

## 5. Collection Type Inference

### 5.1 Lists

Without bidirectional context:

```python
[]              # list[Never]  — empty, element type is bottom
[1, 2, 3]       # list[int]
[1, "hi"]       # list[int | str]  — union, not object
[1, 2.0]        # list[int | float]
```

With bidirectional context:

```python
x: list[float] = [1, 2, 3]   # list[float] — ints widen to float via expected type
```

> **Difference from Pyright**: Pyright's loose mode uses `list[Unknown]` for heterogeneous lists. Basilisk always uses union types. Pyright's `strictListInference` is always-on in Basilisk.

### 5.2 Dicts

```python
{}                  # dict[Never, Never]
{"a": 1, "b": 2}   # dict[str, int]
{"a": 1, "b": "x"} # dict[str, int | str]
{1: "a", "b": 2}   # dict[int | str, str | int]
```

### 5.3 Sets

```python
set()           # set[Never]
{1, 2, 3}       # set[int]
{1, "hi"}       # set[int | str]
```

### 5.4 Tuples

Tuples are **fixed-length by default**. Each element is typed independently:

```python
(1, "hi", 3.0)         # tuple[int, str, float]
(1,)                   # tuple[int]
()                     # tuple[()]
```

Homogeneous variable-length tuple: `tuple[int, ...]`

```python
def variadic(*args: int) -> None:
    reveal_type(args)  # tuple[int, ...]
```

> **Authority**: [Typing spec — Tuple types](https://typing.readthedocs.io/en/latest/spec/special-forms.html#tuple).

### 5.5 Comprehensions

```python
[x * 2 for x in range(10)]         # list[int]
{k: v for k, v in d.items()}       # dict[KT, VT]  where d: dict[KT, VT]
{x for x in "hello"}               # set[str]
(x for x in range(3))              # Generator[int, None, None]
```

---

## 6. Generic Type Inference

### 6.1 TypeVar Solving

When a generic function is called, Basilisk solves TypeVars using **bidirectional constraint propagation**:

1. Collect all constraints from argument types against TypeVar-bearing parameter types
2. Compute the **meet** (intersection) of upper-bound constraints and the **join** (union) of lower-bound constraints
3. If ambiguous, use the expected return type as an additional constraint (bidirectional)
4. If still ambiguous, emit an error rather than falling back to `Unknown`

```python
T = TypeVar("T")

def first(lst: list[T]) -> T: ...

x = first([1, 2, 3])         # T solved to int → x: int
y = first([1, "hi"])          # T solved to int | str → y: int | str
z: float = first([1, 2, 3])  # T solved to int; int assignable to float ✓
```

> **Authority**: [PEP 484 §Generics](https://peps.python.org/pep-0484/#generics).

### 6.2 Constrained TypeVars

```python
AnyStr = TypeVar("AnyStr", str, bytes)

def encode(x: AnyStr) -> AnyStr: ...

encode("hello")   # AnyStr = str → str
encode(b"bytes")  # AnyStr = bytes → bytes
encode(42)        # BSK-E0012: int does not match any constraint (str, bytes)
```

When a subtype is passed to a constrained TypeVar, the type is **widened to the matching constraint**, not kept at the subtype:

```python
class MyStr(str): pass

result = encode(MyStr("x"))   # AnyStr = str (not MyStr) — widened to constraint
reveal_type(result)            # str
```

> **Authority**: [Typing spec — Constrained TypeVars](https://typing.readthedocs.io/en/latest/spec/generics.html#constrained-type-variables).
> [Pyright docs](https://github.com/microsoft/pyright/blob/main/docs/type-inference.md): "When a subtype is passed to a constrained TypeVar, the inferred type is the matching constraint, not the subtype."

### 6.3 Bound TypeVars

```python
C = TypeVar("C", bound="Comparable")

def sort(items: list[C]) -> list[C]: ...
```

TypeVar bound constraints are **upper bounds**: any subtype of `Comparable` satisfies `C`. The solved type is the argument type itself (not widened to the bound).

### 6.4 Variance Inference (PEP 695)

With PEP 695 generic syntax, Basilisk **automatically infers variance**:

```python
class Stack[T]:
    def push(self, item: T) -> None: ...
    def pop(self) -> T: ...
```

- `T` appears in both input (`push`) and output (`pop`) positions → **invariant**

```python
class Readable[T]:
    def read(self) -> T: ...
```

- `T` appears only in output positions → **covariant** (`T` is inferred as `T_co`)

```python
class Consumer[T]:
    def consume(self, item: T) -> None: ...
```

- `T` appears only in input positions → **contravariant** (`T` is inferred as `T_contra`)

> **Authority**: [PEP 695 §Variance Inference](https://peps.python.org/pep-0695/#variance-inference).
> [Conformance suite `generics_variance_inference.py`](https://github.com/python/typing/blob/main/conformance/tests/generics_variance_inference.py).

### 6.5 TypeVar Defaults (PEP 696)

```python
from typing import TypeVar

T = TypeVar("T", default=int)

class Container[T = int]:
    def get(self) -> T: ...

c = Container()         # Container[int] — default applied
d = Container[str]()    # Container[str] — explicit wins
```

> **Authority**: [PEP 696](https://peps.python.org/pep-0696/).

### 6.6 ParamSpec

```python
from typing import ParamSpec, Callable

P = ParamSpec("P")

def logged(f: Callable[P, T]) -> Callable[P, T]:
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> T:
        print("calling")
        return f(*args, **kwargs)
    return wrapper
```

`P` captures the full parameter specification. The wrapped function has the same signature as the original.

> **Authority**: [PEP 612](https://peps.python.org/pep-0612/).

---

## 7. Type Narrowing

### 7.1 `isinstance` Narrowing

```python
def f(x: int | str) -> None:
    if isinstance(x, int):
        reveal_type(x)  # int
    else:
        reveal_type(x)  # str
```

Narrowing with `isinstance` against a union:
- In the `if` branch: `x` is narrowed to the intersection of its current type and the checked type
- In the `else` branch: `x` is narrowed to the **complement** — original type minus the checked type

For `isinstance(x, (A, B))` (tuple of types): the `if` branch narrows to `A | B`.

### 7.2 `is None` / `is not None`

```python
def f(x: int | None) -> None:
    if x is None:
        reveal_type(x)  # None
    else:
        reveal_type(x)  # int
```

### 7.3 Truthiness Narrowing

```python
def f(x: str | None) -> None:
    if x:
        reveal_type(x)  # str  — None and "" are falsy, so x must be non-empty str
```

Truthiness narrowing removes falsy types from the union (`None`, `Literal[0]`, `Literal[""]`, `Literal[False]`) in the truthy branch, and narrows to falsy types in the falsy branch.

### 7.4 Assignment Narrowing

```python
x: int | str = get_value()
x = 42
reveal_type(x)  # int — narrowed by assignment
```

After an assignment, the type of the variable is the type of the assigned value (possibly narrower than the declared type).

### 7.5 Pattern Matching Narrowing (PEP 634)

```python
def process(cmd: Command) -> None:
    match cmd:
        case Quit():
            reveal_type(cmd)  # Quit
        case Move(x=x, y=y):
            reveal_type(cmd)  # Move
            reveal_type(x)    # int (from Move.x annotation)
        case _:
            reveal_type(cmd)  # Command (remaining)
```

Basilisk performs **exhaustiveness checking** on match statements against union types. If all variants of a union are handled, the `case _` branch (if present) has type `Never`.

> **Authority**: [PEP 634](https://peps.python.org/pep-0634/), [PEP 635](https://peps.python.org/pep-0635/).

### 7.6 TypeGuard (PEP 647)

```python
from typing import TypeGuard

def is_str_list(val: list[object]) -> TypeGuard[list[str]]:
    return all(isinstance(x, str) for x in val)

def f(val: list[object]) -> None:
    if is_str_list(val):
        reveal_type(val)  # list[str]  — narrowed in positive branch only
    else:
        reveal_type(val)  # list[object]  — NOT narrowed in negative branch
```

`TypeGuard` narrows **only in the positive branch**. The negative branch retains the original type.

> **Authority**: [PEP 647](https://peps.python.org/pep-0647/).

### 7.7 TypeIs (PEP 742)

`TypeIs` is bidirectional: both branches are narrowed.

```python
from typing import TypeIs

def is_str(val: int | str) -> TypeIs[str]:
    return isinstance(val, str)

def f(val: int | str) -> None:
    if is_str(val):
        reveal_type(val)  # str
    else:
        reveal_type(val)  # int  — complement narrowing
```

> **Authority**: [PEP 742](https://peps.python.org/pep-0742/).

### 7.8 `assert` Narrowing

```python
x: int | None = get()
assert x is not None
reveal_type(x)  # int — narrowed after assert
```

Assertions narrow the type for all code after the `assert` statement (within the same flow path).

### 7.9 Dict Key Existence Narrowing

Basilisk supports narrowing `TypedDict` types via key existence checks — **beyond what Pyright currently implements**:

```python
class Movie(TypedDict, total=False):
    title: str
    year: int

def f(m: Movie) -> None:
    if "title" in m:
        reveal_type(m["title"])  # str — not str | undefined
```

### 7.10 Narrowing Scope Limitations

Narrowing does **not** persist across:

- Function boundaries (inner functions capture the unnarrowed type unless the narrowing condition is proven stable)
- Loop bodies (a narrowed type before a loop is reset to the pre-loop type at each iteration)
- After reassignment of the narrowed variable

---

## 8. Bidirectional Inference

Bidirectional inference propagates the **expected type** from the surrounding context into an expression. This resolves ambiguity that purely bottom-up inference cannot.

### 8.1 Assignment with Annotation

```python
x: list[int] = []          # expected: list[int] → [] infers as list[int], not list[Never]
y: dict[str, int] = {}     # expected: dict[str, int] → {} infers as dict[str, int]
```

### 8.2 Function Call Arguments

```python
def accept(items: list[str]) -> None: ...

accept([])          # expected: list[str] → [] infers as list[str]
accept(["a", "b"])  # list[str] ✓
```

### 8.3 Return Statements

```python
def f() -> list[int]:
    return []   # expected: list[int] → [] infers as list[int]
```

### 8.4 Lambda in Typed Context

```python
from typing import Callable

def apply(f: Callable[[int, int], bool], x: int, y: int) -> bool:
    return f(x, y)

apply(lambda a, b: a < b, 1, 2)
#     ^ a: int, b: int inferred from Callable[[int, int], bool]
```

> **Authority**: [Pyright docs on bidirectional inference](https://github.com/microsoft/pyright/blob/main/docs/type-inference.md#bidirectional-type-inference):
> "If the LHS of an assignment has a declared type, it can influence the inferred type of the RHS."

### 8.5 Conditional Expressions

```python
x: str | int = "hello" if flag else 42
#              ^ str                ^ int — both inferred; joined to str | int
```

### 8.6 Overload Selection with Bidirectional Context

When calling an overloaded function, the expected return type narrows overload candidate selection:

```python
@overload
def parse(s: str) -> int: ...
@overload
def parse(s: str) -> float: ...

result: float = parse("3.14")  # selects float overload via expected type
```

---

## 9. Special Types

### 9.1 `Any`

`Any` is bidirectionally compatible with all types. It represents an **explicit escape hatch**, not a default. In Basilisk, `Any` only appears when the programmer writes it. It is never inferred as a fallback.

> **Authority**: [PEP 484 §The `Any` type](https://peps.python.org/pep-0484/#the-any-type):
> "A special kind of type is `Any`. Every type is consistent with `Any`."

Unannotated parameters do **not** default to `Any` in Basilisk — they produce `BSK-E0001`. This is the critical divergence from mypy's `--ignore-missing-imports` behavior and Pyright's unannotated-parameter inference.

### 9.2 `Never` / `NoReturn`

`Never` is the **bottom type**: no value has type `Never`. Functions inferred to always raise have return type `Never`.

```python
def fail(msg: str) -> Never:
    raise AssertionError(msg)
```

`Never` is assignable to everything. A variable of type `Never` can never be reached. Basilisk uses this for **exhaustiveness checking**:

```python
def check(x: int | str) -> None:
    if isinstance(x, int):
        ...
    elif isinstance(x, str):
        ...
    else:
        reveal_type(x)  # Never — exhaustive
```

### 9.3 `Self`

`Self` represents the current class in a method's return type or parameter type. It is automatically inferred for `self` and `cls` but can be written explicitly for factory methods:

```python
from typing import Self

class Node:
    @classmethod
    def create(cls) -> Self:
        return cls()
```

> **Authority**: [PEP 673](https://peps.python.org/pep-0673/).

### 9.4 `LiteralString`

A supertype of all `Literal[str]` types. Used to enforce that only string literals (not dynamically constructed strings) are passed to security-sensitive APIs:

```python
from typing import LiteralString

def query(sql: LiteralString) -> None: ...

query("SELECT * FROM users")       # ✓ — literal
query("SELECT * FROM " + table)    # BSK-E0015 — not LiteralString
```

> **Authority**: [PEP 675](https://peps.python.org/pep-0675/).

---

## 10. Conformance Test Coverage

The [Python typing conformance suite](https://github.com/python/typing/tree/main/conformance) is the canonical benchmark. Basilisk targets **100% conformance** (Pass on all 150 test files).

Inference-relevant conformance tests:

| Test file | What it verifies |
|---|---|
| `generics_basic.py` | TypeVar solving from call arguments |
| `generics_scoping.py` | TypeVar scope binding |
| `generics_type_erasure.py` | Generic instantiation inference |
| `generics_variance_inference.py` | Auto-variance from usage positions (PEP 695) |
| `generics_self_*.py` | `Self` type in various positions |
| `generics_defaults.py` | TypeVar defaults (PEP 696) |
| `annotations_methods.py` | `self`/`cls` inference |
| `narrowing_typeguard.py` | TypeGuard narrowing (positive branch only) |
| `narrowing_typeis.py` | TypeIs bidirectional narrowing |
| `directives_assert_type.py` | Checker must verify inferred types match `assert_type()` |
| `directives_reveal_type.py` | Checker must emit inferred types via `reveal_type()` |
| `overloads_evaluation.py` | 5-step overload resolution algorithm |
| `specialtypes_any.py` | `Any` bidirectional assignability |
| `specialtypes_never.py` | `Never` / exhaustiveness |
| `literals_semantics.py` | Literal type subtyping |

---

## 11. Where Basilisk Exceeds Pyright

The following capabilities go beyond Pyright's current implementation:

### 11.1 No `Unknown` Fallback

Pyright uses `Unknown` (a special `Any`) when it cannot determine a type. Basilisk **never produces `Unknown`** — every inferred type is either a concrete type or an error.

### 11.2 Strict Container Inference Always On

Pyright's `strictListInference` (union of element types) is off by default. Basilisk applies union inference to all containers in all modes — no configuration switch.

### 11.3 Dict Key Narrowing for TypedDict

TypedDict narrowing via `"key" in d` is beyond Pyright's current narrowing capabilities. Basilisk implements this directly.

### 11.4 Exhaustive Pattern Matching Analysis

Basilisk checks that `match` statements on union types are exhaustive. Pyright performs limited exhaustiveness analysis; Basilisk tracks exact variant coverage.

### 11.5 Lambda Warnings

When a lambda cannot have its parameter types inferred from context, Basilisk emits `BSK-W0040`. Pyright silently uses `Unknown`. This surfaces missing type annotations in higher-order functions early.

### 11.6 Annotation Required, Not Optional

Pyright infers parameter types from defaults and call-site analysis. Basilisk treats every missing annotation as an error. "Silent inference" of public API types is not permitted.

---

## 12. Implementation Notes (Rust)

The type inference engine is implemented in the `basilisk-checker` crate using [Salsa](https://github.com/salsa-rs/salsa) for incremental computation.

Key components:

- **`InferenceEngine`** — top-level bidirectional inference driver
- **`ConstraintSolver`** — TypeVar constraint collection and solving
- **`NarrowingEngine`** — flow-sensitive type narrowing via control-flow graph
- **`OverloadResolver`** — 5-step overload resolution per conformance spec
- **`LiteralFolder`** — Literal type widening/narrowing rules

All inference results are stored as Salsa query results, enabling **sub-10ms incremental re-inference** when a single file changes.

---

## References

1. [PEP 484 — Type Hints](https://peps.python.org/pep-0484/)
2. [PEP 526 — Variable Annotations](https://peps.python.org/pep-0526/)
3. [PEP 695 — Type Parameter Syntax](https://peps.python.org/pep-0695/)
4. [Python Typing Specification](https://typing.readthedocs.io/en/latest/)
5. [Python Typing Conformance Suite](https://github.com/python/typing/tree/main/conformance)
6. [Pyright Type Inference Documentation](https://github.com/microsoft/pyright/blob/main/docs/type-inference.md)
7. [Pyright Type Concepts (Advanced)](https://github.com/microsoft/pyright/blob/main/docs/type-concepts-advanced.md)
8. [Pierce & Turner — Local Type Inference (2000)](https://www.cl.cam.ac.uk/~nk480/bidir.pdf)
9. [mypy — Type Inference and Annotations](https://mypy.readthedocs.io/en/stable/type_inference_and_annotations.html)
10. [BasedPyright — Type Inference](https://docs.basedpyright.com/v1.38.0/usage/type-inference/)
