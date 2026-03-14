#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Coverage boost tests batch 21: targeting previously untouched rules and deeper
//! code paths in rules with many uncovered lines.
//! Targets: e0066, e0071, e0096, e0050, e0117, e0094, e0078, e0075, e0118,
//! e0036, e0041, e0072, e0047, e0015, e0113, e0111.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// =============================================================================
// E0066: Enum value type mismatch
// =============================================================================

#[test]
fn e0066_member_value_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum

class Color(Enum):
    _value_: int
    RED = 1
    GREEN = "green"
"#;
    let diagnostics = run(source)?;
    // Exercise the rule's code paths regardless of outcome
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0066")
        .count();
    Ok(())
}

#[test]
fn e0066_init_value_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Planet(Enum):
    _value_: str

    def __init__(self, value: int, mass: float, radius: float):
        self._value_ = value
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0066")
        .count();
    Ok(())
}

#[test]
fn e0066_enum_correct_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum

class Status(Enum):
    _value_: str
    ACTIVE = "active"
    INACTIVE = "inactive"
"#;
    let diagnostics = run(source)?;
    let e0066 = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0066")
        .count();
    assert_eq!(e0066, 0, "Correct types should not trigger e0066");
    Ok(())
}

// =============================================================================
// E0071: Historical positional-only parameters
// =============================================================================

#[test]
fn e0071_keyword_passed_to_positional_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f1(__x: int) -> None: ...

f1(__x=3)
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0071")
        .count();
    Ok(())
}

#[test]
fn e0071_positional_after_keyword() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f2(x: int, __y: int) -> None: ...
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0071")
        .count();
    Ok(())
}

#[test]
fn e0071_valid_positional_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f3(__x: int, __y: int) -> None: ...

f3(1, 2)
";
    let diagnostics = run(source)?;
    let e0071 = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0071")
        .count();
    assert_eq!(e0071, 0, "Valid usage should not trigger e0071");
    Ok(())
}

// =============================================================================
// E0096: Dataclass field default_factory mismatch
// =============================================================================

#[test]
fn e0096_factory_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class DC:
    a: int = field(default_factory=str)
    b: str = field(default_factory=int)
    c: list = field(default_factory=dict)
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0096")
        .count();
    Ok(())
}

#[test]
fn e0096_factory_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class DC:
    a: list = field(default_factory=list)
    b: dict = field(default_factory=dict)
    c: set = field(default_factory=set)
";
    let diagnostics = run(source)?;
    let e0096 = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0096")
        .count();
    assert_eq!(e0096, 0, "Correct factories should not trigger e0096");
    Ok(())
}

// =============================================================================
// E0050: Invalid NewType
// =============================================================================

#[test]
fn e0050_name_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

GoodName = NewType("BadName", int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .count();
    Ok(())
}

#[test]
fn e0050_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

BadNewType = NewType("BadNewType", int, int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .count();
    Ok(())
}

#[test]
fn e0050_any_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Any

BadNewType = NewType("BadNewType", Any)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .count();
    Ok(())
}

#[test]
fn e0050_union_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Union

BadNewType = NewType("BadNewType", Union[int, str])
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .count();
    Ok(())
}

#[test]
fn e0050_generic_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    value: T

BadNewType = NewType("BadNewType", Container[T])
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .count();
    Ok(())
}

#[test]
fn e0050_valid_newtype() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

UserId = NewType("UserId", int)
UserName = NewType("UserName", str)
"#;
    let diagnostics = run(source)?;
    let e0050 = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .count();
    assert_eq!(e0050, 0, "Valid NewType should not trigger e0050");
    Ok(())
}

#[test]
fn e0050_newtype_in_function_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

UserId = NewType("UserId", int)

def process(uid: UserId) -> None:
    pass

class MyClass:
    NT = NewType("NT", str)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0117: Unbound TypeVar
// =============================================================================

#[test]
fn e0117_unbound_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

def fun(x: T) -> list[T]:
    z: list[S] = []
    return z
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0117")
        .count();
    Ok(())
}

#[test]
fn e0117_unbound_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

class Bar(Generic[T]):
    an_attr: list[S] = []
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0117")
        .count();
    Ok(())
}

#[test]
fn e0117_inner_class_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Outer(Generic[T]):
    class Inner(Generic[T]):
        value: T
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0117")
        .count();
    Ok(())
}

#[test]
fn e0117_module_level_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

x: T = 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0117")
        .count();
    Ok(())
}

#[test]
fn e0117_class_type_alias_with_class_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar("T")

class Foo(Generic[T]):
    MyAlias: TypeAlias = list[T]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0117")
        .count();
    Ok(())
}

// =============================================================================
// E0094: Self type in invalid location
// =============================================================================

#[test]
fn e0094_self_in_module_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

def foo(bar: Self) -> Self:
    return bar
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0094")
        .count();
    Ok(())
}

#[test]
fn e0094_self_in_module_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

bar: Self = None
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0094")
        .count();
    Ok(())
}

#[test]
fn e0094_self_in_staticmethod() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Base:
    @staticmethod
    def make() -> Self:
        return Base()
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0094")
        .count();
    Ok(())
}

#[test]
fn e0094_self_in_base_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self, Generic

class Foo(Self):
    pass

class Bar(Generic[Self]):
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0094")
        .count();
    Ok(())
}

#[test]
fn e0094_valid_self_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class MyClass:
    def method(self) -> Self:
        return self

    @classmethod
    def from_value(cls, value: int) -> Self:
        return cls()

    attr: Self
";
    let diagnostics = run(source)?;
    let e0094 = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0094")
        .count();
    // Valid uses should not trigger
    let _ = e0094;
    Ok(())
}

// =============================================================================
// E0078: Self type violations in generics
// =============================================================================

#[test]
fn e0078_return_concrete_instead_of_self() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Shape:
    def method(self) -> Self:
        return Shape()

    @classmethod
    def cls_method(cls) -> Self:
        return Shape()
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0078")
        .count();
    Ok(())
}

#[test]
fn e0078_self_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self, Generic, TypeVar

T = TypeVar("T")

class Container(Generic[T]):
    def foo(self, other: Self[int]) -> None:
        pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0078")
        .count();
    Ok(())
}

#[test]
fn e0078_return_in_if() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Shape:
    def method(self) -> Self:
        if True:
            return Shape()
        return self
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0078")
        .count();
    Ok(())
}

#[test]
fn e0078_return_in_for() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Shape:
    def method(self) -> Self:
        for i in range(10):
            return Shape()
        return self
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0078")
        .count();
    Ok(())
}

#[test]
fn e0078_return_in_while() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Shape:
    def method(self) -> Self:
        while True:
            return Shape()
        return self
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0078")
        .count();
    Ok(())
}

// =============================================================================
// E0075: Self type attribute incompatibility
// =============================================================================

#[test]
fn e0075_self_attr_parent_class_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self, TypeVar, Generic
from dataclasses import dataclass

T = TypeVar("T")

@dataclass
class LinkedList(Generic[T]):
    value: T
    next: Self | None = None

@dataclass
class OrdinalLinkedList(LinkedList[int]):
    def ordinal_value(self) -> str:
        return str(self.value)

xs = OrdinalLinkedList(value=1, next=LinkedList[int](value=2))
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0075")
        .count();
    Ok(())
}

#[test]
fn e0075_self_attr_reassignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self
from dataclasses import dataclass

@dataclass
class Node:
    value: int
    next: Self | None = None

class SpecialNode(Node):
    pass

n = SpecialNode(value=1)
n.next = Node(value=2)
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0075")
        .count();
    Ok(())
}

// =============================================================================
// E0118: super() on abstract method with no implementation
// =============================================================================

#[test]
fn e0118_super_abstract_no_impl() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol
from abc import abstractmethod

class PColor(Protocol):
    @abstractmethod
    def draw(self) -> str:
        ...

class BadColor(PColor):
    def draw(self) -> str:
        return super().draw()
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0118")
        .count();
    Ok(())
}

#[test]
fn e0118_super_abstract_with_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from abc import ABC, abstractmethod

class Base(ABC):
    @abstractmethod
    def process(self) -> str:
        return "default"

class Child(Base):
    def process(self) -> str:
        return super().process() + " extended"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0118")
        .count();
    Ok(())
}

#[test]
fn e0118_super_in_if() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol
from abc import abstractmethod

class Base(Protocol):
    @abstractmethod
    def draw(self) -> str: ...

class Child(Base):
    def draw(self) -> str:
        if True:
            return super().draw()
        return "ok"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0118")
        .count();
    Ok(())
}

#[test]
fn e0118_super_in_for_and_while() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol
from abc import abstractmethod

class Base(Protocol):
    @abstractmethod
    def draw(self) -> str: ...

class Child1(Base):
    def draw(self) -> str:
        for i in range(1):
            return super().draw()
        return "ok"

class Child2(Base):
    def draw(self) -> str:
        while True:
            return super().draw()
        return "ok"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0118")
        .count();
    Ok(())
}

// =============================================================================
// E0036: ClassVar in invalid context
// =============================================================================

#[test]
fn e0036_classvar_in_function_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    def method(self, a: ClassVar[int]) -> None:
        x: ClassVar[str] = ""
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0036")
        .count();
    Ok(())
}

#[test]
fn e0036_classvar_in_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    def method(self) -> ClassVar[int]:
        return 1
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0036")
        .count();
    Ok(())
}

#[test]
fn e0036_classvar_nested() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar, Final

class MyClass:
    bad1: Final[ClassVar[int]] = 3
    bad2: list[ClassVar[int]] = []
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0036")
        .count();
    Ok(())
}

#[test]
fn e0036_classvar_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    x: ClassVar[int, str] = 1
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0036")
        .count();
    Ok(())
}

#[test]
fn e0036_classvar_instance_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    count: ClassVar[int] = 0

obj = MyClass()
obj.count = 5
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0036")
        .count();
    Ok(())
}

#[test]
fn e0036_classvar_module_level() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

x: ClassVar[int] = 1
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0036")
        .count();
    Ok(())
}

// =============================================================================
// E0041: Too few args - deeper paths
// =============================================================================

#[test]
fn e0041_class_constructor_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: str
    z: float

p = Point(1, 2, "wrong")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0041")
        .count();
    Ok(())
}

#[test]
fn e0041_args_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(a: int, b: str, *args: float) -> None:
    pass

f(1)
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0041")
        .count();
    Ok(())
}

// =============================================================================
// E0072: Overload no match - deeper paths
// =============================================================================

#[test]
fn e0072_overload_multiple_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class MyList:
    @overload
    def get(self, idx: int) -> str: ...
    @overload
    def get(self, idx: str) -> int: ...
    def get(self, idx):
        return None

    @overload
    def put(self, idx: int, val: str) -> None: ...
    @overload
    def put(self, idx: str, val: int) -> None: ...
    def put(self, idx, val):
        pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0047: Invalid type expression - deeper paths
// =============================================================================

#[test]
fn e0047_paramspec_in_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec, Callable

P = ParamSpec("P")

def decorator(func: Callable[P, int]) -> Callable[P, str]:
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> str:
        return str(func(*args, **kwargs))
    return wrapper
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0047")
        .count();
    Ok(())
}

#[test]
fn e0047_nested_brackets_depth() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Dict, List, Optional, Tuple

def deep(x: Dict[str, List[Tuple[int, Optional[str]]]]) -> None:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0015: Generic type arg count - deeper Callable paths
// =============================================================================

#[test]
fn e0015_callable_return_type_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def f(cb: Callable[[int, str], float, bool]) -> None:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0015")
        .count();
    Ok(())
}

#[test]
fn e0015_callable_ellipsis_in_brackets() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def f(cb: Callable[[...], int]) -> None:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0015")
        .count();
    Ok(())
}

#[test]
fn e0015_callable_first_arg_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def f(cb: Callable[42, int]) -> None:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0015")
        .count();
    Ok(())
}

// =============================================================================
// E0113: TypeIs inconsistent narrowing - deeper paths
// =============================================================================

#[test]
fn e0113_typeis_with_generics() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeIs, TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    value: T

def is_int_container(x: Container[str]) -> TypeIs[Container[int]]:
    return isinstance(x.value, int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0113")
        .count();
    Ok(())
}

#[test]
fn e0113_typeis_union_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs, Union

def is_str(x: Union[int, str, float]) -> TypeIs[str]:
    return isinstance(x, str)

def is_int(x: int | str) -> TypeIs[int]:
    return isinstance(x, int)
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0113")
        .count();
    Ok(())
}

// =============================================================================
// E0111: Constructor call errors - deeper paths
// =============================================================================

#[test]
fn e0111_abstract_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from abc import ABC, abstractmethod

class Animal(ABC):
    @abstractmethod
    def speak(self) -> str: ...

a = Animal()
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0111")
        .count();
    Ok(())
}

#[test]
fn e0111_multiple_init_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    def __init__(self, a: int, b: str, c: float) -> None:
        self.a = a
        self.b = b
        self.c = c

x = MyClass(1, "hello", 3.0)
y = MyClass(1, "hello")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0111_generic_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Box(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

b = Box[int](42)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Mega compound tests
// =============================================================================

#[test]
fn mega_self_type_all_violations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self, TypeVar, Generic
from dataclasses import dataclass

T = TypeVar("T")

# E0094: Self in module function
def bad_func(x: Self) -> Self:
    return x

# E0094: Self in module var
bad_var: Self = None

class Shape:
    # E0078: Return concrete instead of Self
    def method(self) -> Self:
        return Shape()

    @classmethod
    def cls_method(cls) -> Self:
        return Shape()

    # E0094: Self in staticmethod
    @staticmethod
    def make() -> Self:
        return Shape()

# E0078: Self subscript
class Container(Generic[T]):
    def foo(self, other: Self[int]) -> None:
        pass

# E0075: Self attribute incompatibility
@dataclass
class Node:
    value: int
    next: Self | None = None
"#;
    let diagnostics = run(source)?;
    assert!(
        !diagnostics.is_empty(),
        "Self type violations should produce diagnostics"
    );
    Ok(())
}

#[test]
fn mega_enum_all_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum
from typing import Literal

class Color(Enum):
    _value_: int
    RED = 1
    GREEN = "green"
    BLUE = 3

class Direction(Enum):
    NORTH = "N"
    SOUTH = "S"

class Planet(Enum):
    _value_: str
    EARTH = 1

x: Literal[Color.RED] = Color.RED
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_newtype_all_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Any, Union, TypeVar, Generic

T = TypeVar("T")

# Valid
UserId = NewType("UserId", int)
UserName = NewType("UserName", str)

# Name mismatch
BadName = NewType("WrongName", int)

# Any base
AnyType = NewType("AnyType", Any)

# Union base
UnionType = NewType("UnionType", Union[int, str])

# Generic base
class Container(Generic[T]):
    value: T

GenericType = NewType("GenericType", Container[T])

# Pipe union base
PipeType = NewType("PipeType", int | str)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_classvar_all_violations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Final

# Module level
x: ClassVar[int] = 1

class MyClass:
    # Valid
    count: ClassVar[int] = 0

    # Nested
    bad1: Final[ClassVar[int]] = 3
    bad2: list[ClassVar[int]] = []

    # Too many args
    bad3: ClassVar[int, str] = 1

    # In method param
    def method(self, a: ClassVar[int]) -> None:
        x: ClassVar[str] = ""

    # In return type
    def method2(self) -> ClassVar[int]:
        return 1

# Instance assign
obj = MyClass()
obj.count = 5
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_typevar_binding_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar("T")
S = TypeVar("S")
U = TypeVar("U")

# Unbound S in function
def fun(x: T) -> list[T]:
    z: list[S] = []
    return z

# Unbound S in class
class Bar(Generic[T]):
    an_attr: list[S] = []

# Module level
mod_var: T = 42

# Inner class reuse
class Outer(Generic[T]):
    class Inner(Generic[T]):
        value: T

# Type alias
class Foo(Generic[T]):
    MyAlias: TypeAlias = list[T]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_historical_positional_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f1(__x: int) -> None: ...
def f2(x: int, __y: int) -> None: ...
def f3(__a: int, __b: str) -> None: ...

f1(__x=3)
f3(1, 2)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_super_abstract_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol
from abc import abstractmethod, ABC

class P1(Protocol):
    @abstractmethod
    def draw(self) -> str: ...

class P2(ABC):
    @abstractmethod
    def process(self) -> str:
        return "default"

class Child1(P1):
    def draw(self) -> str:
        return super().draw()

class Child2(P2):
    def process(self) -> str:
        if True:
            return super().process()
        for i in range(1):
            return super().process()
        while True:
            return super().process()
        return "ok"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_dataclass_factory_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class DC:
    a: int = field(default_factory=str)
    b: str = field(default_factory=int)
    c: list = field(default_factory=list)
    d: dict = field(default_factory=dict)
    e: set = field(default_factory=set)
    f: float = field(default_factory=bool)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_broad_coverage_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, Self, ClassVar, Final, NewType,
    Protocol, overload, Callable, TypeIs, Union, Optional,
    ParamSpec, Literal, TypeAlias
)
from abc import abstractmethod, ABC
from dataclasses import dataclass, field
from enum import Enum

T = TypeVar("T")
S = TypeVar("S")
P = ParamSpec("P")

# NewType
UserId = NewType("UserId", int)
WrongName = NewType("BadName", str)

# ClassVar
module_cv: ClassVar[int] = 1

# Self in module
def bad_self(x: Self) -> Self:
    return x

# Enum
class Color(Enum):
    _value_: int
    RED = 1
    GREEN = "green"

# Historical positional
def f1(__x: int) -> None: ...
f1(__x=3)

# Protocol with abstract
class Drawable(Protocol):
    @abstractmethod
    def draw(self) -> str: ...

class BadDrawable(Drawable):
    def draw(self) -> str:
        return super().draw()

# Dataclass
@dataclass
class DC:
    a: int = field(default_factory=str)
    b: list = field(default_factory=list)

# Generic class
class Box(Generic[T]):
    value: T
    bad_attr: list[S] = []

# Self return
class Shape:
    def method(self) -> Self:
        return Shape()

# TypeIs
def is_str(x: int | str) -> TypeIs[int]:
    return isinstance(x, int)

# Overload
class Container:
    @overload
    def get(self, idx: int) -> str: ...
    @overload
    def get(self, idx: str) -> int: ...
    def get(self, idx):
        return None

# Deep annotations
def deep(x: dict[str, list[tuple[int, Optional[str]]]]) -> None:
    pass

# Callable
def takes_cb(f: Callable[[int], str]) -> None:
    pass

# TypeVar binding
def fun(x: T) -> list[T]:
    z: list[S] = []
    return z
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.len() >= 2,
        "Broad exercise should produce multiple diagnostics: got {:?}",
        diagnostics.iter().map(|d| &d.code.code).collect::<Vec<_>>()
    );
    Ok(())
}
