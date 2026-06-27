# Basilisk Type Inference Specification {#TYPEINF}

Basilisk implements premium type inference that not only improves type safety - it enforces the removal of redundant type annotations. The aim is to achieve something in the ballpark of Hindley Milner style functionality where we do not specify types unless there is a special reason to. We want to avoid forcing Python developers to specify types unless it's absolutely necessary. This means that Python continues to be a less verbose language with full type safety.

> **Canonical Python version**: 3.12
>
> **Authoritative references**: [PEP 484](https://peps.python.org/pep-0484/), [PEP 526](https://peps.python.org/pep-0526/), [Python Typing Spec](https://typing.readthedocs.io/en/latest/), [Python Typing Conformance Suite](https://github.com/python/typing/tree/main/conformance)
>
> **Implementation**: Core inference engine (`inference.rs`, `collection_inference.rs`, `types.rs`, `types_parsing.rs`) is wired into rules E0011, E0013, E0014, E0120, and W0050.

---

## Design Philosophy {#TYPEINF-PHILOSOPHY}

Basilisk's type inference is **precise and bidirectional throughout**. Rather than making inference optional, or falling back to an unresolved/`Any` type when a type cannot be determined, Basilisk either produces a precise type or emits a diagnostic.

Key design decisions in Basilisk's inference engine:

| Capability | Basilisk behavior |
|---|---|
| Unannotated parameter types | **Error** — all parameters must be annotated |
| Return type inference | Inferred **and** validated; a mismatch is an error |
| Container inference | **Union** of element types, always — no loose mode |
| TypeVar constraint solving | **Bidirectional constraint propagation** with exhaustive solving |
| Literal type inference | Literal-first: widen only when an annotation demands it |
| Narrowing coverage | `isinstance`, `is None`, TypeGuard, TypeIs, **pattern-matching exhaustiveness**, **dict key existence**, **attribute presence** |
| Unannotated functions | **Error** — every public function must be annotated |
| Redundant annotations | **Warning** — redundant explicit annotations must be removed |

### Redundant Annotation Principle {#TYPEINF-REDUNDANT}

> **This is a critical, non-negotiable design goal.**

But, ignore this section when it conflicts with PEP conformance

Basilisk enforces a **clean separation between what must be annotated and what must not be**. When the type system can infer a type precisely, writing an explicit annotation for that same type is **noise** — it clutters the code, creates maintenance burden, and masks real inference failures.

**Rule**: If Basilisk can infer the type of an expression unambiguously, and the written annotation is identical to the inferred type, Basilisk emits `BSK-W0050: redundant type annotation — inferred type is identical; remove the annotation`.

```python
# BAD — Basilisk emits BSK-W0050 for every one of these
x: int = 42                     # inferred: int — annotation redundant
y: str = "hello"                # inferred: str — annotation redundant
z: list[int] = [1, 2, 3]        # inferred: list[int] — annotation redundant
items: list[str] = ["a", "b"]   # inferred: list[str] — annotation redundant

def f(n: int) -> int:
    result: int = n * 2         # inferred: int — annotation redundant
    return result

# GOOD — annotation adds information not available from inference alone
x: float = 42                   # widens int to float — meaningful
items: list[int | str] = [1]    # widens list[int] to list[int | str] — meaningful
coords: tuple[float, float] = (0, 0)  # widens tuple[int, int] — meaningful
```

**Annotation is required when it changes the type** (widens, constrains, or documents a contract). Annotation is forbidden when it merely repeats what inference would produce.

This rule applies to:
- Local variable assignments
- Module-level variable assignments
- Class body variable assignments
- For-loop target variables
- With-statement target variables
- Walrus operator targets

**Exceptions** — annotations are always permitted (and required) even when inference could theoretically determine the type:
- Function parameters (§4.1) — annotation is always required, never redundant
- Public function return types (§4.3) — annotation is always required, never redundant
- `TypedDict` fields — annotation is always required
- `NamedTuple` fields — annotation is always required
- `Protocol` member signatures — annotation is always required
- `@dataclass` / `@pydantic.dataclasses.dataclass` / attrs (`@define`, `@attr.s`) /
  pydantic `BaseModel` fields — the annotation is what makes the assignment a
  field; never redundant (issues #110, #39)
- `ClassVar` and `Final` — the qualifier itself is non-redundant even if the base type is inferrable

**Interaction with conformance**: This rule does not conflict with PEP 526 or any conformance test. The conformance suite tests that annotations are respected when present; it does not require annotations where inference suffices. Basilisk goes further by enforcing that redundant annotations are absent.

---

## Governing PEPs {#TYPEINF-PEPS}

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

## Type Inference Overview {#TYPEINF-OVERVIEW}

### What Is Inferred {#TYPEINF-INFERRED}

Basilisk infers types for:

- **Local variable assignments** — `x = 42` → `x: int`
- **Return types** — from the union of all `return` expression types (see §5)
- **Container literals** — list, dict, set, tuple elements (see §6)
- **`self` and `cls`** — always inferred, never annotated (see §4.4)
- **Walrus operator** — `(x := expr)` has the same type as `expr`
- **Comprehensions** — element type from the expression, collection type from the form
- **Generic instantiation** — `list[int]()` → `list[int]`; `Foo(x)` → `Foo[T]` solved from `x` (see §8)
- **Narrowed types** — after guards (see §9)

### What Is Never Inferred {#TYPEINF-REQUIRED}

Basilisk **requires explicit annotations** for:

- **All function parameters** (except `self`, `cls`) — E0001 if missing
- **All public function return types** — E0002 if missing
- **All class-level attributes** at the class body level — E0003 if missing
- **TypedDict fields**, `NamedTuple` fields, `Protocol` members — always explicit

Basilisk does not silently fall back to an unresolved type. A missing annotation is a **diagnostic**, not an inference opportunity.

### Inference Algorithm {#TYPEINF-ALGO}

Basilisk uses a **bidirectional type inference** algorithm:

```
infer_type(expr, expected: Option<Type>) -> Type
```

The `expected` parameter ("expected type" or "pushed type") flows from outer context into inner expressions. When present, it guides inference of ambiguous constructs (empty containers, lambdas, integer literals that could be `Literal[N]`).

This is the same approach described in the [bidirectional typing literature](https://www.cl.cam.ac.uk/~nk480/bidir.pdf) (Pierce & Turner, "Local Type Inference", 2000), in which an expected type flows from the surrounding context into the inference of subexpressions.

---

## Variable Type Inference {#TYPEINF-VARS}

### Simple Assignment {#TYPEINF-VARS-SIMPLE}

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

Basilisk reserves `Literal` inference for module/class constants; within function bodies, literals are widened to their base types unless a literal-sensitive context demands otherwise.

### Multiple Assignment {#TYPEINF-VARS-FLOW}

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

### Annotated Variable {#TYPEINF-VARS-ANNOTATED}

When an annotation is present, the annotation **is** the declared type. The inferred RHS type must be assignable to it:

```python
x: int = 42         # declared: int; RHS infers int ✓
y: float = 42       # declared: float; RHS infers int; int is subtype of float ✓
z: str = 42         # imports_unresolved: int is not assignable to str
```

> **Authority**: [PEP 526 §Annotated assignment statements](https://peps.python.org/pep-0526/#annotated-assignment-statements):
> "If a variable has been annotated, all assignments to that variable will be type-checked."

### Augmented Assignment {#TYPEINF-VARS-AUGMENTED}

```python
x = 1
x += 2   # still int — calls __iadd__ or __add__, return type drives x's new type
```

The type of `x` after `x op= rhs` is the return type of `type(x).__iadd__(rhs)` (or `__add__` if `__iadd__` is absent). If the return type differs from `x`'s current type, the narrowed type applies.

### Walrus Operator {#TYPEINF-VARS-WALRUS}

```python
if (n := len(a)) > 10:
    reveal_type(n)  # int
```

The walrus operator `:=` assigns the value and the **expression type equals the assigned type**. The variable `n` is in scope for the remainder of the enclosing scope (not just the `if` block).

> **Authority**: [PEP 572](https://peps.python.org/pep-0572/): "The value of the target is the same as the value of the value expression."

---

## Function Type Inference {#TYPEINF-FUNC}

### Parameters {#TYPEINF-FUNC-PARAMS}

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

### Default Parameters {#TYPEINF-FUNC-DEFAULTS}

When a parameter has no annotation but has a default, Basilisk **does not** infer the type from the default. The annotation is still required.

```python
def connect(timeout=30):        # BSK-E0001 — annotation required even with default
    pass

def connect(timeout: int = 30): # ✓
    pass
```

### Return Types {#TYPEINF-FUNC-RETURN}

Return types are **inferred from the function body** but an annotation is required for all non-trivial public functions (those that are not `-> None` trivially).

The inferred return type is the **union of all `return` expression types**:

```python
def f(x: int) -> int | str:    # ✓ — annotation matches inference
    if x > 0:
        return x               # int
    return "negative"          # str
```

If the annotated return type is **narrower** than the inferred union, Basilisk emits `returns_compatibility` (return type mismatch).

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

### `self` and `cls` Inference {#TYPEINF-FUNC-SELFCLS}

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

### Lambda Inference {#TYPEINF-FUNC-LAMBDA}

Lambdas cannot have annotated parameters. Basilisk infers lambda parameter types exclusively from **bidirectional context** (the expected type pushed from the outer expression).

```python
# Expected type provides context
transform: Callable[[int], str] = lambda x: str(x)
#                                        ^ x is inferred as int from expected type

# No context — BSK-W0040: lambda parameter types unknown
f = lambda x: x + 1   # warning: x is unknown
```

Without an expected type, a lambda's parameter types cannot be inferred. Rather than leaving them silently untyped, Basilisk emits a **warning** (`BSK-W0040`).

### Overloads {#TYPEINF-FUNC-OVERLOADS}

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

A single `@overload` without an implementation is only valid in stub files (`.pyi`) and Protocol/ABC bodies. In regular modules, it fires `overloads_definitions`.

---

## Collection Type Inference {#TYPEINF-COLLECTIONS}

### Lists {#TYPEINF-COLLECTIONS-LISTS}

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

> **Container inference**: Basilisk always infers a **union** of element types for heterogeneous containers — there is no loose mode and no configuration switch to disable it.

### Dicts {#TYPEINF-COLLECTIONS-DICTS}

```python
{}                  # dict[Never, Never]
{"a": 1, "b": 2}   # dict[str, int]
{"a": 1, "b": "x"} # dict[str, int | str]
{1: "a", "b": 2}   # dict[int | str, str | int]
```

### Sets {#TYPEINF-COLLECTIONS-SETS}

```python
set()           # set[Never]
{1, 2, 3}       # set[int]
{1, "hi"}       # set[int | str]
```

### Tuples {#TYPEINF-COLLECTIONS-TUPLES}

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

### Comprehensions {#TYPEINF-COLLECTIONS-COMPREHENSIONS}

```python
[x * 2 for x in range(10)]         # list[int]
{k: v for k, v in d.items()}       # dict[KT, VT]  where d: dict[KT, VT]
{x for x in "hello"}               # set[str]
(x for x in range(3))              # Generator[int, None, None]
```

---

## Generic Type Inference {#TYPEINF-GENERICS}

### TypeVar Solving {#TYPEINF-GENERICS-TYPEVAR}

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

### Constrained TypeVars {#TYPEINF-GENERICS-CONSTRAINED}

```python
AnyStr = TypeVar("AnyStr", str, bytes)

def encode(x: AnyStr) -> AnyStr: ...

encode("hello")   # AnyStr = str → str
encode(b"bytes")  # AnyStr = bytes → bytes
encode(42)        # calls_argument_type: int does not match any constraint (str, bytes)
```

When a subtype is passed to a constrained TypeVar, the type is **widened to the matching constraint**, not kept at the subtype:

```python
class MyStr(str): pass

result = encode(MyStr("x"))   # AnyStr = str (not MyStr) — widened to constraint
reveal_type(result)            # str
```

> **Authority**: [Typing spec — Constrained TypeVars](https://typing.readthedocs.io/en/latest/spec/generics.html#constrained-type-variables).
> [Pyright docs](https://github.com/microsoft/pyright/blob/main/docs/type-inference.md): "When a subtype is passed to a constrained TypeVar, the inferred type is the matching constraint, not the subtype."

### Bound TypeVars {#TYPEINF-GENERICS-BOUND}

```python
C = TypeVar("C", bound="Comparable")

def sort(items: list[C]) -> list[C]: ...
```

TypeVar bound constraints are **upper bounds**: any subtype of `Comparable` satisfies `C`. The solved type is the argument type itself (not widened to the bound).

### Variance Inference {#TYPEINF-GENERICS-VARIANCE}

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

### TypeVar Defaults {#TYPEINF-GENERICS-DEFAULTS}

```python
from typing import TypeVar

T = TypeVar("T", default=int)

class Container[T = int]:
    def get(self) -> T: ...

c = Container()         # Container[int] — default applied
d = Container[str]()    # Container[str] — explicit wins
```

> **Authority**: [PEP 696](https://peps.python.org/pep-0696/).

### ParamSpec {#TYPEINF-GENERICS-PARAMSPEC}

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

## Type Narrowing {#TYPEINF-NARROWING}

### `isinstance` Narrowing {#TYPEINF-NARROWING-ISINSTANCE}

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

### `is None` / `is not None` {#TYPEINF-NARROWING-NONE}

```python
def f(x: int | None) -> None:
    if x is None:
        reveal_type(x)  # None
    else:
        reveal_type(x)  # int
```

### Truthiness Narrowing {#TYPEINF-NARROWING-TRUTHY}

```python
def f(x: str | None) -> None:
    if x:
        reveal_type(x)  # str  — None and "" are falsy, so x must be non-empty str
```

Truthiness narrowing removes falsy types from the union (`None`, `Literal[0]`, `Literal[""]`, `Literal[False]`) in the truthy branch, and narrows to falsy types in the falsy branch.

### Assignment Narrowing {#TYPEINF-NARROWING-ASSIGN}

```python
x: int | str = get_value()
x = 42
reveal_type(x)  # int — narrowed by assignment
```

After an assignment, the type of the variable is the type of the assigned value (possibly narrower than the declared type).

### Pattern Matching Narrowing {#TYPEINF-NARROWING-MATCH}

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

### TypeGuard {#TYPEINF-NARROWING-TYPEGUARD}

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

### TypeIs {#TYPEINF-NARROWING-TYPEIS}

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

### `assert` Narrowing {#TYPEINF-NARROWING-ASSERT}

```python
x: int | None = get()
assert x is not None
reveal_type(x)  # int — narrowed after assert
```

Assertions narrow the type for all code after the `assert` statement (within the same flow path).

### Dict Key Existence Narrowing {#TYPEINF-NARROWING-DICTKEY}

Basilisk supports narrowing `TypedDict` types via key existence checks:

```python
class Movie(TypedDict, total=False):
    title: str
    year: int

def f(m: Movie) -> None:
    if "title" in m:
        reveal_type(m["title"])  # str — not str | undefined
```

### Narrowing Scope Limitations {#TYPEINF-NARROWING-SCOPE}

Narrowing does **not** persist across:

- Function boundaries (inner functions capture the unnarrowed type unless the narrowing condition is proven stable)
- Loop bodies (a narrowed type before a loop is reset to the pre-loop type at each iteration)
- After reassignment of the narrowed variable

---

## Bidirectional Inference {#TYPEINF-BIDIR}

Bidirectional inference propagates the **expected type** from the surrounding context into an expression. This resolves ambiguity that purely bottom-up inference cannot.

### Assignment with Annotation {#TYPEINF-BIDIR-ASSIGN}

```python
x: list[int] = []          # expected: list[int] → [] infers as list[int], not list[Never]
y: dict[str, int] = {}     # expected: dict[str, int] → {} infers as dict[str, int]
```

### Function Call Arguments {#TYPEINF-BIDIR-CALLARGS}

```python
def accept(items: list[str]) -> None: ...

accept([])          # expected: list[str] → [] infers as list[str]
accept(["a", "b"])  # list[str] ✓
```

### Return Statements {#TYPEINF-BIDIR-RETURN}

```python
def f() -> list[int]:
    return []   # expected: list[int] → [] infers as list[int]
```

### Lambda in Typed Context {#TYPEINF-BIDIR-LAMBDA}

```python
from typing import Callable

def apply(f: Callable[[int, int], bool], x: int, y: int) -> bool:
    return f(x, y)

apply(lambda a, b: a < b, 1, 2)
#     ^ a: int, b: int inferred from Callable[[int, int], bool]
```

> **Authority**: [Pyright docs on bidirectional inference](https://github.com/microsoft/pyright/blob/main/docs/type-inference.md#bidirectional-type-inference):
> "If the LHS of an assignment has a declared type, it can influence the inferred type of the RHS."

### Conditional Expressions {#TYPEINF-BIDIR-CONDITIONAL}

```python
x: str | int = "hello" if flag else 42
#              ^ str                ^ int — both inferred; joined to str | int
```

### Overload Selection with Bidirectional Context {#TYPEINF-BIDIR-OVERLOAD}

When calling an overloaded function, the expected return type narrows overload candidate selection:

```python
@overload
def parse(s: str) -> int: ...
@overload
def parse(s: str) -> float: ...

result: float = parse("3.14")  # selects float overload via expected type
```

---

## Subtyping {#TYPEINF-SUBTYPING}

Basilisk implements both **nominal** and **structural** subtyping. This is the core of type compatibility — `is_assignable_to(source, target)` must answer "can a value of type `source` be used where type `target` is expected?"

> **Authority**: [PEP 484 §Subtype relationships](https://peps.python.org/pep-0484/), [PEP 544 §Protocols: Structural subtyping](https://peps.python.org/pep-0544/), [Python Typing Spec — Type system concepts](https://typing.readthedocs.io/en/latest/spec/concepts.html)

### Nominal Subtyping {#TYPEINF-SUBTYPING-NOMINAL}

A type `A` is a nominal subtype of `B` if `B` appears in `A.__mro__` (Method Resolution Order). This is Python's standard class inheritance model.

```python
class Animal: ...
class Dog(Animal): ...

x: Animal = Dog()  # OK — Dog is a nominal subtype of Animal
```

**MRO resolution** uses [C3 linearization](https://www.python.org/download/releases/2.3/mro/) (same as CPython). The MRO is computed per class and cached in `ResolvedModule`.

**Builtin type hierarchy** (hardcoded):
- `bool` <: `int` <: `float` <: `complex`
- `bytearray` <: `bytes` (for read contexts)
- All classes <: `object`
- `Never` <: everything (bottom type)

### Protocol Structural Subtyping {#TYPEINF-SUBTYPING-PROTOCOL}

A type `A` structurally satisfies a `Protocol` `P` if `A` provides **all members** declared in `P` with compatible types. No explicit inheritance is required.

```python
class Drawable(Protocol):
    def draw(self, x: int, y: int) -> None: ...

class Circle:
    def draw(self, x: int, y: int) -> None: ...

c: Drawable = Circle()  # OK — Circle structurally satisfies Drawable
```

**Protocol conformance algorithm**:

1. **Collect protocol members**: methods, properties, class variables, instance attributes from the Protocol class and all its Protocol bases (walk the MRO, stop at `Protocol` itself).

2. **For each protocol member**, check that the candidate class provides a matching member:
   - **Method**: candidate must have a method with the same name whose signature is **compatible** (see §9.6 Callable subtyping).
   - **Property (read-only)**: candidate must have a readable attribute or property with a return type that is a **subtype** of the protocol property's return type (covariant).
   - **Property (read-write)**: candidate must have a writable attribute. The type must be **invariant** (both subtype and supertype of the protocol's declared type).
   - **Class variable**: candidate must have a class-level attribute with compatible type.
   - **Instance attribute**: candidate must have an instance attribute (including dataclass fields, `NamedTuple` fields, or `__init__`-assigned attributes) with compatible type.

3. **Attribute vs. property equivalence**: A plain class attribute `val: T` satisfies a protocol's `@property` requirement for `val -> T` (read-only). A mutable attribute satisfies a read-write `@property` requirement.

4. **Inherited members**: Walk the candidate class's full MRO to find members. A method inherited from a base class satisfies the protocol requirement.

5. **`Self` type**: Protocol methods using `Self` in return types are satisfied when the candidate returns `Self` or the candidate's own type.

> **Authority**: [PEP 544 §Protocol members](https://peps.python.org/pep-0544/#protocol-members), [Typing spec — Protocols](https://typing.readthedocs.io/en/latest/spec/protocol.html)

### TypedDict Structural Subtyping {#TYPEINF-SUBTYPING-TYPEDDICT}

TypedDict-to-TypedDict assignability is structural, not nominal:

```python
class MovieBase(TypedDict):
    name: str

class Movie(TypedDict):
    name: str
    year: int

m: MovieBase = Movie(name="Alien", year=1979)  # OK — Movie has all MovieBase fields
```

**Rules**:
- All required fields of the target must exist in the source with compatible types.
- `ReadOnly` fields are covariant (source field type must be subtype of target).
- Mutable fields are **invariant** (exact type match required).
- `NotRequired` fields in the target may be absent in the source.
- `extra_items` (PEP 728): if the target allows extra items of type `T`, the source's extra fields must have types assignable to `T`.

> **Authority**: [PEP 589 §TypedDict](https://peps.python.org/pep-0589/), [PEP 705 §ReadOnly](https://peps.python.org/pep-0705/), [PEP 728 §extra_items](https://peps.python.org/pep-0728/)

### Generic Subtyping {#TYPEINF-SUBTYPING-GENERIC}

Generic types combine nominal subtyping with variance:

```python
class Animal: ...
class Dog(Animal): ...

x: list[Animal] = [Dog()]  # ERROR — list is invariant
y: Sequence[Animal] = [Dog()]  # OK — Sequence is covariant
```

**Variance rules** for generic type parameters:
- **Covariant** (`T_co`): `G[A]` <: `G[B]` if `A` <: `B`. Read-only containers (`Sequence`, `Iterator`, `FrozenSet`, `tuple`).
- **Contravariant** (`T_contra`): `G[A]` <: `G[B]` if `B` <: `A`. Write-only positions (function parameters in `Callable`).
- **Invariant** (default): `G[A]` <: `G[B]` only if `A` == `B`. Mutable containers (`list`, `dict`, `set`).

**Generic subtyping algorithm**:
1. Check nominal subtyping: does source class's MRO include the target's base class?
2. Find the TypeVar substitution: how does the source specialize the target's TypeVars?
3. Apply variance rules to each TypeVar position.

### Union and Special-Form Subtyping {#TYPEINF-SUBTYPING-UNION}

- `A` <: `A | B` (always — a type is a subtype of any union containing it)
- `A | B` <: `C` only if `A` <: `C` AND `B` <: `C`
- `Optional[T]` = `T | None`
- `Any` is bidirectionally compatible with all types (not a real subtype, an escape hatch)
- `Never` <: everything (bottom type, assignable to all types)
- `object` >: everything except `None` under strict subtyping

### Callable Subtyping {#TYPEINF-SUBTYPING-CALLABLE}

Callable subtyping follows **parameter contravariance** and **return covariance**:

```python
# Callable[[ParamTypes], ReturnType]
# Parameters are contravariant, return type is covariant

f: Callable[[Animal], Dog]  # accepts Animal, returns Dog
g: Callable[[Dog], Animal]  # accepts Dog, returns Animal

# f is NOT assignable to g: Dog (param of g) is not supertype of Animal (param of f)
# g is NOT assignable to f: Animal (return of g) is not subtype of Dog (return of f)
```

**Callable compatibility rules**:
- Source return type must be a **subtype** of target return type (covariant).
- Target parameter types must be **subtypes** of source parameter types (contravariant).
- Source may have fewer required parameters than target (extra defaults OK).
- `*args`/`**kwargs` in source accepts any parameter count in target.
- `Callable[..., R]` (ellipsis params) is compatible with any parameter signature.

> **Authority**: [PEP 484 §Callable](https://peps.python.org/pep-0484/#callable), [Typing spec — Callables](https://typing.readthedocs.io/en/latest/spec/callables.html)

### Implementation: `is_subtype_of()` {#TYPEINF-SUBTYPING-IMPL}

The current `is_assignable_to()` in `types.rs` handles primitives, containers, unions, optionals, and callables but falls back to name comparison for `Named` types. The full subtyping engine replaces this with:

```rust
fn is_subtype_of(source: &ResolvedType, target: &ResolvedType, ctx: &SubtypeContext) -> bool {
    match (source, target) {
        // Nominal: check MRO
        (Class(src), Class(tgt)) => ctx.mro_contains(src, tgt),
        // Protocol: structural check
        (_, Protocol(proto)) => ctx.satisfies_protocol(source, proto),
        // Generic: variance-aware
        (Generic(src_base, src_args), Generic(tgt_base, tgt_args)) =>
            ctx.check_generic_subtype(src_base, src_args, tgt_base, tgt_args),
        // TypedDict: field-by-field
        (TypedDict(src), TypedDict(tgt)) => ctx.check_typeddict_compat(src, tgt),
        // Callable: contravariant params, covariant return
        (Callable(src), Callable(tgt)) => ctx.check_callable_subtype(src, tgt),
        // ... other cases
    }
}
```

`SubtypeContext` holds the MRO cache, protocol member tables, and generic variance info needed for recursive subtype checks.

---

## Special Types {#TYPEINF-SPECIAL}

### `Any` {#TYPEINF-SPECIAL-ANY}

`Any` is bidirectionally compatible with all types. It represents an **explicit escape hatch**, not a default. In Basilisk, `Any` only appears when the programmer writes it. It is never inferred as a fallback.

> **Authority**: [PEP 484 §The `Any` type](https://peps.python.org/pep-0484/#the-any-type):
> "A special kind of type is `Any`. Every type is consistent with `Any`."

Unannotated parameters do **not** default to `Any` in Basilisk — they produce `BSK-E0001`. Basilisk never silently infers a public-API parameter type; a missing annotation is always a diagnostic.

### `Never` / `NoReturn` {#TYPEINF-SPECIAL-NEVER}

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

### `Self` {#TYPEINF-SPECIAL-SELF}

`Self` represents the current class in a method's return type or parameter type. It is automatically inferred for `self` and `cls` but can be written explicitly for factory methods:

```python
from typing import Self

class Node:
    @classmethod
    def create(cls) -> Self:
        return cls()
```

> **Authority**: [PEP 673](https://peps.python.org/pep-0673/).

### `LiteralString` {#TYPEINF-SPECIAL-LITERALSTRING}

A supertype of all `Literal[str]` types. Used to enforce that only string literals (not dynamically constructed strings) are passed to security-sensitive APIs:

```python
from typing import LiteralString

def query(sql: LiteralString) -> None: ...

query("SELECT * FROM users")       # ✓ — literal
query("SELECT * FROM " + table)    # callables_annotation — not LiteralString
```

> **Authority**: [PEP 675](https://peps.python.org/pep-0675/).

---

## Conformance Test Coverage {#TYPEINF-CONFORMANCE}

The [Python typing conformance suite](https://github.com/python/typing/tree/main/conformance) is the canonical benchmark. Basilisk **targets** 100% conformance (Pass on all 146 test files) — a target, not a present-day achievement. The official, unmodified `python/typing` scorer currently reports **68 of 146 files passing (46.6%, counting errors and warnings — the strictest grading)**, with the binary run with **every rule enabled** — no config, no `basilisk.json`, no "spec-conformance mode", no exceptions. The remaining gap is **265 false positives and 0 missed required errors**: the checker catches every required error, and every failing fixture fails only because strict-by-default house-style rules (require-annotation E0001/E0002/E0004, missing-`@override` E0025, explicit-`Any` W0014, redundant-annotation W0050) fire on spec-valid code where the spec treats unannotated as inferred rather than an error. The only legitimate path to 100% is fixing the checker so these strict defaults stop firing on spec-valid code, with every rule still enabled — never by disabling a rule.

> **History (stated plainly):** the last honest score was 59 of 146 = 40.4% (285 false positives) at PR #183. PRs #184/#185/#191 inflated the reported number to a fake 100% by writing a `basilisk.json` that **disabled** those six house rules at score time (the so-called "spec-conformance mode"). The checker was not made smarter; the false positives were merely hidden. That disabling has been **removed**, and disabling any conformance rule for scoring is now forbidden. Genuine progress over that span was real but modest: 40.4% → 46.6%. There is no "spec-conformance mode" any more.

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

## Distinctive Inference Behaviors {#TYPEINF-EXCEEDS}

The following are deliberate, distinctive behaviors of Basilisk's inference engine:

### No Unresolved-Type Fallback {#TYPEINF-EXCEEDS-NOUNKNOWN}

Basilisk **never produces an unresolved/`Unknown` type** when it cannot determine a type — every inferred type is either a concrete type or an error.

### Strict Container Inference Always On {#TYPEINF-EXCEEDS-CONTAINERS}

Basilisk applies union-of-element-types inference to all containers unconditionally — there is no loose mode and no configuration switch to disable it.

### Dict Key Narrowing for TypedDict {#TYPEINF-EXCEEDS-DICTKEY}

Basilisk narrows `TypedDict` types via `"key" in d` key-existence checks directly.

### Exhaustive Pattern Matching Analysis {#TYPEINF-EXCEEDS-EXHAUSTIVE}

Basilisk checks that `match` statements on union types are exhaustive, tracking exact variant coverage.

### Lambda Warnings {#TYPEINF-EXCEEDS-LAMBDA}

When a lambda's parameter types cannot be inferred from context, Basilisk emits `BSK-W0040` rather than leaving them silently untyped — surfacing missing annotations in higher-order functions early.

### Annotation Required, Not Optional {#TYPEINF-EXCEEDS-REQUIRED}

Basilisk treats every missing annotation as an error. "Silent inference" of public-API types is not permitted.

---

## Implementation Notes {#TYPEINF-IMPL}

The type inference engine is implemented in the `basilisk-checker` crate using [Salsa](https://github.com/salsa-rs/salsa) for incremental computation.

Key components:

- **`InferenceEngine`** — top-level bidirectional inference driver
- **`ConstraintSolver`** — TypeVar constraint collection and solving
- **`NarrowingEngine`** — flow-sensitive type narrowing via control-flow graph
- **`OverloadResolver`** — 5-step overload resolution per conformance spec
- **`LiteralFolder`** — Literal type widening/narrowing rules

All inference results are stored as Salsa query results, enabling **sub-10ms incremental re-inference** when a single file changes.

---

## References {#TYPEINF-REFS}

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
