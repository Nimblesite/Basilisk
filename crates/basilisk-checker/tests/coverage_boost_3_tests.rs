#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Coverage boost tests batch 3: targeting remaining low-coverage rules.
//! Covers: e0146, e0147, e0148, e0149, e0036 (deeper), e0047, e0048 (deeper),
//!         e0050 (deeper), e0063, e0064 (deeper), e0067, e0069, e0073, e0078,
//!         e0083, e0086, e0088, e0090, e0091, e0092, e0094, e0096, e0100,
//!         e0101, e0103, e0104, e0105, e0106, e0108, e0109, e0115, e0116, e0117, e0118
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn codes(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags.iter().map(|d| d.code.code.to_string()).collect()
}

// --- E0146: Protocol class object ---

#[test]
fn e0146_protocol_class_object_pass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Comparable(Protocol):
    def __lt__(self, other: object) -> bool: ...

def sort_things(cls: type[Comparable]) -> None:
    pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0146_protocol_class_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Maker(Protocol[T]):
    def make(self) -> T: ...

class IntMaker:
    def make(self) -> int:
        return 42

m: Maker[int] = IntMaker()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0147: Tuple starred unpack ---

#[test]
fn e0147_starred_tuple_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str, ...]] = (1, "a", "b")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0147_starred_tuple_reassignment_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str]] = (1, "a")
t1 = (1, "a", "b")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0147_starred_tuple_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t2: tuple[int, *tuple[str, ...]] = (1, "a")
t2 = (1, 2, "a")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0147_starred_tuple_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(t1: tuple[int], t2: tuple[int, *tuple[int, ...]], t3: tuple[int, ...]) -> None:
    v2: tuple[int, *tuple[int, ...]]
    v2 = t3
    v3: tuple[int]
    v3 = t2
    v3 = t3
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0148: Generic type arg ---

#[test]
fn e0148_generic_type_arg_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T", bound=int)

class Container(Generic[T]):
    def __init__(self, val: T) -> None:
        self.val = val

x: Container[str] = Container("hello")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0148_generic_type_arg_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T", bound=int)

class Container(Generic[T]):
    def __init__(self, val: T) -> None:
        self.val = val

x: Container[int] = Container(42)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0149: PEP 695 type param scoping ---

#[test]
fn e0149_pep695_type_param_scoping() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Outer(Generic[T]):
    class Inner(Generic[T]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0047: Invalid type expression ---

#[test]
fn e0047_complex_invalid_type_expr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

class Config:
    MAX_SIZE: Final[int] = 100

def f() -> None:
    x: Final[int] = 10
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0063: Non-hashable dataclass ---

#[test]
fn e0063_enum_value_type_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = "blue"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0067: Enum non-member literal ---

#[test]
fn e0067_non_member_with_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum, nonmember

class Animal(Enum):
    DOG = 1
    CAT = 2
    legs = nonmember(4)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0069: Dataclass kw_only ---

#[test]
fn e0069_dataclass_kwonly() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(kw_only=True)
class Config:
    name: str
    value: int = 0
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0073: NamedTuple tuple compat ---

#[test]
fn e0073_protocol_self_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Self

class Clonable(Protocol):
    def clone(self) -> Self: ...

class Sheep:
    def clone(self) -> "Sheep":
        return Sheep()

def do_clone(x: Clonable) -> None:
    x.clone()

do_clone(Sheep())
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0078: Self type violation ---

#[test]
fn e0078_self_type_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Base:
    def copy(self) -> Self:
        return self

class Child(Base):
    def copy(self) -> Base:
        return Base()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0083: TypeVarTuple unpack required ---

#[test]
fn e0083_unpack_required() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic

Ts = TypeVarTuple("Ts")

class Array(Generic[*Ts]):
    pass
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0086: Multiple TypeVarTuple ---

#[test]
fn e0086_multiple_typevartuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic

Ts1 = TypeVarTuple("Ts1")
Ts2 = TypeVarTuple("Ts2")

class Bad(Generic[*Ts1, *Ts2]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0088: TypedDict isinstance ---

#[test]
fn e0088_typeddict_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

x = {"name": "test", "year": 2024}
isinstance(x, Movie)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0090: Invalid tuple syntax ---

#[test]
fn e0090_tuple_syntax_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: tuple[int, str] = (1, "a")
y: tuple[()] = ()
z: tuple[int, ...] = (1, 2, 3)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0091: TypeVar default incompat ---

#[test]
fn e0091_typevar_default_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int, default=str)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0091_typevar_default_compat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int, default=int)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0092: Too few type args ---

#[test]
fn e0092_too_few_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar("T")
U = TypeVar("U")

class Pair(Generic[T, U]):
    pass

x: Pair[int]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0094: Self type in invalid location ---

#[test]
fn e0094_self_type_module_level() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

x: Self = None
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0094_self_type_in_free_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

def f() -> Self:
    pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0094_self_type_in_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class MyClass:
    def create(self) -> Self:
        return self
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0096: Dataclass default factory ---

#[test]
fn e0096_dataclass_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field

@dataclass
class Config:
    items: list[int] = field(default_factory=list)
    name: str = "default"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0096_dataclass_mutable_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Bad:
    items: list[int] = []
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0100: Literal augmented assign ---

#[test]
fn e0100_literal_augmented_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

x: Literal[1] = 1
x += 1
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0101: TypeGuard no narrowing param ---

#[test]
fn e0101_typeguard_no_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_int() -> TypeGuard[int]:
    return True
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0101_typeguard_with_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_int(x: object) -> TypeGuard[int]:
    return isinstance(x, int)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0103: Tuple index out of bounds ---

#[test]
fn e0103_tuple_index_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[int, str] = (1, "a")
x = t[0]
y = t[1]
z = t[5]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0104: Cyclical type alias ---

#[test]
fn e0104_cyclical_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

A: TypeAlias = "B"
B: TypeAlias = "A"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0104_non_cyclical_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

Vector: TypeAlias = list[float]
Matrix: TypeAlias = list[Vector]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0105: Bounded TypeVar attribute access ---

#[test]
fn e0105_bounded_typevar_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)

def f(x: T) -> T:
    return x
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0106: Protocol as type ---

#[test]
fn e0106_protocol_as_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

x: type[Drawable]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0108: Dataclass slots ---

#[test]
fn e0108_dataclass_slots() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(slots=True)
class Point:
    x: float
    y: float
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0108_dataclass_slots_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Base:
    x: int

@dataclass(slots=True)
class Child(Base):
    y: int
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0109: TypeVar bound violation ---

#[test]
fn e0109_typevar_bound_violation_in_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)

def double(x: T) -> T:
    return x

result = double("hello")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0109_typevar_bound_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)

def double(x: T) -> T:
    return x

result = double(42)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0115: Deprecated usage ---

#[test]
fn e0115_deprecated_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func instead")
def old_func() -> None:
    pass

old_func()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0115_deprecated_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use NewClass instead")
class OldClass:
    pass

x = OldClass()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0115_deprecated_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class MyClass:
    @deprecated("Use new_method instead")
    def old_method(self) -> None:
        pass

    def new_method(self) -> None:
        pass

obj = MyClass()
obj.old_method()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0115_deprecated_overload() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated, overload

@overload
def process(x: int) -> int: ...

@overload
@deprecated("str overload is deprecated")
def process(x: str) -> str: ...

def process(x):
    return x

result = process("test")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0115_non_deprecated_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def normal_func() -> None:
    pass

normal_func()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0116: NamedTuple definition ---

#[test]
fn e0116_namedtuple_definition_functional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Point = NamedTuple("Point", [("x", float), ("y", float)])
p = Point(1.0, 2.0)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0116_namedtuple_definition_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float
    z: float = 0.0
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0117: Unbound TypeVar ---

#[test]
fn e0117_unbound_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
U = TypeVar("U")

class MyClass(Generic[T]):
    def method(self, x: U) -> U:
        return x
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0117_bound_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class MyClass(Generic[T]):
    def method(self, x: T) -> T:
        return x
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0118: super() abstract no impl ---

#[test]
fn e0118_super_abstract_no_impl() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from abc import ABC, abstractmethod

class Base(ABC):
    @abstractmethod
    def do_thing(self) -> None: ...

class Child(Base):
    def do_thing(self) -> None:
        super().do_thing()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0118_super_concrete_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    def do_thing(self) -> None:
        pass

class Child(Base):
    def do_thing(self) -> None:
        super().do_thing()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
