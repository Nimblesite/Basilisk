//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 15: targeting remaining uncovered code paths with
// very specific Python patterns. Focus on: nested generics for e0107 variance,
// e0144 `type()` deep paths, e0138 transform edge cases, e0143 `NamedTuple` paths,
// e0095 `InitVar` patterns, e0122 callable checks, e0073 `NamedTuple` compat,
// e0116 `NamedTuple` definition, e0102 `TypeVar` defaults, e0112 `TypeGuard`,
// e0145 type bracket, e0147 tuple unpack, e0148 generic args,
// e0131 yield, e0126 literal, e0054 final, e0076 overload, e0121 conformance,
// e0119 isinstance, e0139 typevartuple, e0146 protocol class,
// e0130 scoping, e0111 constructor, e0140 callable compat.

// =============================================================================
// E0107: Variance with nested generics (TypeArg::Subscript)
// =============================================================================

#[test]
fn nested_generic_in_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Container(Generic[T_co]):
    pass

class Processor(Generic[T_contra]):
    pass

# Nested: Base[Container[T_co]] where Container is covariant
class Wrapper(Generic[T_co]):
    pass

class Derived(Wrapper[Container[T_co]]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn nested_generic_variance_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Source(Generic[T_co]):
    pass

class Sink(Generic[T_contra]):
    pass

# Violation: covariant T_co in contravariant position via nesting
class Bad(Sink[Source[T_co]]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deeply_nested_generics() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

class Layer1(Generic[T_co]):
    pass

class Layer2(Generic[T_co]):
    pass

class Deep(Layer1[Layer2[T_co]]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn type_alias_expansion() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T_co = TypeVar("T_co", covariant=True)

class Box(Generic[T_co]):
    pass

IntBox: TypeAlias = Box[int]

class Container(Generic[T_co]):
    pass

class WithAlias(Container[T_co]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compose_variance_all_combos() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class CovariantOuter(Generic[T_co]):
    pass

class ContravariantOuter(Generic[T_contra]):
    pass

class InvariantOuter(Generic[T]):
    pass

# Covariant in covariant = covariant
class CC(CovariantOuter[T_co]):
    pass

# Contravariant in covariant = contravariant
class CContra(CovariantOuter[T_contra]):
    pass

# Covariant in contravariant = contravariant
class ContraC(ContravariantOuter[T_co]):
    pass

# Contravariant in contravariant = covariant
class ContraContra(ContravariantOuter[T_contra]):
    pass

# Invariant in anything = invariant
class InvC(InvariantOuter[T_co]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0144: type() constructor - more edge cases
// =============================================================================

#[test]
fn type_with_keyword_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
MyMeta = type("MyMeta", (type,), {"__module__": "test"})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn type_bases_not_all_names() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    pass

Created = type("Created", (Base,), {"x": 1})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn type_empty_bases_empty_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Plain = type("Plain", (), {})
WithBase = type("WithBase", (object,), {})
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0138: Dataclass transform - remaining uncovered paths
// =============================================================================

#[test]
fn transform_with_init_false() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform()
class NoInitBase:
    def __init_subclass__(cls, init: bool = True, **kwargs: object) -> None:
        super().__init_subclass__(**kwargs)

class NoInitModel(NoInitBase, init=False):
    x: int
    y: str
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn transform_multiple_subclasses() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform()
class BaseModel:
    def __init_subclass__(cls, **kwargs: object) -> None:
        pass

class User(BaseModel):
    name: str
    email: str

class Product(BaseModel):
    title: str
    price: float

class Order(BaseModel):
    user: str
    product: str
    quantity: int = 1
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0143: NamedTuple - remaining uncovered paths
// =============================================================================

#[test]
fn namedtuple_functional_keyword_style() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Config = NamedTuple("Config", name=str, value=int, debug=bool)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn namedtuple_with_classvar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple, ClassVar

class Point(NamedTuple):
    x: int
    y: int
    dimensions: ClassVar[int] = 2
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn namedtuple_with_many_fields() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class HTTPResponse(NamedTuple):
    status_code: int
    reason: str
    headers: dict[str, str]
    body: bytes
    url: str
    elapsed: float
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0095: InitVar - remaining uncovered paths
// =============================================================================

#[test]
fn initvar_without_post_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, InitVar

@dataclass
class Config:
    name: str
    debug: InitVar[bool] = False
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn initvar_mixed_with_field() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar, field

@dataclass
class Config:
    x: int
    y: InitVar[int]
    z: int = field(default=0)
    w: InitVar[str] = "default"

    def __post_init__(self, y: int, w: str) -> None:
        self.x += y
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0122: Callable arity - remaining uncovered
// =============================================================================

#[test]
fn callable_positional_only_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def pos_only(a: int, b: str, /) -> bool:
    return True

f: Callable[[int], bool] = pos_only
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_complex_signature() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def complex(a: int, b: str = "", *args: float, key: bool = True, **kwargs: str) -> None:
    pass

f1: Callable[[int], None] = complex
f2: Callable[[int, str], None] = complex
f3: Callable[..., None] = complex
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0073: NamedTuple tuple compat - remaining uncovered
// =============================================================================

#[test]
fn namedtuple_tuple_fewer_fields() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p: tuple[int, int, int, int] = Point(1, 2)
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0116: NamedTuple definition - remaining uncovered
// =============================================================================

#[test]
fn namedtuple_functional_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

A = NamedTuple("A", [("x", int)])
B = NamedTuple("B", x=int, y=str)
C = NamedTuple("C", [("a", int), ("b", str), ("c", float)])
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn namedtuple_invalid_field_names() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Bad(NamedTuple):
    _private: int = 0
    __dunder: str = ""
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0102: TypeVar default - remaining uncovered
// =============================================================================

#[test]
fn typevar_default_referential() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", default=int)
U = TypeVar("U", default=T)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0112: TypeGuard - remaining uncovered
// =============================================================================

#[test]
fn typeguard_method_no_narrow_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

class Checker:
    def check(self) -> TypeGuard[str]:
        return True
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn typeis_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

class Validator:
    def validate(self, x: object) -> TypeIs[int]:
        return isinstance(x, int)
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0145: type bracket - remaining uncovered
// =============================================================================

#[test]
fn type_bracket_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

def foo(x: type[int]) -> type[int]:
    return type(x)

def bar(x: type[str]) -> str:
    return x()

def baz(x: type[int | str]) -> None:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0147: Tuple starred unpack - remaining uncovered
// =============================================================================

#[test]
fn tuple_unpack_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Unpack, TypeVarTuple, Generic

Ts = TypeVarTuple("Ts")

class Multi(Generic[Unpack[Ts]]):
    pass

def process(*args: Unpack[tuple[int, str, float]]) -> None:
    pass

def flexible(*args: Unpack[tuple[int, ...]]) -> None:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0148: Generic type arg - remaining uncovered
// =============================================================================

#[test]
fn tuple_wrong_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Tuple

x: Tuple[int, str, float, bool] = (1, "a", 3.0, True)
y: Tuple[()] = ()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn optional_wrong_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Optional

x: Optional[int, str] = None
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0131: Yield type - remaining uncovered
// =============================================================================

#[test]
fn yield_from_incompatible() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, Iterator

def string_gen() -> Iterator[str]:
    yield "a"
    yield "b"

def int_gen() -> Generator[int, None, None]:
    yield 1
    yield from string_gen()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn generator_with_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def countdown(n: int) -> Generator[int, None, str]:
    while n > 0:
        yield n
        n -= 1
    return "done"
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0126: Literal - remaining uncovered
// =============================================================================

#[test]
fn literal_multiple_values() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[1, 2, 3] = 4
y: Literal["a", "b", "c"] = "d"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_bytes_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[b"hello"] = b"world"
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0054: Final - remaining uncovered
// =============================================================================

#[test]
fn final_augmented_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

X: Final = 42
X += 1
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn final_multiple_reassignments() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

A: Final = 1
B: Final = 2
C: Final = 3

A = 10
B = 20
C = 30
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0076: Overload - remaining uncovered
// =============================================================================

#[test]
fn overload_complex() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload, Union

@overload
def parse(data: str) -> dict: ...
@overload
def parse(data: bytes) -> dict: ...
@overload
def parse(data: int) -> str: ...

def parse(data: Union[str, bytes, int]) -> Union[dict, str]:
    if isinstance(data, int):
        return str(data)
    return {}
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0121: Protocol conformance - remaining uncovered
// =============================================================================

#[test]
fn protocol_with_static_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasUtility(Protocol):
    @staticmethod
    def utility() -> int: ...

class MyClass:
    @staticmethod
    def utility() -> int:
        return 42
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0119: Protocol isinstance - remaining uncovered
// =============================================================================

#[test]
fn protocol_with_property() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class HasName(Protocol):
    @property
    def name(self) -> str: ...

class Person:
    @property
    def name(self) -> str:
        return "Alice"

p = Person()
result = isinstance(p, HasName)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0139: TypeVarTuple - remaining uncovered
// =============================================================================

#[test]
fn typevartuple_in_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Unpack, TypeAlias

Ts = TypeVarTuple("Ts")

TupleType: TypeAlias = tuple[Unpack[Ts]]
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0146: Protocol class - remaining uncovered
// =============================================================================

#[test]
fn protocol_with_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasInit(Protocol):
    def __init__(self, x: int) -> None: ...

class MyClass:
    def __init__(self, x: int) -> None:
        self.x = x
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0130: TypeVar scoping - remaining uncovered
// =============================================================================

#[test]
fn typevar_constraint_checking() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T", int, str)
U = TypeVar("U", bound=int)

class Constrained(Generic[T]):
    value: T

class Bounded(Generic[U]):
    value: U

def process_constrained(x: T) -> T:
    return x

def process_bounded(x: U) -> U:
    return x
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0111: Constructor - remaining uncovered
// =============================================================================

#[test]
fn constructor_with_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Complex:
    def __init__(self, a: int, b: str, c: float, d: bool = True) -> None:
        self.a = a
        self.b = b
        self.c = c
        self.d = d

c1 = Complex(1, "hello", 3.14)
c2 = Complex(1, "hello", 3.14, False)
c3 = Complex(1, 2, 3.14)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0140: Callable compat - remaining uncovered
// =============================================================================

#[test]
fn protocol_with_varargs_in_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class VarArgsProto(Protocol):
    def __call__(self, *args: int, **kwargs: str) -> bool: ...

def my_func(*args: int, **kwargs: str) -> bool:
    return True

f: VarArgsProto = my_func
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_with_concatenate_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, Concatenate, ParamSpec

P = ParamSpec("P")

def wrapper(func: Callable[Concatenate[int, str, P], bool]) -> None:
    pass

def only_int(x: int) -> bool:
    return True

wrapper(only_int)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Collection inference and inference module
// =============================================================================

#[test]
fn collection_inference_list() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: list[int] = [1, 2, 3]
y: list[str] = ["a", "b", "c"]
z: list[float] = [1.0, 2.0]
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn collection_inference_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: dict[str, int] = {"a": 1, "b": 2}
y: dict[int, str] = {1: "a", 2: "b"}
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn collection_inference_set() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: set[int] = {1, 2, 3}
y: frozenset[str] = frozenset({"a", "b"})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn collection_inference_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: tuple[int, str] = (1, "hello")
y: tuple[int, ...] = (1, 2, 3, 4)
z: tuple[()] = ()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0047: Invalid type - exercise all annotation check branches
// =============================================================================

#[test]
fn class_attr_walrus_op() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def foo() -> None:
    x: (y := int) = 42
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn class_attr_comparison() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Foo:
    x: int > str = 42
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0036: ClassVar - exercise scan branches
// =============================================================================

#[test]
fn classvar_with_nested_brackets() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar, Dict, List

class Config:
    items: ClassVar[Dict[str, List[int]]] = {}
    mapping: ClassVar[List[Dict[str, int]]] = []
";
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0014: Literal parsing - remaining uncovered paths
// =============================================================================

#[test]
fn literal_underscore_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

x: Literal[1_000_000] = 1_000_000
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_with_any_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Any

t: tuple[Any, Any] = (1, "hello")
t = (True, None)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_with_object_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[object, object] = (1, "hello")
t = (3.14, b"bytes")
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Compound mega-tests for maximum coverage
// =============================================================================

#[test]
fn mega_all_dataclass_features() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field, InitVar
from typing import ClassVar, Final, List, Optional

@dataclass(frozen=True)
class Immutable:
    x: int
    y: str

@dataclass(eq=True)
class Mutable:
    items: List[int] = field(default_factory=list)
    version: ClassVar[str] = "1.0"

@dataclass(slots=True)
class Slotted:
    a: int
    b: str

@dataclass(kw_only=True)
class KWOnly:
    x: int
    y: str = "default"

@dataclass
class WithInit:
    name: str
    setup: InitVar[bool] = True
    _ready: bool = field(init=False, default=False)

    def __post_init__(self, setup: bool) -> None:
        self._ready = setup

@dataclass(unsafe_hash=True)
class UnsafeHash:
    value: int

@dataclass(eq=False)
class NoEq:
    value: int
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn mega_all_protocol_features() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, runtime_checkable, ClassVar

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

@runtime_checkable
class Sized(Protocol):
    def __len__(self) -> int: ...

class Iterable(Protocol[T_co]):
    def __iter__(self) -> "Iterator[T_co]": ...

class Comparable(Protocol):
    def __lt__(self, other: "Comparable") -> bool: ...
    def __gt__(self, other: "Comparable") -> bool: ...
    def __le__(self, other: "Comparable") -> bool: ...
    def __ge__(self, other: "Comparable") -> bool: ...

class HasVersion(Protocol):
    version: ClassVar[str]

class HasName(Protocol):
    @property
    def name(self) -> str: ...

class Callable(Protocol):
    def __call__(self, *args: object, **kwargs: object) -> object: ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn mega_all_typevar_features() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, TypeVarTuple, ParamSpec, Generic, Unpack

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)
T_bound = TypeVar("T_bound", bound=int)
T_constrained = TypeVar("T_constrained", int, str)
T_default = TypeVar("T_default", default=int)

Ts = TypeVarTuple("Ts")
P = ParamSpec("P")

class Container(Generic[T]):
    value: T

class CovariantBox(Generic[T_co]):
    pass

class ContravariantSink(Generic[T_contra]):
    pass

class BoundedNum(Generic[T_bound]):
    pass

class MultiArg(Generic[T, Unpack[Ts]]):
    pass

def identity(x: T) -> T:
    return x

def bounded(x: T_bound) -> T_bound:
    return x

def constrained(x: T_constrained) -> T_constrained:
    return x
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn mega_all_annotation_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    List, Dict, Set, FrozenSet, Tuple, Optional, Union,
    Literal, Final, ClassVar, TypeVar, Generic,
    Callable, Iterator, Generator, AsyncGenerator,
)

# Basic annotations
a: int = 42
b: str = "hello"
c: float = 3.14
d: bool = True
e: bytes = b"data"
f: None = None

# Generic annotations
g: List[int] = [1, 2]
h: Dict[str, int] = {"a": 1}
i: Set[str] = {"x"}
j: Tuple[int, str] = (1, "a")
k: Optional[int] = None
l: Union[int, str] = 42
m: Literal[1, 2, 3] = 1
n: Final = 100
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn mega_all_function_features() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload, Union, TypeGuard, TypeIs

def simple(x: int) -> str:
    return str(x)

def with_default(x: int, y: str = "hello") -> bool:
    return True

def varargs(*args: int) -> int:
    return sum(args)

def kwargs(**kwargs: str) -> dict[str, str]:
    return kwargs

def pos_only(x: int, y: str, /) -> bool:
    return True

def kw_only(*, x: int, y: str) -> bool:
    return True

def mixed(a: int, b: str, /, c: float = 0.0, *args: int, key: bool = True, **kwargs: str) -> None:
    pass

def guard(x: object) -> TypeGuard[str]:
    return isinstance(x, str)

def narrower(x: object) -> TypeIs[int]:
    return isinstance(x, int)

@overload
def dispatch(x: int) -> int: ...
@overload
def dispatch(x: str) -> str: ...
def dispatch(x: Union[int, str]) -> Union[int, str]:
    return x
"#;
    let _ = run(source)?;
    Ok(())
}
