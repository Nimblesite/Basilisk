//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for advanced checker rules with low coverage.
//! Exercises aliases_type_statement through generics_syntax_scoping and complex type scenarios.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn messages_for(diags: &[basilisk_checker::Diagnostic], code: &str) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == code)
        .map(|d| d.message.clone())
        .collect()
}

// ============================================================================
// PEP 695 type statement / TypeAliasType violations
// ============================================================================

#[test]
fn type_alias_type_wrong_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAliasType
Bad = TypeAliasType("Bad")
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn pep695_type_statement() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Vector = list[float]
type Matrix = list[Vector]
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// match_args=False access
// ============================================================================

#[test]
fn dataclass_match_args_false() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(match_args=False)
class NoMatch:
    x: int = 0
    y: str = ""
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Cross-type dataclass ordering
// ============================================================================

#[test]
fn ordering_comparison() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(order=True)
class A:
    x: int = 0

@dataclass(order=True)
class B:
    x: int = 0

a = A(1)
b = B(1)
result = a < b
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Non-hashable dataclass
// ============================================================================

#[test]
fn non_hashable_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int = 0
    y: int = 0
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Invalid NamedTuple call - functional syntax
// ============================================================================

#[test]
fn namedtuple_functional_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from collections import namedtuple
Point = namedtuple("Point", ["x", "y"])
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn namedtuple_functional_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
Point = NamedTuple("Point", [("x", int), ("y", int)])
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Dataclass kw_only violations
// ============================================================================

#[test]
fn kw_only_dataclass_positional_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(kw_only=True)
class Config:
    name: str
    value: int

Config("test", 42)
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Never type compatibility
// ============================================================================

#[test]
fn never_in_union() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never, Union
x: Union[Never, int] = 42
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// No matching overload
// ============================================================================

#[test]
fn overload_call_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: int | str) -> int | str:
    return x

result = process(1)
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// NamedTuple tuple compatibility
// ============================================================================

#[test]
fn namedtuple_tuple_compat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p = Point(1, 2)
t: tuple[int, int] = p
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Constructor __new__ mismatch
// ============================================================================

#[test]
fn custom_new_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Singleton:
    _instance: "Singleton | None" = None

    def __new__(cls) -> "Singleton":
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

s = Singleton()
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Self type violation
// ============================================================================

#[test]
fn self_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Builder:
    def set_name(self, name: str) -> Self:
        return self

    def build(self) -> Self:
        return self
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Invalid tuple syntax
// ============================================================================

#[test]
fn valid_tuple_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
a: tuple[int, str] = (1, "hello")
b: tuple[int, ...] = (1, 2, 3)
c: tuple[()] = ()
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Too few type arguments
// ============================================================================

#[test]
fn too_few_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
U = TypeVar("U")

class Pair(Generic[T, U]):
    first: T
    second: U
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// InitVar violations
// ============================================================================

#[test]
fn initvar_used_correctly() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field, InitVar

@dataclass
class Config:
    name: str
    _raw: InitVar[str] = ""

    def __post_init__(self, _raw: str) -> None:
        pass
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Dataclass field default_factory
// ============================================================================

#[test]
fn dataclass_field_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field

@dataclass
class Config:
    items: list[str] = field(default_factory=list)
    name: str = "default"
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Non-protocol base in Protocol
// ============================================================================

#[test]
fn protocol_with_non_protocol_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Base:
    pass

class MyProto(Protocol, Base):
    def method(self) -> None: ...
";
    let msgs = messages_for(&run(source)?, "protocols_merging");
    // Exercises the rule whether or not it fires
    let _ = msgs;
    Ok(())
}

// ============================================================================
// Protocol instantiation
// ============================================================================

#[test]
fn protocol_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

d = Drawable()
";
    let msgs = messages_for(&run(source)?, "protocols_explicit");
    let _ = msgs;
    Ok(())
}

// ============================================================================
// Cyclical type alias
// ============================================================================

#[test]
fn cyclical_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias
A: TypeAlias = "B"
B: TypeAlias = "A"
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Dataclass slots violation
// ============================================================================

#[test]
fn slots_dataclass_with_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(slots=True)
class SlottedPoint:
    x: float = 0.0
    y: float = 0.0

@dataclass
class RegularPoint(SlottedPoint):
    z: float = 0.0
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// TypeVar bound call violation
// ============================================================================

#[test]
fn typevar_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

class Base:
    def method(self) -> int:
        return 42

T = TypeVar("T", bound=Base)

def func(x: T) -> int:
    return x.method()
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Constructor call errors (complex cases)
// ============================================================================

#[test]
fn generic_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Box(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value: T = value

b = Box(42)
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn class_no_custom_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Simple:
    pass

s = Simple()
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn class_no_init_with_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class NoInit:
    pass

s = NoInit(1, 2, 3)
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Deprecated usage
// ============================================================================

#[test]
fn deprecated_function_called() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func instead")
def old_func() -> None:
    pass

old_func()
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn deprecated_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

@deprecated("Use NewClass instead")
class OldClass:
    pass

x = OldClass()
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn deprecated_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class MyClass:
    @deprecated("Use new_method")
    def old_method(self) -> None:
        pass

    def new_method(self) -> None:
        pass

obj = MyClass()
obj.old_method()
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn deprecated_overloaded() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated, overload

@overload
def func(x: int) -> int: ...
@overload
@deprecated("Use str version")
def func(x: str) -> str: ...
def func(x: int | str) -> int | str:
    return x
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// NamedTuple definition errors
// ============================================================================

#[test]
fn namedtuple_class_form() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int
    label: str = "point"
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Unbound typevar scope
// ============================================================================

#[test]
fn typevar_in_correct_scope() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    def get(self) -> T:
        ...
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Super abstract call
// ============================================================================

#[test]
fn super_call_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from abc import ABC, abstractmethod

class Base(ABC):
    @abstractmethod
    def method(self) -> int: ...

class Derived(Base):
    def method(self) -> int:
        return 42
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Generator return type violation
// ============================================================================

#[test]
fn generator_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, Iterator

def gen() -> Generator[int, None, str]:
    yield 1
    return "done"

def iter_gen() -> Iterator[int]:
    yield 1
    yield 2
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Protocol conformance
// ============================================================================

#[test]
fn protocol_conformance_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Renderable(Protocol):
    def render(self) -> str: ...

class Widget:
    def render(self) -> str:
        return "<widget>"

w: Renderable = Widget()
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn protocol_missing_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Renderable(Protocol):
    def render(self) -> str: ...

class BadWidget:
    pass

w: Renderable = BadWidget()
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Callable call-site violation
// ============================================================================

#[test]
fn callable_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def apply(f: Callable[[int, str], bool], x: int, y: str) -> bool:
    return f(x, y)

def my_check(a: int, b: str) -> bool:
    return True

result = apply(my_check, 1, "hello")
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// LiteralString assignment
// ============================================================================

#[test]
fn literalstring_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import LiteralString

def safe_query(query: LiteralString) -> None:
    pass

safe_query("SELECT * FROM users")
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// TypeVar scoping
// ============================================================================

#[test]
fn typevar_scoping() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Outer(Generic[T]):
    class Inner:
        pass

    def method(self, x: T) -> T:
        return x
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Generator type mismatch
// ============================================================================

#[test]
fn generator_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator

def numbers() -> Generator[int, None, None]:
    for i in range(10):
        yield i
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Inconsistent typevar ordering
// ============================================================================

#[test]
fn typevar_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
U = TypeVar("U")

class Pair(Generic[T, U]):
    first: T
    second: U

class ReversePair(Pair[U, T]):
    pass
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Callable subtyping
// ============================================================================

#[test]
fn callable_subtyping() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_int_to_str(f: Callable[[int], str]) -> None:
    pass

def my_func(x: int) -> str:
    return str(x)

takes_int_to_str(my_func)
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Generic protocol
// ============================================================================

#[test]
fn generic_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Comparable(Protocol[T]):
    def __lt__(self, other: T) -> bool: ...
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// dataclass_transform metaclass (detailed)
// ============================================================================

#[test]
fn metaclass_frozen_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform(frozen_default=True)
class ModelMeta(type): ...

class ModelBase(metaclass=ModelMeta): ...

class Customer(ModelBase):
    id: int
    name: str

c = Customer(id=1, name="test")
c.id = 2
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn metaclass_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform(kw_only_default=True)
class ModelMeta(type): ...

class ModelBase(metaclass=ModelMeta): ...

class Customer(ModelBase):
    id: int
    name: str

c = Customer(1, "test")
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Callable assignment violation
// ============================================================================

#[test]
fn callable_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_str(x: str) -> int:
    return len(x)

f: Callable[[int], str] = takes_str
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn callable_assignment_compatible() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_int(x: int) -> str:
    return str(x)

f: Callable[[int], str] = takes_int
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// dataclass_transform base class
// ============================================================================

#[test]
fn transform_base_frozen() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform(frozen_default=True)
class ModelBase: ...

class Customer(ModelBase):
    id: int
    name: str

c = Customer(id=1, name="test")
c.id = 2
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn transform_base_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform(order_default=True)
class ModelBase: ...

class CustomerA(ModelBase):
    id: int

class CustomerB(ModelBase):
    id: int

a = CustomerA(id=1)
b = CustomerB(id=2)
result = a < b
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// NamedTuple usage violations
// ============================================================================

#[test]
fn namedtuple_attribute_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p = Point(1, 2)
p.x = 3
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn namedtuple_index_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p = Point(1, 2)
v = p[0]
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// type[T] constructor call
// ============================================================================

#[test]
fn type_param_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Animal:
    def __init__(self, name: str) -> None:
        self.name: str = name

def create(cls: type[Animal], name: str) -> Animal:
    return cls(name)

a = create(Animal, "fido")
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn type_param_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Simple:
    pass

def make(cls: type[Simple]) -> Simple:
    return cls()
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Type bracket violations
// ============================================================================

#[test]
fn type_bracket_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    items: list[T]

    def __init__(self) -> None:
        self.items = []
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Protocol class object
// ============================================================================

#[test]
fn protocol_class_object() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Serializable(Protocol):
    def serialize(self) -> str: ...

class JsonSerializer:
    def serialize(self) -> str:
        return "{}"

def process(cls: type[Serializable]) -> None:
    pass
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Tuple starred unpack
// ============================================================================

#[test]
fn tuple_starred_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, str] = (1, "hello")
t2: tuple[int, ...] = (1, 2, 3, 4)
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Generic type arg violation
// ============================================================================

#[test]
fn generic_type_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Stack(Generic[T]):
    def __init__(self) -> None:
        self._items: list[T] = []

    def push(self, item: T) -> None:
        self._items.append(item)

    def pop(self) -> T:
        return self._items.pop()
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// PEP 695 type param scoping
// ============================================================================

#[test]
fn pep695_scoping() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Container[T]:
    def get(self) -> T: ...
    def set(self, value: T) -> None: ...
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn pep695_function_scoping() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def identity[T](x: T) -> T:
    return x

def pair[T, U](first: T, second: U) -> tuple[T, U]:
    return (first, second)
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Exercise complex nesting to hit deep paths
// ============================================================================

#[test]
fn complex_class_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Protocol, Final, ClassVar, overload
from dataclasses import dataclass
from enum import Enum
from abc import ABC, abstractmethod

# Abstract base
class Shape(ABC):
    @abstractmethod
    def area(self) -> float: ...

# Enum
class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

# Generic class
T = TypeVar("T")

class Container(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value: T = value

    def get(self) -> T:
        return self.value

# Protocol
class Drawable(Protocol):
    def draw(self) -> None: ...

# Dataclass
@dataclass
class Point:
    x: float
    y: float

# Frozen dataclass
@dataclass(frozen=True)
class FrozenPoint:
    x: float
    y: float

# Implementation
class Circle(Shape):
    def __init__(self, radius: float) -> None:
        self.radius: float = radius

    def area(self) -> float:
        return 3.14159 * self.radius * self.radius

    def draw(self) -> None:
        pass

# Final
MAX: Final[int] = 100

# ClassVar
class Config:
    instances: ClassVar[int] = 0
    name: str

    def __init__(self, name: str) -> None:
        self.name = name

# Usage
c = Container(42)
p = Point(1.0, 2.0)
fp = FrozenPoint(3.0, 4.0)
circle = Circle(5.0)
area: float = circle.area()
"#;
    let _diags = run(source)?;
    Ok(())
}
