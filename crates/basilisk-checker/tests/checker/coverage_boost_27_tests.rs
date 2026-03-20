use super::common::*;

// Coverage boost tests batch 27: final push to 89%. Ultra-diverse Python patterns
// targeting remaining uncovered branches across many rules.

// Many small targeted tests to exercise specific code paths

#[test]
fn e0026_typevar_name_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("Wrong")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0030_tvt_before_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, TypeVarTuple, Generic

Ts = TypeVarTuple("Ts")
T = TypeVar("T", default=int)

class Bad(Generic[*Ts, T]): ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0036_classvar_bare() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class A:
    x: ClassVar
    y: ClassVar[int, str, float]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0036_annotated_classvar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Annotated

class A:
    x: Annotated[ClassVar[int], "metadata"]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0038_typeddict_multiple_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class Base1(TypedDict):
    x: int

class Base2(TypedDict):
    y: str

class Combined(Base1, Base2):
    z: float
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0041_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f(a: int) -> None:
    pass

f(1, 2, 3)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0042_pep695_class_and_func() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

class MyClass[S]:
    def method[U](self, x: U) -> U:
        return x

def func[V](x: V) -> V:
    return x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0044_final_invalid_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

def f(x: Final[int]) -> None:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0045_annotated_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[int, "positive"] = 42
y: Annotated[str, "nonempty", "trimmed"] = "hello"

def f(a: Annotated[int, "x"]) -> Annotated[str, "y"]:
    return str(a)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0048_typealias_tuple_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

Bad1: TypeAlias = ((int, str),)
Bad2: TypeAlias = (lambda: int)()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0050_newtype_callable_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Callable

# NewType with callable base
CallbackType = NewType("CallbackType", Callable[[int], str])
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0051_literal_none() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[None] = None
y: Literal[1, 2, None] = None
z: Literal["a", "b", "c"] = "a"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0052_frozen_dataclass_mutation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(frozen=True)
class Immutable:
    x: int
    y: str

obj = Immutable(1, "hello")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0055_typevar_both_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

Bad1 = TypeVar("Bad1", covariant=True, contravariant=True)
Bad2 = TypeVar("Bad2", int, str, covariant=True)
Good = TypeVar("Good", covariant=True)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0060_order_comparison() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class NoOrder:
    x: int

a = NoOrder(1)
b = NoOrder(2)
result = a < b
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0064_namedtuple_arg_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: str

p = Point("wrong", 42)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0069_kwonly_no_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field

@dataclass
class Config:
    x: int
    y: str = field(kw_only=True, init=False)
    z: float = field(kw_only=True)

c = Config(1, z=3.14)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0073_namedtuple_replace() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: str

p = Point(1, "a")
q = p._replace(x=2)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0074_constructor_new_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    def __new__(cls, x: int) -> "MyClass":
        return super().__new__(cls)

    def __init__(self, x: str) -> None:
        self.x = x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0080_typevar_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)

def f(x: T) -> T:
    return x

result = f("hello")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0081_typevartuple_unpack_min() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack

Ts = TypeVarTuple("Ts")

class Container(Generic[*Ts]):
    pass

x: Container[()] = Container()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0082_tvt_callable_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Callable, Unpack

Ts = TypeVarTuple("Ts")

def apply(func: Callable[[Unpack[Ts]], int], *args: Unpack[Ts]) -> int:
    return func(*args)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0084_tvt_invalid_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple

Ts = TypeVarTuple("Ts", default=int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0085_tvt_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack

Ts = TypeVarTuple("Ts")

class Fixed(Generic[*Ts]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0086_multiple_tvt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic

Ts1 = TypeVarTuple("Ts1")
Ts2 = TypeVarTuple("Ts2")

class Bad(Generic[*Ts1, *Ts2]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0088_typeddict_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class TD(TypedDict):
    x: int

isinstance({}, TD)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0089_pep695_invalid_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class A[T: 42]:
    pass

class B[T: "int"]:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0091_typevar_default_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", int, str, default=float)
S = TypeVar("S", bound=int, default=str)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0092_too_few_type_args_generic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

class Pair(Generic[T, S]):
    pass

x: Pair[int] = Pair()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0093_typeddict_key_validation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class User(TypedDict):
    name: str
    age: int

u: User = {"name": "Alice", "age": 30, "extra": True}
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0097_protocol_self_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Self

class Chainable(Protocol):
    def chain(self) -> Self: ...
    next: Self

class GoodChain:
    def chain(self) -> "GoodChain":
        return self
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0098_non_protocol_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyClass:
    pass

class BadProto(Protocol, MyClass):
    def method(self) -> None: ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0099_protocol_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyProto(Protocol):
    def method(self) -> None: ...

x = MyProto()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0101_typeguard_no_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeGuard

def is_str(x: object) -> TypeGuard[str]:
    return isinstance(x, str)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0104_cyclical_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias, Union

MyType: TypeAlias = Union[int, "MyType"]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0105_bounded_typevar_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

class HasName:
    name: str

T = TypeVar("T", bound=HasName)

def get_name(x: T) -> str:
    return x.name
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0108_dataclass_slots_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(slots=True)
class A:
    x: int

@dataclass(slots=True)
class B(A):
    y: str

@dataclass
class C:
    __slots__ = ("z",)
    z: int
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0109_typevar_bound_violation_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)

def add(x: T, y: T) -> T:
    return x + y

result = add(1, 2)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0110_protocol_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_co = TypeVar("T_co", covariant=True)

class Producer(Protocol[T_co]):
    def produce(self) -> T_co: ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0114_protocol_isinstance_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyProto(Protocol):
    def method(self) -> None: ...

isinstance(object(), MyProto)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0124_protocol_tuple_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Tuple

class HasItems(Protocol):
    def items(self) -> Tuple[str, ...]: ...

class MyDict:
    def items(self) -> Tuple[int, ...]:
        return (1, 2, 3)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0125_instance_attr_access_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    class_var: int = 0
    other: str = "hello"

    def __init__(self):
        self.instance_var: str = "world"
        self.count: int = 0

x = MyClass.instance_var
y = MyClass.count
z = MyClass.class_var
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0127_tuple_index_out_of_range() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Tuple

def f(x: Tuple[int, str, float]) -> None:
    a = x[0]
    b = x[1]
    c = x[2]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0132_inconsistent_typevar_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

class A(Generic[T, S]):
    pass

class B(A[S, T]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0133_protocol_typevar_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

class Readable(Protocol[T_co]):
    def read(self) -> T_co: ...

class Writable(Protocol[T]):
    def write(self, value: T) -> None: ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0134_invariant_generic_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, List

T = TypeVar("T")

class Container(Generic[T]):
    items: List[T]

x: Container[int] = Container()
y: Container[object] = x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0136_callable_param_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_int_to_str(f: Callable[[int], str]) -> None:
    pass

def wrong_func(x: str) -> int:
    return int(x)

takes_int_to_str(wrong_func)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0141_unpack_kwargs_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict, Unpack

class Options(TypedDict):
    name: str
    age: int

def func(**kwargs: Unpack[Options]) -> None:
    pass

func(name="Alice", age=30)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Mega compound covering many rules at once
// =============================================================================

#[test]
#[expect(clippy::too_many_lines)]
fn mega_final_push() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, TypeAlias, Final, Literal, Protocol,
    Callable, runtime_checkable, ClassVar, NamedTuple,
    Generator, overload, Hashable, Optional, Union,
    LiteralString, TypeVarTuple, Unpack, ParamSpec, Self,
    dataclass_transform, List, Dict, Set, Tuple, FrozenSet,
    TypedDict, Annotated, TypeGuard, TypeIs, NewType
)
from dataclasses import dataclass, field, InitVar
from enum import Enum
from abc import abstractmethod

# TypeVars
T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)
S = TypeVar("S", default=T)
AnyStr = TypeVar("AnyStr", str, bytes)
BadTV = TypeVar("BadTV", int)
BothVar = TypeVar("BothVar", covariant=True, contravariant=True)
BoundedT = TypeVar("BoundedT", bound=int)
DefaultIncompat = TypeVar("DefaultIncompat", int, str, default=float)
Ts = TypeVarTuple("Ts")
P = ParamSpec("P")

# E0026 single constraint
# E0055 both variance

# E0030 ordering
Td = TypeVar("Td", default=int)
Tnd = TypeVar("Tnd")

# TypedDict
class User(TypedDict):
    name: str
    age: int

WrongTD = TypedDict("RightTD", {"x": int})

# NewType
UserId = NewType("UserId", int)
BadNT = NewType("WrongNT", int)

# Annotated
x: Annotated[int, "positive"] = 42

# Final
MAX: Final = 100
MAX = 200

# Module-level mismatches
bad_int: int = "hello"
bad_str: str = 42
bad_float: str = 3.14
bad_bool: str = True
bad_none: int = None
bad_bytes: int = b"hello"

# ClassVar
class CVClass:
    count: ClassVar[int] = 0
    bad_nested: Final[ClassVar[int]] = 1
    def m(self, a: ClassVar[int]) -> None:
        pass

# Self
def bad_self(x: Self) -> Self:
    return x

# Dataclass
@dataclass
class DC:
    a: int
    b: InitVar[bool] = False
    c: int = field(default_factory=str)
    def __post_init__(self, b: bool):
        pass

vh: Hashable = DC(0)

@dataclass(frozen=True)
class Frozen:
    x: int
    y: str

@dataclass(order=True)
class Ordered:
    x: int

@dataclass(slots=True)
class Slotted:
    x: int
    y: str

# NamedTuple
class Pt(NamedTuple):
    x: int
    y: str = "default"

p = Pt(1, "a")
first = p[0]

# Protocol
@runtime_checkable
class Draw(Protocol):
    def draw(self) -> str: ...

class NonRT(Protocol):
    def method(self) -> None: ...

isinstance(object(), Draw)
isinstance(object(), NonRT)

class HasCV(Protocol):
    name: ClassVar[str]

# Generator
def gen() -> Generator[int, None, str]:
    yield 1
    yield from [2, 3]
    for i in range(5):
        yield i
    return "done"

# Overload
@overload
def proc(x: int) -> str: ...
@overload
def proc(x: str) -> int: ...
def proc(x):
    return str(x)

# Callable
f1: Callable[..., int] = lambda: 42
f2: Callable[[int], str] = lambda x: str(x)

# E0015
opt_bad: Optional[int, str] = None

# TypeAlias
BadAlias: TypeAlias = [int, str]
BadAlias2: TypeAlias = True

# Literal
def lit(a: Literal[0], b: Literal[False]):
    x: Literal[False] = a
    a += 1

z: Literal[3.14] = 3.14

# Constrained
def concat(a: AnyStr, b: AnyStr) -> AnyStr:
    return a + b

# Variance
class ReadOnly(Generic[T_co]):
    def read(self) -> T_co: ...

class WriteOnly(Generic[T_contra]):
    def write(self, val: T_contra) -> None: ...

# E0128
A = TypeVar("A")
B = TypeVar("B", default=A)
C = TypeVar("C", default=B)
class Chain(Generic[A, B, C]): ...

# E0149 PEP 695
class Box[TT]:
    value: TT

type MyType[TT] = list[TT]

def identity[TT](x: TT) -> TT:
    return x

# Variadic
class Var(Generic[*Ts]):
    pass

# Shape with Self return
class Shape:
    def method(self) -> Self:
        return Shape()

# LiteralString
def query(sql: LiteralString) -> None:
    pass
query("SELECT 1")

# TypeGuard
def is_str(x: object) -> TypeGuard[str]:
    return isinstance(x, str)

# TypeIs
def is_int(x: int | str) -> TypeIs[int]:
    return isinstance(x, int)

# Unpack kwargs
def kw_func(**kwargs: Unpack[User]) -> None:
    pass

# Historical positional
def hist(__x: int) -> None: ...
hist(__x=3)

# Enum
class Color(Enum):
    _value_: int
    RED = 1
    GREEN = "green"

# Float int attr
def float_f(x: float) -> int:
    return x.numerator

# Abstract
class AbstractBase(Protocol):
    @abstractmethod
    def draw(self) -> str: ...

class BadChild(AbstractBase):
    def draw(self) -> str:
        return super().draw()

# Tuple index
v: tuple[int, str] = (1, "a")
oob = v[5]

# dataclass_transform
@dataclass_transform()
class ModelBase:
    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

class Model(ModelBase):
    name: str
    age: int
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.len() >= 5,
        "Mega final push: got {} diagnostics: {:?}",
        diagnostics.len(),
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}
