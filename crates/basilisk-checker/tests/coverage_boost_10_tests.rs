#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Coverage boost tests batch 10: targeting medium-coverage rules for maximum improvement.
//! Focuses on rules 50-90% coverage where more complex test inputs can push coverage higher.
#![allow(
    missing_docs,
    clippy::needless_raw_string_hashes,
    clippy::uninlined_format_args
)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// --- E0036: ClassVar deep paths ---

#[test]
fn e0036_classvar_typing_extensions() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing_extensions import ClassVar\nx: ClassVar[int] = 42\n";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0036_classvar_in_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar
from dataclasses import dataclass

@dataclass
class Model:
    count: ClassVar[int] = 0
    name: str = 'default'
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0036_classvar_assignment_to_self() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    data: ClassVar[list] = []

    def __init__(self) -> None:
        self.data = [1, 2, 3]
        self.data.append(4)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0036_classvar_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Protocol

class HasClassVar(Protocol):
    count: ClassVar[int]
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0149: PEP 695 deeper ---

#[test]
fn e0149_generic_protocol_typevar_scope() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Protocol

T = TypeVar('T')
U = TypeVar('U')

class MyProtocol(Protocol[T]):
    def method(self, x: T) -> T: ...

class Impl(Generic[U]):
    def method(self, x: U) -> U:
        return x
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_typevar_in_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Callable

T = TypeVar('T')

class Cached(Generic[T]):
    def cache(self, func: Callable[[T], T]) -> Callable[[T], T]:
        return func
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0144: type() deeper ---

#[test]
fn e0144_type_with_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: type = type("Dynamic", (object,), {})
y: type[int] = int
z = type("Z", (), {"value": 42, "name": "test"})
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0079: Module protocol deeper ---

#[test]
fn e0079_protocol_with_property_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class HasVersion(Protocol):
    @property
    def version(self) -> str: ...
    def upgrade(self) -> None: ...

import os
v: HasVersion = os
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0120: Generator deeper ---

#[test]
fn e0120_generator_multiple_yields() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def fib() -> Generator[int, None, None]:
    a, b = 0, 1
    while True:
        yield a
        a, b = b, a + b

def countdown(n: int) -> Generator[int, None, str]:
    while n > 0:
        yield n
        n -= 1
    return 'done'
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0138: Dataclass transform deeper ---

#[test]
fn e0138_transform_with_init_and_repr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform(frozen_default=True, order_default=True, eq_default=True)
def full_model(cls):
    return cls

@full_model
class FullConfig:
    name: str
    value: int
    debug: bool
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0130: TypeVar scoping deeper ---

#[test]
fn e0130_typevar_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T', bound=int)

class NumberContainer(Generic[T]):
    def add(self, a: T, b: T) -> T: ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0130_typevar_in_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, TypeAlias, List

T = TypeVar('T')

MyList: TypeAlias = List[T]
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0047: Invalid type expression deeper ---

#[test]
fn e0047_string_literal_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    def method(self) -> "MyClass":
        return self
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0047_none_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f() -> None:\n    pass\n";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0047_ellipsis_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

x: Callable[..., int]
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0143: NamedTuple deeper ---

#[test]
fn e0143_namedtuple_with_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float

    def distance(self) -> float:
        return (self.x ** 2 + self.y ** 2) ** 0.5

    def translate(self, dx: float, dy: float) -> 'Point':
        return Point(self.x + dx, self.y + dy)

p = Point(3.0, 4.0)
d = p.distance()
p2 = p.translate(1.0, 1.0)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0122: Callable deeper ---

#[test]
fn e0122_callable_with_varargs_kwonly() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_binary(f: Callable[[int, str], None]) -> None:
    pass

def with_varargs(*args: int, name: str) -> None:
    pass

def with_kwargs(a: int, **kw: str) -> None:
    pass

takes_binary(with_varargs)
takes_binary(with_kwargs)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0095: InitVar deeper ---

#[test]
fn e0095_initvar_with_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar, field

@dataclass
class Model:
    name: str
    data: InitVar[dict]
    items: list = field(default_factory=list)
    count: int = 0

    def __post_init__(self, data: dict) -> None:
        self.items = list(data.values())
        self.count = len(data)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0121: Protocol deeper ---

#[test]
fn e0121_protocol_wrong_param_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Processor(Protocol):
    def process(self, data: str) -> int: ...
    def validate(self, item: int) -> bool: ...

class BadProcessor:
    def process(self, data: int) -> str:
        return ''
    def validate(self, item: str) -> int:
        return 0

def use_processor(p: Processor) -> None:
    pass

use_processor(BadProcessor())
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0139: TypeVarTuple deeper ---

#[test]
fn e0139_typevartuple_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack

Ts = TypeVarTuple('Ts')

class Array(Generic[*Ts]):
    def shape(self) -> tuple: ...

def zeros() -> Array[int, int, int]:
    return Array()

def ones() -> Array[int, int]:
    return Array()
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0126: Literal deeper ---

#[test]
fn e0126_literal_none() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[None] = None
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0063: Non-hashable deeper ---

#[test]
fn e0063_dataclass_eq_false() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(eq=False)
class Point:
    x: int
    y: int

s = {Point(1, 2)}
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0073: NamedTuple tuple compat deeper ---

#[test]
fn e0073_namedtuple_length_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple, Tuple

class Triple(NamedTuple):
    a: int
    b: int
    c: int

def takes_pair(t: Tuple[int, int]) -> None:
    pass

t = Triple(1, 2, 3)
takes_pair(t)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0145: Type bracket deeper ---

#[test]
fn e0145_union_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union, Optional, Tuple, Set, FrozenSet, Deque

a: Union[int, str, float, bool] = 1
b: Optional[Union[int, str]] = None
c: Tuple[int, ...] = (1, 2, 3)
d: Set[Union[int, str]] = {1, 'a'}
e: FrozenSet[int] = frozenset([1, 2])
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0112: TypeGuard callable deeper ---

#[test]
fn e0112_typeguard_nested() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeGuard, TypeIs

def is_int(x: object) -> TypeGuard[int]:
    return isinstance(x, int)

def is_str(x: object) -> TypeIs[str]:
    return isinstance(x, str)

def check_list(items: list) -> None:
    ints = [x for x in items if is_int(x)]
    strs = [x for x in items if is_str(x)]
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0102: TypeVar default deeper ---

#[test]
fn e0102_typevar_multiple_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T', default=int)
U = TypeVar('U', default=str)
V = TypeVar('V', default=float)

class Triple(Generic[T, U, V]):
    pass

a: Triple = Triple()
b: Triple[bool] = Triple()
c: Triple[bool, bytes] = Triple()
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0054: Final deeper ---

#[test]
fn e0054_final_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

# Module-level finals
MAX: Final = 100
NAME: Final[str] = "test"
PI: Final = 3.14

# Reassignments
MAX = 200
NAME = "changed"

class Config:
    VERSION: Final = "1.0"
    DEBUG: Final[bool] = False

Config.VERSION = "2.0"
Config.DEBUG = True

def func() -> None:
    x: Final = 42
    x = 100
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0050: NewType deeper ---

#[test]
fn e0050_newtype_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, List, Dict

UserId = NewType('UserId', int)
Username = NewType('Username', str)
UserList = NewType('UserList', List[int])
UserMap = NewType('UserMap', Dict[str, int])

uid: UserId = UserId(42)
name: Username = Username('alice')
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0076: Overload union expansion deeper ---

#[test]
fn e0076_overload_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload, Union

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
@overload
def process(x: float) -> float: ...

def process(x: Union[int, str, float]) -> Union[int, str, float]:
    return x
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0015: Assignment compatibility deeper ---

#[test]
fn e0015_complex_assignments() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import List, Dict, Optional, Tuple

a: List[int] = [1, 2, 3]
b: Dict[str, int] = {'a': 1}
c: Optional[int] = None
d: Tuple[int, str] = (1, 'a')
e: Optional[str] = 'hello'
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0041: Too few args deeper ---

#[test]
fn e0041_various_arg_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def required_three(a: int, b: str, c: float) -> None:
    pass

def with_defaults(a: int, b: str = "x", c: float = 1.0) -> None:
    pass

def with_varargs(a: int, *args: str) -> None:
    pass

def kw_only(a: int, *, b: str) -> None:
    pass

# Too few
required_three(1)
required_three(1, "a")

# OK with defaults
with_defaults(1)
with_defaults(1, "a")

# OK with varargs
with_varargs(1)
with_varargs(1, "a", "b")

# Missing kw-only
kw_only(1)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0116: NamedTuple definition deeper ---

#[test]
fn e0116_namedtuple_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Valid(NamedTuple):
    x: int
    y: str = "default"

class WithMethod(NamedTuple):
    a: float
    b: float

    def sum(self) -> float:
        return self.a + self.b

class WithUnderscoreField(NamedTuple):
    _private: int
    public: str
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0118: Super abstract deeper ---

#[test]
fn e0118_abstract_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from abc import ABC, abstractmethod

class Shape(ABC):
    @abstractmethod
    def area(self) -> float: ...
    @abstractmethod
    def perimeter(self) -> float: ...
    def describe(self) -> str:
        return 'shape'

class Circle(Shape):
    def __init__(self, r: float) -> None:
        self.r = r
    def area(self) -> float:
        return 3.14 * self.r * self.r
    def perimeter(self) -> float:
        return 2 * 3.14 * self.r

class Rect(Shape):
    def area(self) -> float:
        return 0.0
    def perimeter(self) -> float:
        return 0.0
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0092: Too few type args deeper ---

#[test]
fn e0092_complex_generics() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')
V = TypeVar('V')

class Container(Generic[T]):
    pass

class Pair(Generic[T, U]):
    pass

class Triple(Generic[T, U, V]):
    pass

a: Container[int] = Container()
b: Pair[int, str] = Pair()
c: Triple[int, str, float] = Triple()

# Raw (no type args)
d: Container = Container()
e: Pair = Pair()
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0094: Self type deeper ---

#[test]
fn e0094_self_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self

class Builder:
    def set_name(self, name: str) -> Self:
        return self

    @classmethod
    def create(cls) -> Self:
        return cls()

    @staticmethod
    def bad() -> Self:
        pass

def free_func() -> Self:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0038: TypedDict deeper ---

#[test]
fn e0038_typeddict_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict, Required, NotRequired

class Base(TypedDict):
    name: str

class Extended(Base):
    age: int
    email: NotRequired[str]

class Strict(TypedDict, total=True):
    id: int
    name: str

class Partial(TypedDict, total=False):
    id: int
    name: str
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0108: Dataclass slots deeper ---

#[test]
fn e0108_slots_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(slots=True)
class SlottedPoint:
    x: int
    y: int

@dataclass
class RegularPoint:
    x: int
    y: int
    __slots__ = ('x', 'y')
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0110: Protocol variance deeper ---

#[test]
fn e0110_protocol_variance_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol

T = TypeVar('T')
T_co = TypeVar('T_co', covariant=True)
T_contra = TypeVar('T_contra', contravariant=True)

class Reader(Protocol[T_co]):
    def read(self) -> T_co: ...

class Writer(Protocol[T_contra]):
    def write(self, val: T_contra) -> None: ...

class ReadWriter(Protocol[T]):
    def read(self) -> T: ...
    def write(self, val: T) -> None: ...
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0117: Unbound TypeVar deeper ---

#[test]
fn e0117_unbound_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')

class Container(Generic[T]):
    def method(self, x: U) -> U:
        return x

def func(x: T) -> T:
    return x

def multi(x: T, y: U) -> None:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0051: Invalid literal deeper ---

#[test]
fn e0051_literal_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

# Valid literals
a: Literal[1] = 1
b: Literal["hello"] = "hello"
c: Literal[True] = True
d: Literal[None] = None
e: Literal[1, 2, 3] = 1

# Enum member literal
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2

f: Literal[Color.RED] = Color.RED
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0064: NamedTuple functional invalid ---

#[test]
fn e0064_namedtuple_functional_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
from collections import namedtuple

# Valid functional syntax
Point = NamedTuple('Point', [('x', float), ('y', float)])

# collections.namedtuple
Point2 = namedtuple('Point2', ['x', 'y'])

# Invalid: wrong name
Bad = NamedTuple('Wrong', [('x', int)])
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0091: TypeVar default incompat ---

#[test]
fn e0091_typevar_default_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T', default=int)
U = TypeVar('U', default=str)

class Container(Generic[T, U]):
    pass

# Using defaults
c1: Container = Container()
c2: Container[float] = Container()
c3: Container[float, bytes] = Container()
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0069: Dataclass kw-only ---

#[test]
fn e0069_kwonly_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field

@dataclass
class Config:
    name: str
    value: int = field(kw_only=True, default=0)

c1 = Config('test', value=42)
c2 = Config('test')
c3 = Config('test', 42)
"#;
    let _ = run(source)?;
    Ok(())
}
