# Basilisk type inference {#TYPEINF}

Basilisk combines conservative shared inference with focused typing-rule algorithms. The default configuration follows the typing specification; optional house rules can require or discourage annotations without changing PEP behavior (see [TYPEINF-REDUNDANT]).

> **Canonical Python version**: 3.12
>
> **Authoritative references**: [PEP 484](https://peps.python.org/pep-0484/), [PEP 526](https://peps.python.org/pep-0526/), [Python Typing Spec](https://typing.python.org/en/latest/spec/), [Python Typing Conformance Suite](https://github.com/python/typing/tree/main/conformance)
>
> **Implementation**: Core inference engine (`inference.rs`, `collection_inference.rs`, `types.rs`, `types_parsing.rs`) is wired into rules E0011, E0013, E0014, E0120, and W0050.

---

## Redundant Annotation Principle {#TYPEINF-REDUNDANT}

`BSK-0050` is an opt-in house rule. When the narrow syntactic inference engine can prove
that an assignment annotation exactly repeats its RHS type, it may suggest removing the
annotation. It never overrides typing-spec syntax or a semantic annotation purpose.

```python
# BAD — BSK-0050: annotation equals inferred type
x: int = 42                     # inferred: int
z: list[int] = [1, 2, 3]        # inferred: list[int]

# GOOD — annotation adds information
x: float = 42                   # widens int → float
items: list[int | str] = [1]    # widens list[int] → list[int | str]
```

Applies to:
- Local variable assignments
- Module-level variable assignments
- Class body variable assignments
- For-loop target variables
- With-statement target variables
- Walrus operator targets

**Exceptions** — annotations are never considered redundant in these positions;
separate opt-in rules may require them:
- Function parameters (§4.1)
- Public function return types (§4.3)
- `TypedDict` fields — annotation is always required
- `NamedTuple` fields — annotation is always required
- `Protocol` member signatures — annotation is always required
- `@dataclass` / `@pydantic.dataclasses.dataclass` / attrs (`@define`, `@attr.s`) /
  pydantic `BaseModel` fields — the annotation is what makes the assignment a
  field; never redundant (issues #110, #39)
- `ClassVar` and `Final` — the qualifier itself is non-redundant even if the base type is inferrable

This does not conflict with PEP 526 or the conformance suite, which tests that annotations are respected when present but never requires them where inference suffices.

---

## Type Inference Overview {#TYPEINF-OVERVIEW}

### What Is Inferred {#TYPEINF-INFERRED}

Basilisk infers types for:

- **Local variable assignments** — `x = 42` → `x: int`
- **Return types** — for the expression forms supported by focused resolver/checker paths
- **Container literals** — list, dict, set, tuple elements (see §6)
- **`self` and `cls`** — always inferred, never annotated (see §4.4)
- **Walrus operator** — `(x := expr)` has the same type as `expr`
- **Comprehensions** — element type from the expression, collection type from the form
- **Generic instantiation** — in rule-specific TypeVar/bound/default cases
- **Narrowed types** — in the implemented guard and flow paths (see §9)

### Annotation policy {#TYPEINF-REQUIRED}

Inference and annotation policy are separate. PEP rules consume inferred and
declared types where available. Missing-annotation house rules are opt-in
configuration ([CHKARCH-CONFIGURATION-ONLY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIGURATION-ONLY));
the default PEP configuration does not turn an unannotated parameter or return
into an error merely because it is unannotated. `TypedDict`, `NamedTuple`,
Protocol, and qualifier syntax still require annotations where the typing spec
defines the annotation as part of the construct.

### Inference algorithm {#TYPEINF-ALGO}

The shared engine is conservative and primarily bottom-up: literal and
collection syntax produces an `InferredType`; unsupported expressions produce
`Unknown` rather than a guessed type. Expected-type and flow reasoning live in
focused rule/resolver paths, not in a general `infer_type(expr, expected)`
engine. Consolidating those paths is tracked in
[NARROWPLAN-INFERENCE](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INFERENCE).

## Variable Type Inference {#TYPEINF-VARS}

### Simple Assignment {#TYPEINF-VARS-SIMPLE}

```python
x = 42          # int
y = "hello"     # str
z = 3.14        # float
b = True        # bool
n = None        # None
```

The shared `infer_rhs` engine widens literal syntax to its base type. Focused typing rules may
retain literal values where the typing specification requires literal-sensitive behavior.

```python
# Shared RHS inference
STATUS = "active"   # str
MAX = 100           # int

# Function body — widened
def f() -> None:
    x = "active"   # str  (widened)
    y: Literal["active"] = "active"  # Literal["active"] (annotation drives it)
```

### Multiple Assignment {#TYPEINF-VARS-FLOW}

A standalone `FlowUnionTracker` can join recorded assignments into a union, but it is not
wired into the production resolver/checker control-flow graph. Full branch-sensitive
inference is tracked in the narrowing plan; the following is target behavior, not a current
general guarantee:

```python
def f(cond: bool) -> None:
    if cond:
        x = 1       # int
    else:
        x = "hi"    # str
    reveal_type(x)  # int | str
```

> **Authority**: [PEP 484 §Union types](https://peps.python.org/pep-0484/#union-types), [typing spec — narrowing](https://typing.readthedocs.io/en/latest/spec/narrowing.html).

### Annotated Variable {#TYPEINF-VARS-ANNOTATED}

When an annotation is present, the annotation **is** the declared type. The inferred RHS type must be assignable to it:

```python
x: int = 42         # declared: int; RHS infers int ✓
y: float = 42       # declared: float; RHS infers int; int is subtype of float ✓
z: str = 42         # assignment mismatch: int is not assignable to str
```

> **Authority**: [PEP 526 §Annotated assignment statements](https://peps.python.org/pep-0526/#annotated-assignment-statements):
> "If a variable has been annotated, all assignments to that variable will be type-checked."

### Augmented Assignment {#TYPEINF-VARS-AUGMENTED}

```python
x = 1
x += 2   # still int — the target keeps its existing type
```

Augmented assignment (`x op= rhs`) does not re-type the target: `x` keeps its previously declared or inferred type. Basilisk does not resolve `__iadd__`/`__add__` return types to compute a new type. Operator return-type inference is tracked by [NARROWPLAN-EXPRESSIONS](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-EXPRESSIONS). Augmented assignment is still analyzed for `Final`/`ReadOnly` reassignment violations and literal semantics.

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

When the opt-in annotation policy is enabled, an unannotated non-receiver parameter fires
`BSK-0001`. The unconfigured PEP default does not require annotations merely for style.

```python
def process(data):          # BSK-0001: parameter 'data' has no type annotation
    pass

def process(data: bytes):   # ✓
    pass
```

The only parameters inferred rather than annotated are:

- `self` in instance methods → inferred as `Self` (the containing class bound to `Self`)
- `cls` in class methods → inferred as `type[Self]`
- `__` (positional-only placeholder) → accepted as `Any` for compatibility

> **Authority**: [PEP 673 (Self type)](https://peps.python.org/pep-0673/) for `Self` semantics.

### Default Parameters {#TYPEINF-FUNC-DEFAULTS}

A default expression does not become a declared parameter type. Under the opt-in annotation
policy the parameter still needs an annotation.

```python
def connect(timeout=30):        # BSK-0001 — annotation required even with default
    pass

def connect(timeout: int = 30): # ✓
    pass
```

### Return Types {#TYPEINF-FUNC-RETURN}

Focused resolver/checker paths infer simple return expressions and validate them against a
declared return type. The opt-in annotation policy can separately require a public return
annotation; there is no universal PEP-default requirement.

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

`Self` participates in inheritance: a subclass calling an inherited method infers the subclass type, not the base class type.

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

The shared engine represents a lambda as `Callable[..., Unknown]`; it does not infer lambda
parameter or return types from an expected callable. The opt-in `BSK-0040` rule warns when a
module/class variable is assigned a lambda without a target annotation.

```python
transform: Callable[[int], str] = lambda x: str(x)  # declared target accepted
f = lambda x: x + 1   # BSK-0040 when the strictness tag is enabled
```

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

```python
[]              # list[Never]  — empty, element type is bottom
[1, 2, 3]       # list[int]
[1, "hi"]       # list[int | str]  — union, not object
[1, 2.0]        # list[int | float]
```

An annotation is checked separately for assignability; it is not pushed into literal
inference. Heterogeneous elements are joined as a union unconditionally.

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

Focused call-resolution paths bind simple TypeVars from argument/parameter shapes and apply
bounds, constraints, and defaults for the rules that own them. Basilisk does not yet have a
general bidirectional constraint solver, meet/join engine, or expected-return-type feedback
loop; consolidation is tracked by the narrowing plan.

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

A subtype passed to a constrained TypeVar is **widened to the matching constraint**, not kept at the subtype:

```python
class MyStr(str): pass

result = encode(MyStr("x"))   # AnyStr = str (not MyStr) — widened to constraint
reveal_type(result)            # str
```

> **Authority**: [Typing spec — Constrained TypeVars](https://typing.readthedocs.io/en/latest/spec/generics.html#constrained-type-variables), [Pyright type-inference docs](https://github.com/microsoft/pyright/blob/main/docs/type-inference.md).

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

`P` captures the full parameter specification; the wrapper has the same signature as the original.

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

Narrowing `isinstance` against a union:
- `if` branch: intersection of current type and checked type
- `else` branch: the **complement** — original type minus the checked type
- `isinstance(x, (A, B))`: `if` branch narrows to `A | B`

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
x = 42   # x keeps its declared type int | str
```

Basilisk does not narrow a variable's type on assignment: the variable retains its declared type. The flow environment used for `assert_type` checking (`crates/basilisk-resolver/src/visitor/assert_narrow.rs`) narrows only on supported guards; assignment statements do not update it. Assignment narrowing is tracked by [NARROWPLAN-FLOW](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-FLOW).

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

Basilisk performs **exhaustiveness checking** on match statements against union types: if all variants are handled, the `case _` branch (if present) has type `Never`.

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

```python
class Movie(TypedDict, total=False):
    title: str
    year: int

def f(m: Movie) -> None:
    if "title" in m:
        m["title"]   # the TypedDict type is NOT narrowed by the `in` check
```

Basilisk does not narrow `TypedDict` types via `"key" in td` checks; no `in`-comparison narrowing exists. Access checking for non-required keys is conservative, so no diagnostic depends on this narrowing. Key-existence (`in`-guard) narrowing is tracked in [NARROWPLAN-INFERENCE](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INFERENCE).

### Narrowing Scope Limitations {#TYPEINF-NARROWING-SCOPE}

Narrowing does **not** persist across:

- Function boundaries (inner functions capture the unnarrowed type unless the narrowing condition is proven stable)
- Loop bodies (a narrowed type before a loop is reset to the pre-loop type at each iteration)
- After reassignment of the narrowed variable

---

## Subtyping {#TYPEINF-SUBTYPING}

Basilisk implements both **nominal** and **structural** subtyping. `is_assignable_to(source, target)` answers "can a value of type `source` be used where type `target` is expected?"

> **Authority**: [PEP 484 §Subtype relationships](https://peps.python.org/pep-0484/), [PEP 544 §Protocols: Structural subtyping](https://peps.python.org/pep-0544/), [Python Typing Spec — Type system concepts](https://typing.readthedocs.io/en/latest/spec/concepts.html)

### Nominal Subtyping {#TYPEINF-SUBTYPING-NOMINAL}

`A` is a nominal subtype of `B` if `B` appears in `A.__mro__` (Method Resolution Order) — Python's standard class inheritance model.

```python
class Animal: ...
class Dog(Animal): ...

x: Animal = Dog()  # OK — Dog is a nominal subtype of Animal
```

**MRO resolution** is simplified: rules walk `ClassInfo.bases` transitively per class (no C3 linearization engine and no MRO cache in `ResolvedModule`); consolidation is tracked by [NARROWPLAN-SUBTYPING](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-SUBTYPING).

**Builtin numeric tower.** The typing-spec promotions ([Special cases for float and complex](https://typing.python.org/en/latest/spec/special-types.html#special-cases-for-float-and-complex)) hold: `bool`/`int` are accepted where `float` is expected, and `bool`/`int`/`float` where `complex` is expected. Two layers implement this:

- Annotation-text level (the conformance rules): `rules/shared.rs::is_numeric_subtype` encodes the full `bool <: int <: float <: complex` chain, mirrored by rule-local helpers (`narrowing_typeis`, `narrowing_typeis_2`, `overloads_evaluation`, `generics_typevartuple_callable`, `aliases_implicit`, `generics_syntax_scoping`).
- `InferredType` level: the annotation parser folds `complex` into `Float` (`types_parsing.rs`: `"float" | "complex" => Float`), so the `int → float` and `int`/`float → complex` promotions hold by construction (`bool` acceptance lives at the text level). Accepted trade-off: a `complex`-typed value is not rejected where `float` is expected — the conformance suite does not exercise that direction.

**Other builtin relations:**
- All classes <: `object` (`object` parses to the `Any` escape hatch for assignment purposes).
- `Never` <: everything (bottom type).
- There is **no** `bytearray <: bytes` promotion: the [current typing spec](https://typing.python.org/en/latest/spec/special-types.html#special-cases-for-float-and-complex) defines promotions only for `float`/`complex` (the historical `bytes` shorthand was removed), and no conformance test requires it. `bytearray` parses to `Named("bytearray")` and is assignable essentially only to itself, `object`, and `Any`.

### Protocol Structural Subtyping {#TYPEINF-SUBTYPING-PROTOCOL}

`A` structurally satisfies a `Protocol` `P` if `A` provides **all members** declared in `P` with compatible types — no explicit inheritance required.

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

### Implementation: `InferredType::is_assignable_to()` {#TYPEINF-SUBTYPING-IMPL}

Subtyping is decided by `InferredType::is_assignable_to(&self, other)` in `crates/basilisk-checker/src/types.rs` — a pure structural match over the `InferredType` enum, called on production paths by the compatibility rules (e.g. `rules/assignment_compatibility`, `rules/returns_compatibility`). It implements:

- `Any` / `Unknown` bidirectional compatibility and `Never` as bottom ([TYPEINF-SPECIAL-ANY](#TYPEINF-SPECIAL-ANY), [TYPEINF-SPECIAL-NEVER](#TYPEINF-SPECIAL-NEVER)).
- Partial, literal-level numeric relations: `int` (and `Literal` ints/floats) <: `float`, `Literal[True/False]` <: `bool`/`int`, plus `Literal`/`LiteralString`/`str` relations ([TYPEINF-SUBTYPING-NOMINAL](#TYPEINF-SUBTYPING-NOMINAL), [TYPEINF-SPECIAL-LITERALSTRING](#TYPEINF-SPECIAL-LITERALSTRING)). The full `bool <: int <: float <: complex` tower lives in the annotation-text-level helpers used by the conformance rules.
- `Optional`/`Union` decomposition: `A | B <: C` iff both sides do; `A <: A | B` ([TYPEINF-SUBTYPING-UNION](#TYPEINF-SUBTYPING-UNION)).
- Element-assignability (covariant) checks for `list`/`set`/`dict`; fixed-length, homogeneous `tuple[X, ...]`, and PEP 646 unpacked (`*tuple[...]`/`*Ts`) tuple matching ([TYPEINF-SUBTYPING-GENERIC](#TYPEINF-SUBTYPING-GENERIC), [TYPEINF-COLLECTIONS-TUPLES](#TYPEINF-COLLECTIONS-TUPLES)).
- Callable contravariant parameters / covariant return, with `...` params gradual ([TYPEINF-SUBTYPING-CALLABLE](#TYPEINF-SUBTYPING-CALLABLE)); `TypeForm` covariance.

`Named` types (user classes and unparameterised imports) compare by base name before `[`: `Foo[int]` and `Foo[float]` are treated as compatible. This is deliberate — without whole-program generic variance analysis, stricter matching would emit false positives, and the conformance gate holds `max_false_positives` at zero.

Nominal MRO walking and structural Protocol/TypedDict compatibility are NOT centralized here: they live in the per-conformance-area rule modules (`rules/protocols_*`, `rules/typeddicts_*`, and the class-bases-walking `is_subtype_of` helper in `rules/generics_basic_3/helpers.rs`). There is no shared `SubtypeContext` or MRO cache.

---

## Special Types {#TYPEINF-SPECIAL}

### `Any` {#TYPEINF-SPECIAL-ANY}

`Any` is bidirectionally compatible with all types — an **explicit escape hatch**, never inferred as a fallback; it appears only when written. Unannotated parameters do not silently become explicit `Any`; the opt-in annotation policy may report `BSK-0001`.

> **Authority**: [PEP 484 §The `Any` type](https://peps.python.org/pep-0484/#the-any-type): "Every type is consistent with `Any`."

### `Never` / `NoReturn` {#TYPEINF-SPECIAL-NEVER}

`Never` is the **bottom type**: no value has it, it is assignable to everything, and functions inferred to always raise return it.

```python
def fail(msg: str) -> Never:
    raise AssertionError(msg)
```

Basilisk uses `Never` for **exhaustiveness checking**:

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

`Self` represents the current class in a method's return or parameter type. Inferred automatically for `self`/`cls`; written explicitly for factory methods:

```python
from typing import Self

class Node:
    @classmethod
    def create(cls) -> Self:
        return cls()
```

> **Authority**: [PEP 673](https://peps.python.org/pep-0673/).

### `LiteralString` {#TYPEINF-SPECIAL-LITERALSTRING}

A supertype of all `Literal[str]` types, enforcing that only string literals (not dynamically constructed strings) reach security-sensitive APIs:

```python
from typing import LiteralString

def query(sql: LiteralString) -> None: ...

query("SELECT * FROM users")       # ✓ — literal
query("SELECT * FROM " + table)    # callables_annotation — not LiteralString
```

> **Authority**: [PEP 675](https://peps.python.org/pep-0675/).

---

## Distinctive Inference Behaviors {#TYPEINF-EXCEEDS}

Deliberate, distinctive behaviors of Basilisk's inference engine:

### Conservative `Unknown` Sentinel {#TYPEINF-EXCEEDS-NOUNKNOWN}

When syntactic RHS inference cannot determine a type (call expressions, `type(...)` calls, arbitrary expressions, lambda return types — `infer_rhs` in `crates/basilisk-checker/src/inference.rs`), it produces the internal sentinel `InferredType::Unknown` (`crates/basilisk-checker/src/types.rs`). `Unknown` is deliberately conservative: `is_assignable_to` treats it as bidirectionally compatible, and rules that encounter it generally suppress their diagnostic rather than guess. Recursive value-alias matching and `TypeForm` RHS validation are narrow exceptions that preserve real incompatibility diagnostics. `Unknown` never becomes explicit `Any` and does not alter the separately configured annotation policy.

### Strict Container Inference Always On {#TYPEINF-EXCEEDS-CONTAINERS}

Union-of-element-types inference applies to all containers unconditionally — no loose mode, no switch to disable.

### Exhaustive Pattern Matching Analysis {#TYPEINF-EXCEEDS-EXHAUSTIVE}

`match` statements on union types are checked for exhaustiveness with exact variant coverage.

### Lambda Warnings {#TYPEINF-EXCEEDS-LAMBDA}

With the `strictness` tag enabled, a module/class variable assigned a lambda without a target
annotation emits `BSK-0040`. The diagnostic is an annotation nudge, not evidence that lambda
parameters were otherwise contextually inferred.

### Annotation Required, Not Optional {#TYPEINF-EXCEEDS-REQUIRED}

When the require-annotation house rules are enabled, missing public-API annotations are diagnostics. They are not part of the unconfigured PEP default.

---

## Implementation notes {#TYPEINF-IMPL}

Shared inference lives in `basilisk-checker`:

- `inference.rs` — conservative RHS inference.
- `collection_inference.rs` — collection element joins.
- `types.rs` and `types_parsing.rs` — `InferredType`, assignability, and
  annotation parsing.
- Focused resolver/rule modules — narrowing, overload, Literal, Protocol, and
  TypedDict behavior.

The LSP analysis path is memoized by the Salsa database described in
[CHKARCH-INCREMENTAL-SALSA](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA).
A separate content-addressed cache serves opt-in cross-session CLI reuse.
