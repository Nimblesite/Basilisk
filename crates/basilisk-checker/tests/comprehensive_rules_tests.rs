//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Comprehensive integration tests that exercise many checker rules through complex Python code.
//! These tests ensure code paths in rule implementations are covered even when no diagnostics fire.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn _codes(diags: &[basilisk_checker::Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

// ============================================================================
// Protocol-related rules (E0077, E0097, E0106, E0110, E0114, E0119, E0121, E0123, E0124, E0133, E0137, E0146)
// ============================================================================

#[test]
fn exercise_protocol_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, Self

class Copyable(Protocol):
    def copy(self) -> Self: ...

class Hashable(Protocol):
    def __hash__(self) -> int: ...

class Comparable(Protocol):
    def __lt__(self, other: Self) -> bool: ...
    def __gt__(self, other: Self) -> bool: ...

class MyCls:
    def copy(self) -> MyCls:
        return MyCls()
    def __hash__(self) -> int:
        return 0
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_protocol_with_attributes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Sized(Protocol):
    size: int
    def get_size(self) -> int: ...

class Container(Protocol):
    items: list[int]
    def add(self, item: int) -> None: ...
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_protocol_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Reader(Protocol[T_co]):
    def read(self) -> T_co: ...

class Writer(Protocol[T_contra]):
    def write(self, data: T_contra) -> None: ...
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// TypeVar-related rules (E0080, E0091, E0102, E0105, E0109, E0117, E0128, E0130, E0132)
// ============================================================================

#[test]
fn exercise_typevar_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)
U = TypeVar("U", bound=str)
V = TypeVar("V", int, str)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_typevar_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T", default=int)
U = TypeVar("U", default=str)

class Container(Generic[T, U]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_typevar_covariant_contravariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Producer(Generic[T_co]):
    pass

class Consumer(Generic[T_contra]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// TypeVarTuple-related rules (E0081-E0086, E0139)
// ============================================================================

#[test]
fn exercise_typevartuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack

Ts = TypeVarTuple("Ts")

class Array(Generic[*Ts]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_paramspec() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec, Callable, TypeVar

P = ParamSpec("P")
R = TypeVar("R")

def decorator(func: Callable[P, R]) -> Callable[P, R]:
    return func
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Dataclass-related rules (E0059, E0060, E0063, E0069, E0096, E0108, E0138, E0142)
// ============================================================================

#[test]
fn exercise_dataclass_features() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class Point:
    x: float
    y: float

@dataclass(frozen=True)
class FrozenPoint:
    x: float
    y: float

@dataclass(order=True)
class OrderedPoint:
    x: float = 0.0
    y: float = 0.0

@dataclass(slots=True)
class SlottedPoint:
    x: float
    y: float
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_dataclass_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass(kw_only=True)
class Config:
    name: str
    value: int = 0

@dataclass
class MixedConfig:
    pos: str
    kw: int = field(kw_only=True, default=0)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_dataclass_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class Container:
    items: list[int] = field(default_factory=list)
    mapping: dict[str, int] = field(default_factory=dict)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_dataclass_match_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(match_args=False)
class Point:
    x: float
    y: float
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Enum-related rules (E0040, E0066, E0067, E0068)
// ============================================================================

#[test]
fn exercise_enum_variants() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum, IntEnum, StrEnum, Flag

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

class Status(StrEnum):
    ACTIVE = "active"
    INACTIVE = "inactive"

class Perm(IntEnum):
    READ = 4
    WRITE = 2
    EXEC = 1

class FileFlag(Flag):
    R = 4
    W = 2
    X = 1
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// TypedDict-related rules (E0029, E0032, E0035, E0037, E0038, E0056, E0088, E0093)
// ============================================================================

#[test]
fn exercise_typeddict_variants() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict, Required, NotRequired

class Movie(TypedDict):
    title: str
    year: int

class PartialMovie(TypedDict, total=False):
    title: str
    year: int

class MixedMovie(TypedDict):
    title: Required[str]
    director: NotRequired[str]
    year: int
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_typeddict_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Base(TypedDict):
    name: str

class Extended(Base):
    age: int

class MoreExtended(Extended):
    email: str
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Callable-related rules (E0122, E0136, E0140)
// ============================================================================

#[test]
fn exercise_callable_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def apply(f: Callable[[int], str], x: int) -> str:
    return f(x)

def compose(f: Callable[[int], str], g: Callable[[str], bool]) -> Callable[[int], bool]:
    def composed(x: int) -> bool:
        return g(f(x))
    return composed

callback: Callable[..., None] = print
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Generator-related rules (E0120, E0131)
// ============================================================================

#[test]
fn exercise_generator_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, Iterator, AsyncGenerator

def count() -> Generator[int, None, None]:
    yield 1
    yield 2
    yield 3

def items() -> Iterator[str]:
    yield "a"
    yield "b"
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Overload-related rules (E0020, E0021, E0072, E0076)
// ============================================================================

#[test]
fn exercise_overload_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
@overload
def process(x: float) -> float: ...
def process(x: int | str | float) -> int | str | float:
    return x
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_class_overloads() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class Converter:
    @overload
    def convert(self, x: int) -> str: ...
    @overload
    def convert(self, x: str) -> int: ...
    def convert(self, x: int | str) -> int | str:
        if isinstance(x, int):
            return str(x)
        return len(x)
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Class inheritance rules (E0016, E0017, BSK-E0025, E0034, E0107, E0118, E0125, E0134)
// ============================================================================

#[test]
fn exercise_class_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import override

class Base:
    count: int = 0
    def process(self, data: str) -> str:
        return data

class Child(Base):
    count: int = 10
    @override
    def process(self, data: str) -> str:
        return data.upper()

class GrandChild(Child):
    @override
    def process(self, data: str) -> str:
        return data.lower()
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_abstract_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from abc import ABC, abstractmethod

class Shape(ABC):
    @abstractmethod
    def area(self) -> float: ...

    @abstractmethod
    def perimeter(self) -> float: ...

class Circle(Shape):
    def __init__(self, radius: float) -> None:
        self.radius: float = radius

    def area(self) -> float:
        return 3.14 * self.radius * self.radius

    def perimeter(self) -> float:
        return 2 * 3.14 * self.radius
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Literal-related rules (E0051, E0100, E0103, E0126, E0127, E0129)
// ============================================================================

#[test]
fn exercise_literal_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal, Final

x: Literal[1, 2, 3] = 1
y: Literal["a", "b"] = "a"
z: Literal[True, False] = True

MAX: Final[int] = 100
NAME: Final = "basilisk"
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Match statement rules (E0023)
// ============================================================================

#[test]
fn exercise_match_statements() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def handle(cmd: str) -> str:
    match cmd:
        case "start":
            return "starting"
        case "stop":
            return "stopping"
        case _:
            return "unknown"
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// PEP 695 rules (E0042, E0057, E0089, E0149)
// ============================================================================

#[test]
fn exercise_pep695_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Container[T]:
    value: T

    def get(self) -> T:
        return self.value

def first[T](items: list[T]) -> T:
    return items[0]

type Vector = list[float]
type Matrix = list[list[float]]
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Self type rules (E0075, E0078, E0094)
// ============================================================================

#[test]
fn exercise_self_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Builder:
    def set_name(self, name: str) -> Self:
        return self

    def set_value(self, value: int) -> Self:
        return self

    def build(self) -> Self:
        return self
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// NoReturn/Never rules (E0062, E0070)
// ============================================================================

#[test]
fn exercise_noreturn() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NoReturn, Never

def fail() -> NoReturn:
    raise RuntimeError("failed")

def unreachable() -> Never:
    raise AssertionError("unreachable")
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// TypeGuard/TypeIs rules (E0101, E0112, E0113)
// ============================================================================

#[test]
fn exercise_typeguard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_str(x: object) -> TypeGuard[str]:
    return isinstance(x, str)

def is_int_list(x: object) -> TypeGuard[list[int]]:
    return isinstance(x, list)
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Deprecated rules (E0115)
// ============================================================================

#[test]
fn exercise_deprecated() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func instead")
def old_func() -> None:
    pass

class MyClass:
    @deprecated("Use new_method")
    def old_method(self) -> None:
        pass
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// NamedTuple rules (E0064, E0073, E0116, E0143)
// ============================================================================

#[test]
fn exercise_namedtuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

class Point3D(NamedTuple):
    x: int
    y: int
    z: int = 0

p = Point(1, 2)
p3 = Point3D(1, 2, 3)
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Constructor rules (E0074, E0111, E0144)
// ============================================================================

#[test]
fn exercise_constructor_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Animal:
    def __init__(self, name: str, age: int) -> None:
        self.name: str = name
        self.age: int = age

class Dog(Animal):
    def __init__(self, name: str, age: int, breed: str) -> None:
        super().__init__(name, age)
        self.breed: str = breed

a = Animal("cat", 3)
d = Dog("fido", 5, "labrador")
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Class attribute rules (BSK-E0005, E0036, E0044, E0045, E0047, E0054, E0100, E0125)
// ============================================================================

#[test]
fn exercise_classvar_and_final() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Final

class Config:
    MAX_SIZE: ClassVar[int] = 100
    DEFAULT_NAME: Final[str] = "default"
    instance_var: int

    def __init__(self, val: int) -> None:
        self.instance_var = val
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Inference edge cases
// ============================================================================

#[test]
fn exercise_inference_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# Various RHS kinds for inference
x: int = 42
y: str = "hello"
z: float = 3.14
b: bool = True
n: bytes = b"hello"
none_val: int | None = None
empty_list: list[int] = []
empty_dict: dict[str, int] = {}
non_empty: list[int] = [1, 2, 3]
mixed: list[int | str] = [1, "two", 3]
tuple_val: tuple[int, str] = (1, "hello")
set_val: set[int] = {1, 2, 3}
dict_val: dict[str, int] = {"a": 1, "b": 2}
"#;
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// dataclass_transform rules (E0138, E0142)
// ============================================================================

#[test]
fn exercise_dataclass_transform_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform(frozen_default=True, order_default=True)
def create_model(cls: type) -> type:
    return cls

@create_model
class Customer:
    id: int
    name: str

@dataclass_transform(frozen_default=False)
def mutable_model(cls: type) -> type:
    return cls

@mutable_model
class MutableCustomer:
    id: int
    name: str
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_dataclass_transform_with_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform()
def model(cls: type) -> type:
    return cls

@model(frozen=True)
class FrozenModel:
    x: int

@model(order=True)
class OrderedModel:
    x: int
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Unpack kwargs (E0141)
// ============================================================================

#[test]
fn exercise_unpack_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict, Unpack

class Options(TypedDict, total=False):
    verbose: bool
    debug: bool
    log_level: int

def configure(**kwargs: Unpack[Options]) -> None:
    pass
";
    let _ = run(source)?;
    Ok(())
}

// ============================================================================
// Flow union tracker
// ============================================================================

#[test]
fn exercise_conditional_assignments() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(flag: bool) -> int:
    if flag:
        result = 42
    else:
        result = 0
    return result
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn exercise_complex_control_flow() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def complex(x: int, y: str) -> str:
    if x > 0:
        val = "positive"
    elif x < 0:
        val = "negative"
    else:
        val = "zero"
    return val
"#;
    let _ = run(source)?;
    Ok(())
}
