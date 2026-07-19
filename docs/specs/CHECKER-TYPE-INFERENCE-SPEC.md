# [TYPEINF-SPEC] Basilisk type inference {#TYPEINF}

Basilisk combines conservative shared inference with focused typing-rule algorithms. The default configuration follows the typing specification; optional house rules can require or discourage annotations without changing PEP behavior (see [TYPEINF-REDUNDANT]).

> **Authoritative references**: [PEP 484](https://peps.python.org/pep-0484/), [PEP 526](https://peps.python.org/pep-0526/), [Python Typing Spec](https://typing.python.org/en/latest/spec/), [Python Typing Conformance Suite](https://github.com/python/typing/tree/main/conformance)
>
> **Implementation**: Core inference engine (`inference.rs`, `collection_inference.rs`, `types.rs`, `types_parsing.rs`) is wired into rules E0011, E0013, E0014, E0120, and W0050.

---

## [TYPEINF-REDUNDANT] Redundant Annotation Principle {#TYPEINF-REDUNDANT}

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

The current rule applies to module-level assignments and ordinary class-body
attributes. Function-local declarations, loop/with targets, and walrus targets
are outside its resolver model and therefore remain silent rather than
guessing that an annotation is redundant.

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

## [TYPEINF-OVERVIEW] Type Inference Overview {#TYPEINF-OVERVIEW}

### [TYPEINF-INFERRED] What Is Inferred {#TYPEINF-INFERRED}

Basilisk infers types for:

- **Local variable assignments** — `x = 42` → `x: int`
- **Return types** — for the expression forms supported by focused resolver/checker paths
- **Container literals** — list, dict, set, tuple elements (see §6)
- **Receiver positions** — recognized by annotation-policy and focused `Self`
  rules; general receiver-result propagation is not yet an inference guarantee
- **Walrus operator** — `(x := expr)` has the same type as `expr`
- **Comprehensions** — element type from the expression, collection type from the form
- **Generic instantiation** — in rule-specific TypeVar/bound/default cases
- **Narrowed types** — in the implemented guard and flow paths (see §9)

### [TYPEINF-REQUIRED] Annotation policy {#TYPEINF-REQUIRED}

Inference and annotation policy are separate. PEP rules consume inferred and
declared types where available. Missing-annotation house rules are opt-in
configuration ([CHKARCH-CONFIGURATION-ONLY](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIGURATION-ONLY));
the default PEP configuration does not turn an unannotated parameter or return
into an error merely because it is unannotated. `TypedDict`, `NamedTuple`,
Protocol, and qualifier syntax still require annotations where the typing spec
defines the annotation as part of the construct.

### [TYPEINF-ALGO] Inference algorithm {#TYPEINF-ALGO}

The shared engine is conservative and primarily bottom-up: literal and
collection syntax produces an `InferredType`; unsupported expressions produce
`Unknown` rather than a guessed type. Expected-type and flow reasoning live in
focused rule/resolver paths, not in a general `infer_type(expr, expected)`
engine. Consolidating those paths is tracked in
[NARROWPLAN-INFERENCE](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INFERENCE).
The target architecture that supersedes this conservative core — bidirectional
checking over a subtype-constraint solver — is specified in
[TYPEINF-TARGET](#TYPEINF-TARGET).

## [TYPEINF-VARS] Variable Type Inference {#TYPEINF-VARS}

### [TYPEINF-VARS-SIMPLE] Simple Assignment {#TYPEINF-VARS-SIMPLE}

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

### [TYPEINF-VARS-FLOW] Multiple Assignment {#TYPEINF-VARS-FLOW}

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

### [TYPEINF-VARS-ANNOTATED] Annotated Variable {#TYPEINF-VARS-ANNOTATED}

When an annotation is present, the annotation **is** the declared type. The inferred RHS type must be assignable to it:

```python
x: int = 42         # declared: int; RHS infers int ✓
y: float = 42       # declared: float; RHS infers int; int is subtype of float ✓
z: str = 42         # assignment mismatch: int is not assignable to str
```

> **Authority**: [PEP 526 §Annotated assignment statements](https://peps.python.org/pep-0526/#annotated-assignment-statements):
> "If a variable has been annotated, all assignments to that variable will be type-checked."

### [TYPEINF-VARS-AUGMENTED] Augmented Assignment {#TYPEINF-VARS-AUGMENTED}

```python
x = 1
x += 2   # still int — the target keeps its existing type
```

Augmented assignment (`x op= rhs`) does not re-type the target: `x` keeps its previously declared or inferred type. Basilisk does not resolve `__iadd__`/`__add__` return types to compute a new type. Operator return-type inference is tracked by [NARROWPLAN-EXPRESSIONS](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-EXPRESSIONS). Augmented assignment is still analyzed for `Final`/`ReadOnly` reassignment violations and literal semantics.

### [TYPEINF-VARS-WALRUS] Walrus Operator {#TYPEINF-VARS-WALRUS}

```python
if (n := len(a)) > 10:
    reveal_type(n)  # int
```

The walrus operator `:=` assigns the value and the **expression type equals the assigned type**. The variable `n` is in scope for the remainder of the enclosing scope (not just the `if` block).

> **Authority**: [PEP 572](https://peps.python.org/pep-0572/): "The value of the target is the same as the value of the value expression."

---

## [TYPEINF-FUNC] Function Type Inference {#TYPEINF-FUNC}

### [TYPEINF-FUNC-PARAMS] Parameters {#TYPEINF-FUNC-PARAMS}

When the opt-in annotation policy is enabled, an unannotated non-receiver parameter fires
`BSK-0001` **only when the current engine cannot infer its type** — see
[TYPEINF-EXCEEDS-REQUIRED](#TYPEINF-EXCEEDS-REQUIRED) for the governing
principle. The unconfigured PEP default does not require annotations merely for style.

```python
def process(data):          # BSK-0001: nothing to infer 'data' from
    pass

def process(data: bytes):   # ✓
    pass
```

Parameters exempt from the opt-in missing-annotation rule:

- receiver-position `self` in instance methods
- receiver-position `cls` in class methods
- `__` (positional-only placeholder) → accepted as `Any` for compatibility
- parameters whose literal default determines the type —
  [TYPEINF-FUNC-DEFAULTS](#TYPEINF-FUNC-DEFAULTS)

> **Authority**: [PEP 673 (Self type)](https://peps.python.org/pep-0673/) for `Self` semantics.

### [TYPEINF-FUNC-DEFAULTS] Default Parameters {#TYPEINF-FUNC-DEFAULTS}

A type-determining literal default infers the parameter type, so `BSK-0001`
MUST NOT fire there — demanding an annotation the engine already knows is
redundant. A default that does **not** determine the type (`None`, empty
containers, calls, lambdas, arbitrary expressions) still requires one.

```python
def connect(timeout=30):             # ✓ — inferred as int from the default
    pass

def connect(timeout: int = 30):      # ✓ — explicit annotation always accepted
    pass

def connect(timeout=None):           # BSK-0001 — None does not determine T | None
    pass

def connect(timeout=make_default()): # BSK-0001 — call results are not inferable
    pass
```

The exemption is exactly as strong as the current engine
(`rhs_fully_determines_type` in `crates/basilisk-checker/src/inference.rs`):
scalar literals and non-empty containers of type-determining elements qualify;
nothing else does.

### [TYPEINF-FUNC-RETURN] Return Types {#TYPEINF-FUNC-RETURN}

Focused resolver/checker paths infer simple return expressions and validate them against a
declared return type. The opt-in annotation policy can separately require a return
annotation, but `BSK-0002` fires **only when the current engine cannot infer the
return type** ([TYPEINF-EXCEEDS-REQUIRED](#TYPEINF-EXCEEDS-REQUIRED)); there is
no universal PEP-default requirement.

```python
def f(x: int) -> int | str:    # ✓ — annotation matches inference
    if x > 0:
        return x               # int
    return "negative"          # str
```

A function is exempt from `BSK-0002` when every `return` is bare or carries a
type-determining literal, or the body has no `return` at all (inferred `None`):

```python
def answer():          # ✓ — inferred as int
    return 42

def log_it(msg: str):  # ✓ — no return: inferred as None
    print(msg)

def fetch(url: str):   # BSK-0002 — call result is not inferable
    return download(url)

def numbers():         # BSK-0002 — Generator[...] is not inferable from returns
    yield 1
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

### [TYPEINF-FUNC-SELFCLS] `self` and `cls` Receiver Handling {#TYPEINF-FUNC-SELFCLS}

The annotation-policy rule recognizes conventional receiver positions and does
not demand annotations for `self` or `cls`. Focused PEP 673 conformance rules
validate explicit `Self` annotations and their legal locations. The shared
expression engine does not yet synthesize a first-class receiver type or
propagate a subclass through an inherited `Self`-returning call; that work is
tracked by [NARROWPLAN-INFERENCE](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INFERENCE).

```python
class Builder:
    def set_name(self, name: str) -> Self:
        self._name = name
        return self

class AdvancedBuilder(Builder):
    pass

b = AdvancedBuilder().set_name("x")  # target behavior: AdvancedBuilder
```

> **Authority**: [PEP 673](https://peps.python.org/pep-0673/).

### [TYPEINF-FUNC-LAMBDA] Lambda Inference {#TYPEINF-FUNC-LAMBDA}

The shared engine represents a lambda as `Callable[..., Unknown]`; it does not infer lambda
parameter or return types from an expected callable. The opt-in `BSK-0040` rule warns when a
module/class variable is assigned a lambda without a target annotation.

```python
transform: Callable[[int], str] = lambda x: str(x)  # declared target accepted
f = lambda x: x + 1   # BSK-0040 when the strictness tag is enabled
```

### [TYPEINF-FUNC-OVERLOADS] Overloads {#TYPEINF-FUNC-OVERLOADS}

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

## [TYPEINF-COLLECTIONS] Collection Type Inference {#TYPEINF-COLLECTIONS}

### [TYPEINF-COLLECTIONS-LISTS] Lists {#TYPEINF-COLLECTIONS-LISTS}

```python
[]              # list[Never]  — empty, element type is bottom
[1, 2, 3]       # list[int]
[1, "hi"]       # list[int | str]  — union, not object
[1, 2.0]        # list[int | float]
```

An annotation is checked separately for assignability; it is not pushed into literal
inference. Heterogeneous elements are joined as a union unconditionally.

### [TYPEINF-COLLECTIONS-DICTS] Dicts {#TYPEINF-COLLECTIONS-DICTS}

```python
{}                  # dict[Never, Never]
{"a": 1, "b": 2}   # dict[str, int]
{"a": 1, "b": "x"} # dict[str, int | str]
{1: "a", "b": 2}   # dict[int | str, str | int]
```

### [TYPEINF-COLLECTIONS-SETS] Sets {#TYPEINF-COLLECTIONS-SETS}

```python
set()           # set[Never]
{1, 2, 3}       # set[int]
{1, "hi"}       # set[int | str]
```

### [TYPEINF-COLLECTIONS-TUPLES] Tuples {#TYPEINF-COLLECTIONS-TUPLES}

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

### [TYPEINF-COLLECTIONS-COMPREHENSIONS] Comprehensions {#TYPEINF-COLLECTIONS-COMPREHENSIONS}

```python
[x * 2 for x in range(10)]         # list[int]
{k: v for k, v in d.items()}       # dict[KT, VT]  where d: dict[KT, VT]
{x for x in "hello"}               # set[str]
(x for x in range(3))              # Generator[int, None, None]
```

---

## [TYPEINF-GENERICS] Generic Type Inference {#TYPEINF-GENERICS}

### [TYPEINF-GENERICS-TYPEVAR] TypeVar Solving {#TYPEINF-GENERICS-TYPEVAR}

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

### [TYPEINF-GENERICS-CONSTRAINED] Constrained TypeVars {#TYPEINF-GENERICS-CONSTRAINED}

The current call-rule path validates that an argument is compatible with one
of a constrained `TypeVar`'s alternatives. It does not yet expose a general
solved-return query, so the result-type and subtype-widening comments below are
typing-spec target semantics rather than a repository-wide inference claim.

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

### [TYPEINF-GENERICS-BOUND] Bound TypeVars {#TYPEINF-GENERICS-BOUND}

```python
C = TypeVar("C", bound="Comparable")

def sort(items: list[C]) -> list[C]: ...
```

TypeVar bound constraints are **upper bounds**: any subtype of `Comparable` satisfies `C`. The solved type is the argument type itself (not widened to the bound).

### [TYPEINF-GENERICS-VARIANCE] Variance Inference {#TYPEINF-GENERICS-VARIANCE}

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

### [TYPEINF-GENERICS-DEFAULTS] TypeVar Defaults {#TYPEINF-GENERICS-DEFAULTS}

Focused PEP 696 rules validate default declaration ordering, bounds, and
constraints. The shared constructor-expression engine does not yet synthesize
`Container[int]` from an omitted type argument; the example below states the
typing-spec result accepted by those focused rules.

```python
from typing import TypeVar

T = TypeVar("T", default=int)

class Container[T = int]:
    def get(self) -> T: ...

c = Container()         # Container[int] — default applied
d = Container[str]()    # Container[str] — explicit wins
```

> **Authority**: [PEP 696](https://peps.python.org/pep-0696/).

### [TYPEINF-GENERICS-PARAMSPEC] ParamSpec {#TYPEINF-GENERICS-PARAMSPEC}

Focused callable rules recognize `ParamSpec`, `P.args`, and `P.kwargs` shapes
and validate supported higher-order declarations. General decorator
application does not yet synthesize a wrapper signature from `P`; the full
signature-preservation example below is the PEP 612 target semantics.

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

## [TYPEINF-NARROWING] Type Narrowing {#TYPEINF-NARROWING}

### [TYPEINF-NARROWING-ISINSTANCE] `isinstance` Narrowing {#TYPEINF-NARROWING-ISINSTANCE}

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

### [TYPEINF-NARROWING-NONE] `is None` / `is not None` {#TYPEINF-NARROWING-NONE}

```python
def f(x: int | None) -> None:
    if x is None:
        reveal_type(x)  # None
    else:
        reveal_type(x)  # int
```

### [TYPEINF-NARROWING-TYPEOF] Exact `type(x) is C` Guards {#TYPEINF-NARROWING-TYPEOF}

`type(x) is C` narrows the positive branch to `C`. The negative branch removes
`C` only when `C` is known final; otherwise subclasses make exclusion unsound.

### [TYPEINF-NARROWING-EQ-LITERAL] Literal Equality Guards {#TYPEINF-NARROWING-EQ-LITERAL}

`x == literal` keeps that literal in the positive branch and removes it in the
negative branch. `!=` swaps those outcomes.

### [TYPEINF-NARROWING-IN-LITERAL] Literal Membership Guards {#TYPEINF-NARROWING-IN-LITERAL}

`x in (literal, ...)` intersects with the listed literals; the complementary
branch subtracts them. `not in` swaps those outcomes.

### [TYPEINF-NARROWING-TRUTHY] Truthiness Narrowing {#TYPEINF-NARROWING-TRUTHY}

```python
def f(x: str | None) -> None:
    if x:
        reveal_type(x)  # str  — None and "" are falsy, so x must be non-empty str
```

Truthiness narrowing removes falsy types from the union (`None`, `Literal[0]`, `Literal[""]`, `Literal[False]`) in the truthy branch, and narrows to falsy types in the falsy branch.

### [TYPEINF-NARROWING-ASSIGN] Assignment Narrowing {#TYPEINF-NARROWING-ASSIGN}

```python
x: int | str = get_value()
x = 42   # declared type remains int | str; flow type becomes int
```

Basilisk keeps the declared type for assignment validation, while its
flow-sensitive environment updates later uses of a simple name to the
synthesized RHS type. Complex targets stay conservative.

### [TYPEINF-NARROWING-MATCH] Pattern Matching Narrowing {#TYPEINF-NARROWING-MATCH}

```python
def process(cmd: Command) -> None:
    match cmd:
        case Quit():
            reveal_type(cmd)  # Quit
        case Move():
            reveal_type(cmd)  # Move
        case _:
            reveal_type(cmd)  # conservative remainder
```

Basilisk narrows a simple union subject for supported class patterns and
performs separate exhaustiveness checking. It does not yet infer types for
pattern-bound attributes or generally rewrite a wildcard case to `Never`.

> **Authority**: [PEP 634](https://peps.python.org/pep-0634/), [PEP 635](https://peps.python.org/pep-0635/).

### [TYPEINF-NARROWING-TYPEGUARD] TypeGuard {#TYPEINF-NARROWING-TYPEGUARD}

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

### [TYPEINF-NARROWING-TYPEIS] TypeIs {#TYPEINF-NARROWING-TYPEIS}

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

### [TYPEINF-NARROWING-ASSERT] `assert` Narrowing {#TYPEINF-NARROWING-ASSERT}

```python
x: int | None = get()
assert x is not None
reveal_type(x)  # int — narrowed after assert
```

Assertions narrow the type for all code after the `assert` statement (within the same flow path).

<a id="TYPEINF-NARROWING-DICTKEY"></a>

### [TYPEINF-NARROWING-TYPEDDICT-KEY] TypedDict Key Existence Narrowing {#TYPEINF-NARROWING-TYPEDDICT-KEY}

```python
class Movie(TypedDict, total=False):
    title: str
    year: int

class WithoutTitle(TypedDict):
    year: int

def f(m: Movie | WithoutTitle) -> None:
    if "title" in m:
        reveal_type(m)  # Movie
```

For a union of modeled `TypedDict` schemas, the positive branch keeps members
that declare the key. The negative branch removes a member only when the key is
required (and therefore always present); optional-key members remain possible.

### [TYPEINF-NARROWING-ISSUBCLASS] `issubclass` Guard Groundwork {#TYPEINF-NARROWING-ISSUBCLASS}

The resolver records supported `issubclass(x, C)` guards. Until first-class
`type[C]` object modeling lands, the checker deliberately returns the original
type in both branches rather than inventing an unsound narrowing.

### [TYPEINF-NARROWING-HASATTR] `hasattr` Guard Groundwork {#TYPEINF-NARROWING-HASATTR}

The resolver records `hasattr(x, "name")` guards. Synthetic protocol
intersection is not implemented yet, so both branches deliberately preserve
the original type.

### [TYPEINF-NARROWING-SCOPE] Narrowing Scope Limitations {#TYPEINF-NARROWING-SCOPE}

Narrowing does **not** persist across:

- Function boundaries (inner functions capture the unnarrowed type unless the narrowing condition is proven stable)
- Loop bodies (a narrowed type before a loop is reset to the pre-loop type at each iteration)
- After reassignment, the prior narrow is replaced by the simple RHS flow type

---

## [TYPEINF-SUBTYPING] Subtyping {#TYPEINF-SUBTYPING}

Basilisk implements both **nominal** and **structural** subtyping. `is_assignable_to(source, target)` answers "can a value of type `source` be used where type `target` is expected?"

> **Authority**: [PEP 484 §Subtype relationships](https://peps.python.org/pep-0484/), [PEP 544 §Protocols: Structural subtyping](https://peps.python.org/pep-0544/), [Python Typing Spec — Type system concepts](https://typing.readthedocs.io/en/latest/spec/concepts.html)

### [TYPEINF-SUBTYPING-NOMINAL] Nominal Subtyping {#TYPEINF-SUBTYPING-NOMINAL}

`A` is a nominal subtype of `B` if `B` appears in `A.__mro__` (Method Resolution Order) — Python's standard class inheritance model.

```python
class Animal: ...
class Dog(Animal): ...

x: Animal = Dog()  # OK — Dog is a nominal subtype of Animal
```

Nominal-subtyping rules may walk `ClassInfo.bases` transitively; the shared MRO
model remains tracked by
[NARROWPLAN-SUBTYPING](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-SUBTYPING).

**Builtin numeric tower.** The typing-spec promotions ([Special cases for float and complex](https://typing.python.org/en/latest/spec/special-types.html#special-cases-for-float-and-complex)) hold: `bool`/`int` are accepted where `float` is expected, and `bool`/`int`/`float` where `complex` is expected. Two layers implement this:

- Annotation-text level (the conformance rules): `rules/shared.rs::is_numeric_subtype` encodes the full `bool <: int <: float <: complex` chain, mirrored by rule-local helpers (`narrowing_typeis`, `narrowing_typeis_2`, `overloads_evaluation`, `generics_typevartuple_callable`, `aliases_implicit`, `generics_syntax_scoping`).
- `InferredType` level: the annotation parser folds `complex` into `Float` (`types_parsing.rs`: `"float" | "complex" => Float`), so the `int → float` and `int`/`float → complex` promotions hold by construction (`bool` acceptance lives at the text level). Accepted trade-off: a `complex`-typed value is not rejected where `float` is expected — the conformance suite does not exercise that direction.

**Other builtin relations:**
- All classes <: `object` (`object` parses to the `Any` escape hatch for assignment purposes).
- `Never` <: everything (bottom type).
- There is **no** `bytearray <: bytes` promotion: the [current typing spec](https://typing.python.org/en/latest/spec/special-types.html#special-cases-for-float-and-complex) defines promotions only for `float`/`complex` (the historical `bytes` shorthand was removed), and no conformance test requires it. `bytearray` parses to `Named("bytearray")` and is assignable essentially only to itself, `object`, and `Any`.

### [TYPEINF-SUBTYPING-PROTOCOL] Protocol Structural Subtyping {#TYPEINF-SUBTYPING-PROTOCOL}

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

### [TYPEINF-SUBTYPING-TYPEDDICT] TypedDict Structural Subtyping {#TYPEINF-SUBTYPING-TYPEDDICT}

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

### [TYPEINF-SUBTYPING-GENERIC] Generic Subtyping {#TYPEINF-SUBTYPING-GENERIC}

Generic types combine nominal subtyping with variance:

```python
class Animal: ...
class Dog(Animal): ...

dogs: list[Dog] = []
x: list[Animal] = dogs       # ERROR — two typed lists are invariant
y: Sequence[Animal] = dogs   # OK — Sequence is covariant
```

**Variance rules** for generic type parameters:
- **Covariant** (`T_co`): `G[A]` <: `G[B]` if `A` <: `B`. Read-only containers (`Sequence`, `Iterator`, `FrozenSet`, `tuple`).
- **Contravariant** (`T_contra`): `G[A]` <: `G[B]` if `B` <: `A`. Write-only positions (function parameters in `Callable`).
- **Invariant** (default): `G[A]` <: `G[B]` only if `A` == `B`. Mutable containers (`list`, `dict`, `set`).

**Generic subtyping algorithm**:
1. Check nominal subtyping: does source class's MRO include the target's base class?
2. Find the TypeVar substitution: how does the source specialize the target's TypeVars?
3. Apply variance rules to each TypeVar position.

### [TYPEINF-SUBTYPING-UNION] Union and Special-Form Subtyping {#TYPEINF-SUBTYPING-UNION}

- `A` <: `A | B` (always — a type is a subtype of any union containing it)
- `A | B` <: `C` only if `A` <: `C` AND `B` <: `C`
- `Optional[T]` = `T | None`
- `Any` is bidirectionally compatible with all types (not a real subtype, an escape hatch)
- `Never` <: everything (bottom type, assignable to all types)
- the simplified annotation parser treats `object` as a gradual `Any` spelling

### [TYPEINF-SUBTYPING-CALLABLE] Callable Subtyping {#TYPEINF-SUBTYPING-CALLABLE}

Callable subtyping follows **parameter contravariance** and **return covariance**:

```python
# Callable[[ParamTypes], ReturnType]
# Parameters are contravariant, return type is covariant

f: Callable[[Animal], Dog]  # accepts Animal, returns Dog
g: Callable[[Dog], Animal]  # accepts Dog, returns Animal

# f IS assignable to g: it accepts every Dog and returns a Dog (an Animal)
# g is NOT assignable to f: Animal (return of g) is not subtype of Dog (return of f)
```

**Callable compatibility rules**:
- Source return type must be a **subtype** of target return type (covariant).
- Target parameter types must be **subtypes** of source parameter types (contravariant).
- Source may have fewer required parameters than target (extra defaults OK).
- `*args`/`**kwargs` in source accepts any parameter count in target.
- `Callable[..., R]` (ellipsis params) is compatible with any parameter signature.

> **Authority**: [PEP 484 §Callable](https://peps.python.org/pep-0484/#callable), [Typing spec — Callables](https://typing.readthedocs.io/en/latest/spec/callables.html)

### [TYPEINF-SUBTYPING-IMPL] Implementation: `InferredType::is_assignable_to()` {#TYPEINF-SUBTYPING-IMPL}

Subtyping is decided by `InferredType::is_assignable_to(&self, other)` in `crates/basilisk-checker/src/types.rs` — a pure structural match over the `InferredType` enum, called on production paths by the compatibility rules (e.g. `rules/assignment_compatibility`, `rules/returns_compatibility`). It implements:

- `Any` / `Unknown` bidirectional compatibility and `Never` as bottom ([TYPEINF-SPECIAL-ANY](#TYPEINF-SPECIAL-ANY), [TYPEINF-SPECIAL-NEVER](#TYPEINF-SPECIAL-NEVER)).
- Partial, literal-level numeric relations: `int` (and `Literal` ints/floats) <: `float`, `Literal[True/False]` <: `bool`/`int`, plus `Literal`/`LiteralString`/`str` relations ([TYPEINF-SUBTYPING-NOMINAL](#TYPEINF-SUBTYPING-NOMINAL), [TYPEINF-SPECIAL-LITERALSTRING](#TYPEINF-SPECIAL-LITERALSTRING)). The full `bool <: int <: float <: complex` tower lives in the annotation-text-level helpers used by the conformance rules.
- `Optional`/`Union` decomposition: `A | B <: C` iff both sides do; `A <: A | B` ([TYPEINF-SUBTYPING-UNION](#TYPEINF-SUBTYPING-UNION)).
- Bidirectional element compatibility (invariance, with gradual `Any`/`Unknown` consistency) for mutable `list`/`set`/`dict`; fixed-length, homogeneous `tuple[X, ...]`, and PEP 646 unpacked (`*tuple[...]`/`*Ts`) tuple matching ([TYPEINF-SUBTYPING-GENERIC](#TYPEINF-SUBTYPING-GENERIC), [TYPEINF-COLLECTIONS-TUPLES](#TYPEINF-COLLECTIONS-TUPLES)).
- Callable contravariant parameters / covariant return, with `...` params gradual ([TYPEINF-SUBTYPING-CALLABLE](#TYPEINF-SUBTYPING-CALLABLE)); `TypeForm` covariance.

`Named` types (user classes and unparameterised imports) compare by base name before `[`: `Foo[int]` and `Foo[float]` are treated as compatible. This is deliberate — without whole-program generic variance analysis, stricter matching would emit false positives, and the conformance gate holds `max_false_positives` at zero.

Nominal MRO walking and structural Protocol/TypedDict compatibility are NOT centralized here: they live in the per-conformance-area rule modules (`rules/protocols_*`, `rules/typeddicts_*`, and the class-bases-walking `is_subtype_of` helper in `rules/generics_basic_3/helpers.rs`). There is no shared `SubtypeContext` or MRO cache.

---

## [TYPEINF-SPECIAL] Special Types {#TYPEINF-SPECIAL}

### [TYPEINF-SPECIAL-ANY] `Any` {#TYPEINF-SPECIAL-ANY}

`Any` is bidirectionally compatible with all types. It arises from explicit
`Any` and from typing-defined gradual spellings such as `object` and bare
generics in the simplified annotation parser; it is never the fallback for a
failed expression inference (that sentinel is `Unknown`). Unannotated
parameters do not silently become explicit `Any`; the opt-in annotation policy
may report `BSK-0001`.

> **Authority**: [PEP 484 §The `Any` type](https://peps.python.org/pep-0484/#the-any-type): "Every type is consistent with `Any`."

### [TYPEINF-SPECIAL-NEVER] `Never` / `NoReturn` {#TYPEINF-SPECIAL-NEVER}

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

### [TYPEINF-SPECIAL-SELF] `Self` {#TYPEINF-SPECIAL-SELF}

`Self` represents the current class in a method's return or parameter type.
Focused rules validate explicit PEP 673 uses. Receiver parameters are exempt
from missing-annotation policy, but the shared engine does not yet synthesize a
first-class automatic `Self`/`type[Self]` receiver type (see
[TYPEINF-FUNC-SELFCLS](#TYPEINF-FUNC-SELFCLS)).

```python
from typing import Self

class Node:
    @classmethod
    def create(cls) -> Self:
        return cls()
```

> **Authority**: [PEP 673](https://peps.python.org/pep-0673/).

### [TYPEINF-SPECIAL-LITERALSTRING] `LiteralString` {#TYPEINF-SPECIAL-LITERALSTRING}

A supertype of all `Literal[str]` types, enforcing that only string literals (not dynamically constructed strings) reach security-sensitive APIs:

```python
from typing import LiteralString

def query(sql: LiteralString) -> None: ...

query("SELECT * FROM users")       # ✓ — literal
query("SELECT * FROM " + table)    # callables_annotation — not LiteralString
```

> **Authority**: [PEP 675](https://peps.python.org/pep-0675/).

---

## [TYPEINF-EXCEEDS] Distinctive Inference Behaviors {#TYPEINF-EXCEEDS}

Deliberate, distinctive behaviors of Basilisk's inference engine:

### [TYPEINF-EXCEEDS-NOUNKNOWN] Conservative `Unknown` Sentinel {#TYPEINF-EXCEEDS-NOUNKNOWN}

When syntactic RHS inference cannot determine a type (call expressions, `type(...)` calls, arbitrary expressions, lambda return types — `infer_rhs` in `crates/basilisk-checker/src/inference.rs`), it produces the internal sentinel `InferredType::Unknown` (`crates/basilisk-checker/src/types.rs`). `Unknown` is deliberately conservative: `is_assignable_to` treats it as bidirectionally compatible, and rules that encounter it generally suppress their diagnostic rather than guess. Recursive value-alias matching and `TypeForm` RHS validation are narrow exceptions that preserve real incompatibility diagnostics. `Unknown` never becomes explicit `Any` and does not alter the separately configured annotation policy.

### [TYPEINF-EXCEEDS-CONTAINERS] Strict Container Inference Always On {#TYPEINF-EXCEEDS-CONTAINERS}

Union-of-element-types inference applies to all containers unconditionally — no loose mode, no switch to disable.

### [TYPEINF-EXCEEDS-EXHAUSTIVE] Exhaustive Pattern Matching Analysis {#TYPEINF-EXCEEDS-EXHAUSTIVE}

`match` statements on union types are checked for exhaustiveness with exact variant coverage.

### [TYPEINF-EXCEEDS-LAMBDA] Lambda Warnings {#TYPEINF-EXCEEDS-LAMBDA}

With the `strictness` tag enabled, a module/class variable assigned a lambda without a target
annotation emits `BSK-0040`. The diagnostic is an annotation nudge, not evidence that lambda
parameters were otherwise contextually inferred.

### [TYPEINF-EXCEEDS-REQUIRED] Annotation Required Only Where Inference Fails {#TYPEINF-EXCEEDS-REQUIRED}

When the require-annotation house rules are enabled, missing public-API annotations are
diagnostics. They are not part of the unconfigured PEP default.

**Inference-first principle:** a missing-annotation rule MUST NOT fire where the
current engine already infers the type — demanding a type the checker knows is
redundant noise (and contradicts `BSK-0050`, which flags exactly such
annotations as redundant). Each rule fires only where inference fails:

| Rule | Exempt (inferable today) | Still fires (not inferable today) |
|---|---|---|
| `BSK-0001` (parameter) | `self`/`cls`; type-determining literal default ([TYPEINF-FUNC-DEFAULTS](#TYPEINF-FUNC-DEFAULTS)) | no default; `None`/empty-container/call/lambda default |
| `BSK-0002` (return) | all returns bare/literal, or no returns → `None` ([TYPEINF-FUNC-RETURN](#TYPEINF-FUNC-RETURN)) | any uninferable return; generators |
| `BSK-0003` (module var) | any RHS except empty containers / `None` | `[]`, `{}`, `None` |
| `BSK-0005` (class attr) | scalar/tuple literal RHS | everything else |
| `BSK-0004` (`*args`/`**kwargs`), `BSK-0040` (lambda) | — (nothing inferable today) | always |

The exemptions are exactly as strong as today's inference and MUST widen as the
engine grows ([TYPEINF-TARGET](#TYPEINF-TARGET)) — never the reverse.

---

## [TYPEINF-IMPL] Implementation notes {#TYPEINF-IMPL}

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

---

## [TYPEINF-TARGET] Target inference architecture {#TYPEINF-TARGET}

This section specifies the design of the next-generation inference engine.
The current conservative core ([TYPEINF-ALGO](#TYPEINF-ALGO)) is superseded by
this design; delivery is staged in
[NARROWPLAN-INFERENCE](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INFERENCE).
The design is oriented toward
[PEP 827 – Type Manipulation](https://peps.python.org/pep-0827/) — the engine
must be powerful enough to host PEP 827-style conditional/mapped types — but
implementing PEP 827 itself is out of scope; only the inference-engine
groundwork is specified here. Every claim below is grounded in the research
survey in [TYPEINF-RESEARCH](#TYPEINF-RESEARCH). The outcome requirement —
inference measurably superior to every officially-recognized competitor,
proven and held by a self-measured ratcheted scoreboard — is defined in
[NARROWPLAN-SUPERIORITY](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-SUPERIORITY).

### [TYPEINF-TARGET-BIDIRECTIONAL] Bidirectional core {#TYPEINF-TARGET-BIDIRECTIONAL}

Bidirectional typing (Pierce–Turner *Local Type Inference*;
Dunfield–Krishnaswami survey — see
[TYPEINF-RESEARCH-THEORY](#TYPEINF-RESEARCH-THEORY)) is the backbone, layered
over a subtyping-constraint solver rather than global Hindley–Milner. The
Stage 0 engine exposes two total entry points (unsupported shapes return
`Unknown` rather than panicking):

- `synth(e) → τ` — *synthesis*: infer a type bottom-up;
- `check(e, τ)` — *checking*: verify against an expected type propagated
  top-down.

`check` currently propagates expected types into supported container literals,
comprehensions, lambda parameters, and known call arguments. General
higher-order/generic propagation remains a later stage.
This is deliberately where Basilisk aims to beat Pyrefly, whose context
propagation is heuristic ("take one peek ahead"), and ty, which only recently
added outside-in inference (see
[TYPEINF-RESEARCH-COMPETITORS](#TYPEINF-RESEARCH-COMPETITORS)).

### [TYPEINF-TARGET-CONSTRAINTS] Constraint architecture {#TYPEINF-TARGET-CONSTRAINTS}

A **two-stage constraint architecture** (Pottier–Rémy) is implemented for the
Stage 0 expression engine: a
constraint-generation pass over the AST produces subtype constraints
(`τ₁ <: τ₂`), and a separate solver resolves them.

- Type variables are **bounded and polar**: each carries explicit lower/upper
  bounds (like Pyright's type intervals and Pyrefly's `Var`), with the
  input/output polarity discipline borrowed from Dolan's algebraic subtyping
  and Parreaux's Simple-sub — **without** committing to full biunification
  (see [TYPEINF-RESEARCH-THEORY](#TYPEINF-RESEARCH-THEORY) and the transfer
  risk noted there).
- **Generalization is deferred**: infer `list[Var{lower=Literal[1]}]` and
  settle `Var` only at first constraining use. This preserves
  literal/generic precision (`list[int]` vs `list[Literal[1]]`) instead of
  Pyrefly's eager `Literal[1] → int` widening, while staying
  gradual-guarantee-safe.

### [TYPEINF-TARGET-GRADUAL] Gradual guarantee {#TYPEINF-TARGET-GRADUAL}

The gradual guarantee (Siek, Vitousek, Cimini, Boyland, *Refined Criteria for
Gradual Typing* — see [TYPEINF-RESEARCH-GRADUAL](#TYPEINF-RESEARCH-GRADUAL))
is an explicit, testable invariant: consistency (`~`) replaces subtyping where
`Any` is involved, and **removing an annotation must never introduce a new
static error**. This forbids inferring over-precise types from unannotated
code (Pyrefly's known risk) and is enforced by a differential test suite that
strips annotations and asserts no new errors. This matches ty's design
position and composes with the conservative-`Unknown` behavior in
[TYPEINF-EXCEEDS-NOUNKNOWN](#TYPEINF-EXCEEDS-NOUNKNOWN).

### [TYPEINF-TARGET-NARROWING] Flow-sensitive narrowing {#TYPEINF-TARGET-NARROWING}

The current flow engine uses intersection/subtraction over a modeled set of
guards documented in [TYPEINF-NARROWING](#TYPEINF-NARROWING). A future stage
extends this into occurrence typing (Tobin-Hochstadt–Felleisen; Castagna et
al.) over a Salsa-backed use-def map with `phi`/join operators, synthetic
protocol intersections for `hasattr`, and inference-driven reachability.

#### [TYPEINF-NARROWING-ATTR-CALLS] Attribute narrowing across calls {#TYPEINF-NARROWING-ATTR-CALLS}

**Decision (Stage 2):** attribute narrowing (`if x.attr is not None:`)
**survives intervening calls by default.** Any call *could* re-enter and
mutate the attribute, so this default is deliberately unsound — but treating
every call as an invalidation discards nearly every attribute narrow in real
code, which is why the usable behavior is the ecosystem norm. Projects that
want the sound-but-strict behavior set
`narrow-attributes-across-calls = false` under `[tool.basilisk]`
(`BasiliskConfig::narrow_attributes_across_calls`; `None`/unset means the
usable default `true`). The knob is parsed and preserved by configuration now.
Attribute narrowing itself is not implemented, so no checker path consults the
knob yet; Stage 2 must wire it at every future attribute-narrowing application.

### [TYPEINF-TARGET-INCREMENTAL] Incrementality {#TYPEINF-TARGET-INCREMENTAL}

Definition-level Salsa queries are implemented. Expression-level tracked
queries, a compact cross-file interface/signature boundary, and cycle fixpoint
iteration are target work; they are not claimed by the current implementation.
If fine-grained queries cause memory blowup on large targets, AST/binding
eviction (keep only interfaces) sits behind the query layer — see the threshold in
[NARROWPLAN-STAGES](../plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-STAGES).

### [TYPEINF-TARGET-TYPELEVEL] Type-level evaluation (PEP 827 readiness) {#TYPEINF-TARGET-TYPELEVEL}

`tyeval.rs` implements isolated Stage 3 groundwork: a bounded, memoized,
call-by-need evaluator for ground/alias/parameter/list/tuple/union terms with a
gradual `Divergent` fallback and guarded-recursion acceptance. It is not wired
into annotation resolution and does not yet implement conditional or mapped
types. The target extension is constrained as follows.

Type-level computation with conditional/mapped types is Turing-complete
territory (proven for both TypeScript and Python type hints — see
[TYPEINF-RESEARCH-TYPELEVEL](#TYPEINF-RESEARCH-TYPELEVEL)), so the only safe
engineering path is **bounded evaluation**: a call-by-need
normalization-by-evaluation engine over type-level functions, built as
memoized Salsa queries returning types in weak-head normal form (whnf), with:

- **fuel/depth bounds** (TypeScript's instantiation-depth model);
- **memoization** of normalized results;
- a **`Divergent`/`@Todo`-style fallback** that preserves the gradual
  guarantee when evaluation is truncated;
- **GHC-style acceptance conditions** (Paterson/Coverage analogues) that
  statically reject obviously-nonterminating type-level definitions, with an
  opt-in "undecidable" escape hatch.

Mapped types are **kind `Type → Type` operators**; conditional types are
guarded rewrites keyed on a consistency/assignability check (`IsAssignable`
in PEP 827), evaluated lazily so unused branches never diverge. Because
bounded evaluation cannot be complete, some legitimate type-level programs
will hit the bound — an inherent limitation, not an implementation gap.

---

## Research grounding (non-normative) {#TYPEINF-RESEARCH}

The literature survey and competitor analysis behind
[TYPEINF-TARGET](#TYPEINF-TARGET).

### Key findings {#TYPEINF-RESEARCH-FINDINGS}

1. **Global HM/Algorithm W is the wrong foundation for Python.** Damas–Milner
   inference relies on unification (equational constraints) and
   let-generalization, and it does not natively accommodate subtyping,
   mutation (the value restriction problem), or gradual `Any`. Python has all
   three. Every serious Python checker (Pyright, mypy, Pyrefly, ty) instead
   uses **bidirectional checking plus local subtype-constraint solving**. Neil
   Mitchell (Pyrefly) explicitly contrasts Python's origin with "let's pick a
   really nice structure like Hindley-Milner"; Python "grafts types back onto"
   existing programs, so subtype constraints, not unification, are the core.

2. **Bidirectionality is the consensus architecture, and it is where inference
   quality is won or lost.** Pierce & Turner introduced local type inference
   with two modes — *synthesis* (infer a type bottom-up) and *checking*
   (verify against an expected type propagated top-down). Pyright, Pyrefly,
   and ty all use it. Crucially, ty only recently added bidirectional
   ("outside-in") inference — for most of its life it did pure "inside-out"
   inference — and Pyrefly's context propagation is present but heuristic
   ("take one peek ahead"). This is Basilisk's single biggest opportunity.

3. **Algebraic subtyping (MLsub / Dolan–Mycroft; Parreaux's Simple-sub) is the
   state of the art for inference *with* subtyping** and is directly relevant,
   but adopting it wholesale for Python is risky. It gives principal types and
   compact type simplification via biunification and polar (input/output)
   types, and Parreaux's Simple-sub "can be implemented efficiently in under
   500 lines of code (including parsing, simplification, and pretty-printing)"
   (Parreaux, ICFP 2020, <https://doi.org/10.1145/3409006>). However, Python's
   nominal classes, invariant generics, overloads, and gradual `Any` do not
   map cleanly onto MLsub's structural, fully-inferred model. The
   recommendation is to **borrow its ideas (polar types, bounded type
   variables, constraint simplification) selectively** rather than commit to
   full biunification.

4. **Pyrefly's concrete design is now well-documented and beatable on specific
   axes.** Pyrefly compiles each file through six phases (Code → AST → Exports
   → Bindings → Answers → Interface), works at **file-level** incremental
   granularity (not expression/definition level), uses subtype constraints
   with unification variables ("Vars") and cycle-breaking thunks, and
   deliberately chooses **usability over soundness** (e.g., it allows
   attribute narrowing across function calls that Pyre rejected). Its
   inference is "aggressive": it infers concrete unions in unannotated code
   and reports errors there, at the cost of false positives. Pyrefly's
   official site advertises type checking "over 1.85 million lines of code per
   second" (tested on Meta infrastructure, 166 cores / 228 GB RAM), and Meta
   reports re-checking Instagram's ~20M-line codebase in ~13.4 seconds — in
   Mitchell's words, "projects that used to take 14 minutes to type check that
   are now down to more like five seconds."

5. **ty (Astral) is the philosophical opposite and the more sophisticated
   incremental engine.** ty is built on **Salsa with fine-grained,
   multi-granularity queries** (scope-, definition-, expression-, and
   statement-level), uses **fixpoint iteration with a `Divergent` type** for
   recursive cycles, models types as a single `Type` enum with a nested
   `DynamicType` (Any/Unknown/Todo/Divergent), has **first-class intersection
   types** driving narrowing, does **reachability analysis based on type
   inference** (not pattern matching), and enforces the **gradual guarantee**.
   Its generics use an explicit constraint solver
   (`SpecializationBuilder`/`ConstraintSetBuilder`). Astral reports that
   "after editing a load-bearing file in the PyTorch repository, ty recomputes
   diagnostics in 4.7ms: 80x faster than Pyright (386ms) and 500x faster than
   Pyrefly (2.38 seconds)."

6. **Type-level computation is provably unbounded.** TypeScript's type system
   is Turing-complete (Microsoft/TypeScript issue #14833), and Python's type
   hints are Turing-complete: Ori Roth (arXiv:2208.14755, submitted 31 Aug
   2022; published ECOOP 2023, LIPIcs vol. 263, pp. 44:1–44:15) states,
   "Grigore showed that Java generics are Turing complete by describing a
   reduction from Turing machines to Java subtyping. We apply Grigore's
   algorithm to Python type hints and deduce that they are Turing complete."
   PEP 827 explicitly models conditional/mapped types on TypeScript. Therefore
   Basilisk's type-level evaluator must be a bounded normalizer. The proven
   engineering strategies come from Haskell type classes (GHC's Paterson
   Conditions and Coverage Condition ensuring instance-resolution termination,
   relaxed by `UndecidableInstances`) and from checkers' recursion/fuel
   limits.

7. **The gradual guarantee is a precise, mechanizable specification** (Siek,
   Vitousek, Cimini, Boyland, SNAPL 2015): consistency (`~`) replaces
   subtyping where `Any` is involved, and adding precision to annotations must
   never turn a well-typed program ill-typed (static guarantee) nor change its
   behavior except to surface errors (dynamic guarantee). ty treats this as a
   design invariant; Basilisk should encode it as a differential test suite.

### Foundational type-inference theory {#TYPEINF-RESEARCH-THEORY}

**Hindley–Milner / Algorithm W.**

- Robin Milner, "A Theory of Type Polymorphism in Programming," *JCSS* 17(3),
  1978. Introduces Algorithm W and unification-based let-polymorphism.
- Luis Damas & Robin Milner, "Principal Type-Schemes for Functional Programs,"
  POPL 1982, pp. 207–212. <https://doi.org/10.1145/582153.582176> —
  establishes principal types for the HM system.

Why HM is a poor global fit for Python: (a) **Subtyping.** HM unification
solves *equality* constraints (`τ₁ = τ₂`); subtyping requires *inequality*
constraints (`τ₁ <: τ₂`) with upper/lower bounds, which unification cannot
express without extension. Adding subtyping to HM is a decades-long research
problem (Aiken & Wimmers 1993; Fuh & Mishra 1988; Mitchell 1991; Pottier 2001;
Dolan & Mycroft 2017). (b) **Mutation.** Let-generalization is unsound with
mutable references (the value restriction); Python is pervasively mutable, and
invariant containers (`list[T]`) make naive generalization wrong — exactly the
`list[Literal[1]]` vs `list[int]` problem Pyrefly demonstrates. (c) **Gradual
types.** `Any` is neither a top nor bottom type under subtyping; it
participates in *consistency*, not subtyping, so it cannot be modeled as an HM
type variable.

**Bidirectional typing.**

- Benjamin C. Pierce & David N. Turner, "Local Type Inference," POPL 1998,
  pp. 252–265 (<https://doi.org/10.1145/268946.268967>); full version *ACM
  TOPLAS* 22(1):1–44, Jan 2000 (<https://doi.org/10.1145/345099.345100>).
  PDF: <https://www.cis.upenn.edu/~bcpierce/papers/lti-toplas.pdf>. Recovers
  annotations using only *locally* adjacent syntax-tree information; solves
  type arguments in applications via a local constraint solver over
  upper/lower bounds, and propagates expected types downward into
  abstractions.
- Jana Dunfield & Neel Krishnaswami, "Bidirectional Typing," *ACM Computing
  Surveys* 54(5), Article 98, May 2021, 38 pp.
  <https://doi.org/10.1145/3450952>. The canonical survey: *checking* mode
  supports features whose full inference is undecidable; *synthesis* reduces
  annotation burden; the split improves **error locality**. Documents the
  "Pfenning recipe" for deriving bidirectional rules.
- Jana Dunfield & Neelakantan R. Krishnaswami, "Complete and Easy
  Bidirectional Typechecking for Higher-Rank Polymorphism," ICFP 2013,
  pp. 429–442. <https://doi.org/10.1145/2500365.2500582>. Shows how a small,
  implementable bidirectional algorithm with ordered existential-variable
  contexts scales to higher-rank (rank-N) polymorphism — directly relevant to
  Python's `Callable`/`ParamSpec` higher-order types.

**Constraint-based inference.**

- Martin Odersky, Martin Sulzmann & Martin Wehr, "Type Inference with
  Constrained Types" (HM(X)), *Theory and Practice of Object Systems*
  5(1):35–55, 1999. Parameterizes HM over a constraint domain X (subtyping,
  records, type classes).
- François Pottier & Didier Rémy, "The Essence of ML Type Inference,"
  Chapter 10 of *Advanced Topics in Types and Programming Languages*
  (B. Pierce, ed.), MIT Press, 2005, pp. 389–489.
  PDF: <http://gallium.inria.fr/~fpottier/publis/emlti-final.pdf>. The
  definitive treatment of **constraint generation + constraint solving as a
  two-stage architecture**, with a built-in binary subtyping predicate `≤`.
  This two-stage separation (generate constraints in one AST pass, solve
  separately) is the architecture Basilisk adopts
  ([TYPEINF-TARGET-CONSTRAINTS](#TYPEINF-TARGET-CONSTRAINTS)).
- Stephen Dolan, "Algebraic Subtyping," PhD dissertation, University of
  Cambridge, 2017. And Stephen Dolan & Alan Mycroft, "Polymorphism, Subtyping,
  and Type Inference in MLsub," POPL 2017, pp. 60–72.
  <https://doi.org/10.1145/3009837.3009882>. Combines subtyping with ML
  polymorphism while keeping **principal types** and compact types, via
  **biunification** over **polar types** (strict separation of input/output
  types) and type simplification exploiting connections to regular-language
  algebra.
- Lionel Parreaux, "The Simple Essence of Algebraic Subtyping: Principal Type
  Inference with Subtyping Made Easy (Functional Pearl)," *PACMPL* 4(ICFP),
  Article 124, Aug 2020, 28 pp. <https://doi.org/10.1145/3409006>.
  PDF: <https://infoscience.epfl.ch/record/278576>. Reformulates MLsub as
  **Simple-sub**, "implemented efficiently in under 500 lines of code
  (including parsing, simplification, and pretty-printing)," without
  bisubstitution/abstract algebra — the practical entry point if Basilisk
  adopts algebraic-subtyping ideas. Follow-on work extends it (MLstruct,
  OOPSLA 2022, for a Boolean algebra of structural types; "When Subtyping
  Constraints Liberate," POPL 2024, for first-class polymorphism).

### Inference for dynamic and gradual languages {#TYPEINF-RESEARCH-GRADUAL}

- Jeremy G. Siek & Walid Taha, "Gradual Typing for Functional Languages,"
  Scheme and Functional Programming Workshop, 2006, pp. 81–92. Introduces the
  **consistency relation** (`~`) and the gradually typed lambda calculus
  (GTLC): `Any` (written `?`/`⋆`) is consistent with every type but subtyping
  is not transitive through it.
- Jeremy G. Siek, Michael M. Vitousek, Matteo Cimini & John Tang Boyland,
  "Refined Criteria for Gradual Typing," SNAPL 2015, LIPIcs vol. 32,
  pp. 274–293. <https://doi.org/10.4230/LIPIcs.SNAPL.2015.274>. Defines the
  **gradual guarantee** (static + dynamic). Consistency-vs-subtyping
  distinction: a gradual type system replaces `<:` with consistency `~` (or
  "consistent subtyping"), so `int ~ Any` and `Any ~ str` but `int` is not
  `~` `str`. For an inference engine, the guarantee demands: **removing an
  annotation must never introduce a new static error**, which forbids
  inferring over-precise types from unannotated code (exactly Pyrefly's
  risk).
- Sam Tobin-Hochstadt & Matthias Felleisen, "Logical Types for Untyped
  Languages," ICFP 2010, pp. 117–128.
  <https://doi.org/10.1145/1863543.1863561>.
  PDF: <https://www2.ccs.neu.edu/racket/pubs/icfp10-thf.pdf>. Formalizes
  **occurrence typing**: predicates in test positions (`isinstance`,
  `is None`) produce *propositions* about variables that refine types in each
  control-flow branch. Foundational for Typed Racket and for all Python
  narrowing. Newer line: Giuseppe Castagna et al., "On type-cases, union
  elimination, and occurrence typing," *PACMPL* 2022,
  <https://doi.org/10.1145/3498674>, and "Revisiting occurrence typing,"
  *Sci. Comput. Program.* 2022, recast narrowing via set-theoretic
  union/intersection/negation types — the same intersection-based approach ty
  uses.
- Tobias Lindahl & Konstantinos Sagonas, "Practical Type Inference Based on
  Success Typings," PPDP 2006, pp. 167–178.
  <https://doi.org/10.1145/1140335.1140356>. The **"never wrong" philosophy**:
  success typings over-approximate the set of terms that *can* succeed, so the
  tool (Dialyzer) reports only *definite* errors — no false positives. This is
  the correct model for the *optional/gradual* subset of Basilisk's
  diagnostics: report an error only when *no* instantiation could succeed.
- Python-specific / dynamic-language inference context: Giuseppe Castagna,
  Mickaël Laurent & Kim Nguyễn, "Polymorphic Type Inference for Dynamic
  Languages," *PACMPL* 8(POPL):1179–1210, 2024,
  <https://doi.org/10.1145/3632882>
  (arXiv: <https://arxiv.org/pdf/2311.10426>). ML-based inference (Typilus,
  TypeWriter) is tangential and not recommended as an architecture.

### Type-level computation, conditional/mapped types, decidability {#TYPEINF-RESEARCH-TYPELEVEL}

- **PEP 827 – Type Manipulation** (<https://peps.python.org/pep-0827/>)
  proposes type-level introspection and construction "inspired largely by
  TypeScript's conditional and mapped types," using `typing.IsAssignable`,
  "Type Booleans," conditional type expressions, and a `RaiseError` primitive
  for custom compile-time errors. It is intended to be **fully statically
  checkable** (a mypy proof-of-concept is referenced). The Vercel write-up
  "Advancing Python typing"
  (<https://vercel.com/blog/advancing-python-typing>) frames it as giving
  Python "a programmable core."
- **Turing-completeness / undecidability.** "TypeScript's Type System is
  Turing Complete," microsoft/TypeScript issue #14833
  (<https://github.com/microsoft/TypeScript/issues/14833>). Ori Roth, "Python
  Type Hints are Turing Complete," arXiv:2208.14755, 2022
  (<https://arxiv.org/pdf/2208.14755>; ECOOP 2023, LIPIcs vol. 263,
  pp. 44:1–44:15) — proves it via nominal subtyping with variance (building on
  Grigore, "Java Generics are Turing Complete," POPL 2017). Consequence:
  **type-level evaluation cannot be both complete and guaranteed-terminating**;
  Basilisk must bound it.
- **Termination strategies from Haskell type classes.** GHC User's Guide
  §6.8.8, "Instance declarations and resolution"
  (<https://downloads.haskell.org/~ghc/9.2.8/docs/html/users_guide/exts/instances.html>):
  the **Paterson Conditions** (each constraint must have no type variable
  occurring more often than in the head, must have strictly fewer
  constructors+variables than the head, and must mention no type functions)
  plus the **Coverage Condition** ensure instance resolution terminates;
  `UndecidableInstances` lifts these and can loop. This is the model for
  bounding conditional-type recursion.
- **Higher-kinded types / type operators.** Mapped types are type-level
  functions (kind `Type → Type`), i.e., System Fω operators. Evaluating them
  requires **normalization to weak-head normal form (whnf)**; decidability of
  Fω type equality rests on strong normalization of the type-operator
  calculus, which the general recursive PEP 827 setting does *not* enjoy —
  hence bounded evaluation.
- **Practical fast+terminating strategies:** memoization of normalized types
  (Salsa queries are ideal), depth/fuel limits (TypeScript caps instantiation
  depth and type-instantiation count; ty falls back to a widened type "after a
  certain number of iterations"), and normalization to whnf with structural
  caching. ty's `Divergent` type is exactly this fallback made explicit.

### How existing Python checkers do inference {#TYPEINF-RESEARCH-COMPETITORS}

**Pyrefly (Meta, Rust, MIT).**

- Announcement: "Introducing Pyrefly," Engineering at Meta, 15 May 2025
  (<https://engineering.fb.com/2025/05/15/developer-tools/introducing-pyrefly-a-new-type-checker-and-ide-experience-for-python/>).
  Clean-slate successor to OCaml Pyre; aggressive inference of returns and
  local variables.
- Design detail (authoritative): Neil Mitchell, "Pyrefly: Type Checking 1.8
  Million Lines of Python Per Second," Jane Street Tech Talk
  (<https://www.janestreet.com/tech-talks/pyrefly/>). Six-phase per-file
  pipeline: **Code → AST → Exports → Bindings → Answers → Interface**. The
  **Bindings** phase desugars all control flow into a key→value map (with
  `phi` join operators for control-flow merges and narrowing operators),
  effectively a DSL for type checking. The **Answers** phase solves
  per-binding, special-casing every Python type; recursion and generics are
  handled with **`Var`** unification variables and **thunks** for cycles.
  Incrementality is **file-level** ("if you press a keystroke, we invalidate
  that entire file"); cycles up to depth 5 are re-checked incrementally before
  falling back to whole-cycle invalidation. Constraint solving is "almost
  always this type must be a subtype of this type," with unions making it "a
  bit tricky"; Mitchell notes intersection types would push toward "SAT
  solving" for subset checks. On the `x=[]; x.append(1)` example, Pyrefly
  peeks one use ahead and infers `list[int]`, generalizing `Literal[1]` to
  `int` before it enters an invariant container.
- "Lessons from Pyre that Shaped Pyrefly," 18 Mar 2026
  (<https://pyrefly.org/blog/lessons-from-pyre/>): Pyrefly's engine "embraces
  cycles" with cycle detection + fixpoint resolution (Pyre's acyclic-phase
  rule forced MRO recomputation); Pyrefly "chose usability over absolute
  soundness," notably allowing attribute narrowing (`isinstance(c.x, int)`)
  to survive an intervening function call that Pyre rejected as potentially
  mutating.
- Pyrefly team notes module-level (file) incrementality was chosen because it
  "is already fast enough in Rust" and fine-grained is "much more complex" for
  "minimal performance improvements" (Edward Li, PyCon 2025 Typing Summit
  notes: <https://blog.edward-li.com/tech/comparing-pyrefly-vs-ty/>). Pyrefly
  does **not** use Salsa; it keeps manual control over memory eviction
  (<https://pyrefly.org/blog/speed-and-memory-comparison/>).
- **Inference strengths:** aggressive whole-program-ish inference of
  unannotated returns/locals (catches `None * 2` in untyped code), strong
  generics/overloads/ParamSpec support (they "focused on hard problems
  first"), higher typing-spec conformance than ty. **Weaknesses:** file-level
  invalidation limits IDE incrementality granularity; heuristic ("peek one
  ahead") context propagation rather than principled bidirectional checking;
  usability-over-soundness introduces false negatives; aggressive inference
  produces false positives that violate the gradual guarantee.

**ty / red-knot (Astral, Rust, MIT).**

- Built on **Salsa with fine-grained multi-granularity queries**:
  `infer_scope_types`, `infer_definition_types`, `infer_expression_types`, and
  statement-level inference are each memoized tracked queries (DeepWiki,
  <https://deepwiki.com/astral-sh/ruff/5.2-type-inference-engine>, citing
  `crates/ty_python_semantic/src/types/infer.rs`). Carl Meyer: "if you just
  change one part of one file, Salsa can just flow that change through the
  graph of queries and figure out exactly which queries…need to
  re-execute…very fine-grained incrementality" (Talk Python #506,
  <https://talkpython.fm/episodes/show/506/>). Astral reports 4.7ms recompute
  after editing a load-bearing PyTorch file (astral.sh/blog/ty).
- **Cycles:** fixpoint iteration seeded with a `Divergent` type until
  convergence; falls back to a widened type after a bounded number of
  iterations (ty docs: <https://docs.astral.sh/ty/features/type-system/>).
- **Type representation:** single `Type<'db>` enum; dynamic types are a nested
  `DynamicType` enum — `Any` (explicit), `Unknown` (implicit/inferred gap),
  `Todo` (unimplemented feature), `Divergent` (non-converging recursion)
  (ty Typing FAQ: <https://docs.astral.sh/ty/reference/typing-faq/>).
  **First-class intersection types** with negation drive narrowing
  (`isinstance` → `A & B`; `hasattr` → intersection with a synthetic
  protocol).
- **Reachability analysis is based on type inference** (Kleene/ternary
  logic), not pattern-matching known idioms, letting ty prune unreachable
  branches far more generally (ty docs).
- **Bidirectional inference** was added via `TypeContext` after living for a
  long time as pure "inside-out" inference; ty issue #168
  (<https://github.com/astral-sh/ty/issues/168>) records the motivation and
  Carl Meyer's judgment that type context is needed "to apply contextual
  constraints that will help the generic solver find better solutions."
  **Generics** use an explicit constraint solver (`SpecializationBuilder` +
  `ConstraintSetBuilder`), with PEP 695 variance inference.
- **Gradual guarantee** is a core design invariant ("adding type annotations
  to working code never introduces new errors"); ty infers `Unknown` (not a
  concrete type) for unannotated symbols and does not error on them. This is
  the deliberate opposite of Pyrefly.
- Astral reports ty is "consistently between 10x and 60x faster than mypy and
  Pyright" without caching (astral.sh/blog/ty).
- **Strengths:** best-in-class incrementality, principled gradual guarantee,
  intersection types, inference-driven reachability. **Weaknesses:** lower
  spec conformance than Pyrefly — roughly 15% for ty vs ~58% for Pyrefly on
  the typing-spec conformance suite as of March 2026 (Pyrefly
  typing-conformance comparison, via byteiota); some permissive behaviors
  (e.g., `list.append` after inferred empty-container) are acknowledged
  inference *limitations*, not intended design (per the ty team, via Edward
  Li).

**Pyright (Microsoft, TypeScript).**

- Uses bidirectional inference ("bidirectional type inference is used to
  determine the types of the argument expressions") and code-flow-based
  **type narrowing / type guards**, documented in
  `pyright/docs/type-concepts-advanced.md`
  (<https://github.com/microsoft/pyright/blob/main/docs/type-concepts-advanced.md>):
  narrowing on `isinstance`, `is None`, truthiness, discriminated unions, and
  "narrowing for implied else" (limited to declared-type names for
  performance). User-defined type guards: PEP 647 (`TypeGuard`, Eric Traut,
  <https://peps.python.org/pep-0647/>), PEP 724 (stricter guards, withdrawn),
  PEP 742 (`TypeIs`). Eric Traut's constraint solver for generics uses **type
  intervals** (lower/upper bounds) with join at downward and meet at upward
  inference directions (US patent filings describe the algorithm).
  **Strength:** mature, high-conformance bidirectional inference and the
  richest narrowing. **Weakness:** TypeScript implementation is not the
  fastest; not Salsa-incremental.

**mypy.**

- Uses bidirectional checking with **join/meet** operations on the type
  lattice and constraint solving for generics. Known limitation: join-based
  inference "discards valuable type information and leads to many false
  positives" (basedpyright's mypy comparison,
  <https://docs.basedpyright.com/v1.25.0/usage/mypy-comparison/>); e.g.,
  `isinstance` chains widen to `object`. Constraint inference for unions of
  nested generics is incomplete (python/mypy#9435). Narrowing documented at
  <https://mypy.readthedocs.io/en/latest/type_narrowing.html>. **Strength:**
  reference semantics, mature. **Weakness:** slow, join-induced imprecision,
  weaker bidirectional context propagation.

**Inference-quality scorecard (where each is strong/weak):** generic
inference — Pyright/Pyrefly strong, mypy weak (join loss), ty improving via
constraint solver; literal inference — Pyright/Pyrefly strong; narrowing —
Pyright richest, ty most principled (intersections), Pyrefly good, mypy
join-limited; comprehensions/lambdas — all weak without bidirectional context
(ty's `TypeContext` targets lambdas first); higher-order/`ParamSpec` — Pyrefly
strongest; bidirectional context propagation — Pyright most complete, Pyrefly
heuristic, ty catching up.

**Source-quality note.** Competitor performance numbers (Pyrefly's "1.85M
LOC/s" on 166-core Meta infrastructure; ty's "4.7ms" recompute and "10–60x
faster than mypy/Pyright"; the ~58% vs ~15% conformance gap) are
vendor/benchmark claims, not independently audited, and several figures come
from 2026 vendor blogs and a community handbook (pydevtools); treat
comparative percentages as directional. The DeepWiki ty internals are
AI-generated but cite exact source files in astral-sh/ruff and should be
verified against a pinned commit before being quoted further.

### Incremental / demand-driven architecture {#TYPEINF-RESEARCH-INCREMENTAL}

- **Salsa** (<https://github.com/salsa-rs/salsa>): "define your program as a
  set of queries," inputs + memoized pure functions; recomputes on demand with
  **early cutoff** when a recomputed query's result is unchanged. Used by
  rust-analyzer and chalk. Architecture: rust-analyzer dev docs
  (<https://github.com/rust-lang/rust-analyzer/blob/master/docs/dev/architecture.md>)
  and "Durable Incrementality"
  (<https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html>)
  — a global revision counter, forward flooding to check dependencies,
  backward flooding on change, with a **durability** system so
  standard-library queries aren't rechecked on every keystroke.
- Niko Matsakis, "Responsive Compilers," PLISS 2019 — the motivating talk for
  demand-driven, incremental, query-based compiler architecture (referenced
  widely, e.g., <https://rustc-dev-guide.rust-lang.org/queries/salsa.html>).
- **The central tension:** whole-program/global inference conflicts with
  incrementality. Every inferred type that depends on a distant call site
  becomes a cross-file Salsa dependency, so a change invalidates a large query
  subgraph. Pyrefly resolves this by (a) file-level granularity and (b)
  computing a compact **Interface** (exported types only) that "shields"
  downstream files. ty resolves it by fine-grained queries + Salsa early
  cutoff + durability. Both compute a stable per-module *signature* boundary
  so that intra-file changes don't cascade. Research: incremental type
  checking via Salsa-style memoization is the de facto standard; Roslyn's
  **red-green trees** (immutable "green" nodes + positioned "red" facades) are
  the parallel technique in the C# compiler for incremental, persistent
  syntax.
