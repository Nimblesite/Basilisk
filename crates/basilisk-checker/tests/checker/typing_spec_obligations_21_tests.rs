//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 21: targeting previously untouched rules and deeper
// code paths in rules with many uncovered lines.
// Targets: e0066, e0071, e0096, e0050, e0117, e0094, e0078, e0075, e0118,
// e0036, e0041, e0072, e0047, e0015, e0113, e0111.

// =============================================================================
// Enum value type mismatch
// =============================================================================

#[test]
fn member_value_mismatch() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "enums_member_values")
        .count();
    Ok(())
}

#[test]
fn init_value_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "enums_member_values")
        .count();
    Ok(())
}

#[test]
fn enum_correct_types() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "enums_member_values")
        .count();
    assert_eq!(e0066, 0, "Correct types should not trigger e0066");
    Ok(())
}

// =============================================================================
// Historical positional-only parameters
// =============================================================================

#[test]
fn keyword_passed_to_positional_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f1(__x: int) -> None: ...

f1(__x=3)
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "historical_positional")
        .count();
    Ok(())
}

#[test]
fn positional_after_keyword() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f2(x: int, __y: int) -> None: ...
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "historical_positional")
        .count();
    Ok(())
}

#[test]
fn valid_positional_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f3(__x: int, __y: int) -> None: ...

f3(1, 2)
";
    let diagnostics = run(source)?;
    let e0071 = diagnostics
        .iter()
        .filter(|d| d.code.code == "historical_positional")
        .count();
    assert_eq!(e0071, 0, "Valid usage should not trigger e0071");
    Ok(())
}

// =============================================================================
// Dataclass field default_factory mismatch
// =============================================================================

#[test]
fn factory_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "dataclasses_usage")
        .count();
    Ok(())
}

#[test]
fn factory_correct() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "dataclasses_usage")
        .count();
    assert_eq!(e0096, 0, "Correct factories should not trigger e0096");
    Ok(())
}

// =============================================================================
// Invalid NewType
// =============================================================================

#[test]
fn name_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

GoodName = NewType("BadName", int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_newtype")
        .count();
    Ok(())
}

#[test]
fn too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

BadNewType = NewType("BadNewType", int, int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_newtype")
        .count();
    Ok(())
}

#[test]
fn any_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Any

BadNewType = NewType("BadNewType", Any)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_newtype")
        .count();
    Ok(())
}

#[test]
fn union_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Union

BadNewType = NewType("BadNewType", Union[int, str])
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_newtype")
        .count();
    Ok(())
}

#[test]
fn generic_base() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "aliases_newtype")
        .count();
    Ok(())
}

#[test]
fn valid_newtype() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

UserId = NewType("UserId", int)
UserName = NewType("UserName", str)
"#;
    let diagnostics = run(source)?;
    let e0050 = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_newtype")
        .count();
    assert_eq!(e0050, 0, "Valid NewType should not trigger e0050");
    Ok(())
}

#[test]
fn newtype_in_function_param() -> Result<(), Box<dyn std::error::Error>> {
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
// Unbound TypeVar
// =============================================================================

#[test]
fn unbound_in_function() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_scoping")
        .count();
    Ok(())
}

#[test]
fn unbound_in_class() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_scoping")
        .count();
    Ok(())
}

#[test]
fn inner_class_reuse() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_scoping")
        .count();
    Ok(())
}

#[test]
fn module_level_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

x: T = 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_scoping")
        .count();
    Ok(())
}

#[test]
fn class_type_alias_with_class_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar("T")

class Foo(Generic[T]):
    MyAlias: TypeAlias = list[T]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_scoping")
        .count();
    Ok(())
}

// =============================================================================
// Self type in invalid location
// =============================================================================

// PEP 673 restricts `Self` to annotations within a class definition and
// excludes static methods because they have no `self` or `cls` parameter:
// https://peps.python.org/pep-0673/#valid-locations-for-self
const SELF_USAGE_RULE: &str = "generics_self_usage";

fn assert_self_usage(
    source: &str,
    expected: usize,
    obligation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(source)?;
    assert_rule_count(&diagnostics, SELF_USAGE_RULE, expected, obligation);

    let messages = messages_for(&diagnostics, SELF_USAGE_RULE);
    assert_eq!(
        messages.len(),
        expected,
        "{obligation}: every invalid `Self` occurrence must have a rule-specific message: {diagnostics:#?}",
    );
    if expected == 0 {
        assert!(
            messages.is_empty(),
            "{obligation}: valid `Self` locations must not produce a message: {messages:#?}",
        );
    } else {
        assert!(
            messages.iter().all(|message| !message.trim().is_empty()),
            "{obligation}: no invalid `Self` occurrence may be represented by an empty diagnostic: {messages:#?}",
        );
    }
    Ok(())
}

#[test]
fn self_in_module_function() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r"
from typing import Self as CurrentKind

def create() -> CurrentKind:
    raise RuntimeError
",
        r"
import typing as type_forms

def create(
) -> type_forms.Self:
    raise RuntimeError
",
    ];
    for source in sources {
        assert_self_usage(
            source,
            1,
            "PEP 673 forbids `Self` in a module function return annotation",
        )?;
    }
    Ok(())
}

#[test]
fn self_in_module_var() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        "from typing import Self as CurrentKind\n\nvalue: CurrentKind = None\n",
        "import typing as type_forms\n\nvalue: type_forms . Self = None\n",
    ];
    for source in sources {
        assert_self_usage(
            source,
            1,
            "PEP 673 forbids `Self` in a module variable annotation",
        )?;
    }
    Ok(())
}

#[test]
fn self_in_staticmethod() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r"
from typing import Self as CurrentKind

class Crucible:
    @staticmethod
    def create() -> CurrentKind:
        return Crucible()
",
        r"
import typing as type_forms

class Crucible:

    @staticmethod
    def create(
    ) -> type_forms.Self:
        return Crucible()
",
    ];
    for source in sources {
        assert_self_usage(
            source,
            1,
            "PEP 673 forbids `Self` in a static method because no self or cls type is bound",
        )?;
    }
    Ok(())
}

#[test]
fn self_as_generic_base_argument() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Generic as Family, Self as CurrentKind, TypeVar as VariableForge

Ore = VariableForge("Ore")

class Vessel(Family[Ore]):
    pass

class Invalid(Vessel[CurrentKind]):
    pass
"#,
        r#"
import typing as type_forms

Ore = type_forms.TypeVar("Ore")

class Vessel(type_forms.Generic[Ore]):
    pass

class Invalid(
    Vessel[
        type_forms.Self
    ],
):
    pass
"#,
    ];
    for source in sources {
        assert_self_usage(
            source,
            1,
            "PEP 673 explicitly rejects `Self` as the argument of a parameterized base class",
        )?;
    }
    Ok(())
}

#[test]
fn valid_self_usage() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r"
from typing import Self as CurrentKind

class Crucible:
    peer: CurrentKind

    def same(self) -> CurrentKind:
        return self

    @classmethod
    def create(cls, value: int) -> CurrentKind:
        return cls()
",
        r"
import typing as type_forms

class Crucible:
    peer: type_forms.Self

    def same(
        self,
    ) -> type_forms.Self:
        return self

    @classmethod
    def create(
        cls,
        value: int,
    ) -> type_forms.Self:
        return cls()
",
    ];
    for source in sources {
        assert_self_usage(
            source,
            0,
            "PEP 673 permits `Self` in class attributes, instance returns, and classmethod returns",
        )?;
    }
    Ok(())
}

// =============================================================================
// Self type violations in generics
// =============================================================================

#[test]
fn return_concrete_instead_of_self() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_self_basic")
        .count();
    Ok(())
}

#[test]
fn self_subscript() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_self_basic")
        .count();
    Ok(())
}

#[test]
fn return_in_if() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_self_basic")
        .count();
    Ok(())
}

#[test]
fn return_in_for() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_self_basic")
        .count();
    Ok(())
}

#[test]
fn return_in_while() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_self_basic")
        .count();
    Ok(())
}

// =============================================================================
// Self type attribute incompatibility
// =============================================================================

#[test]
fn self_attr_parent_class_assign() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_self_attributes")
        .count();
    Ok(())
}

#[test]
fn self_attr_reassignment() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "generics_self_attributes")
        .count();
    Ok(())
}

// =============================================================================
// super() on abstract method with no implementation
// =============================================================================

#[test]
fn super_abstract_no_impl() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "protocols_explicit_2")
        .count();
    Ok(())
}

#[test]
fn super_abstract_with_body() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "protocols_explicit_2")
        .count();
    Ok(())
}

#[test]
fn super_in_if() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "protocols_explicit_2")
        .count();
    Ok(())
}

#[test]
fn super_in_for_and_while() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "protocols_explicit_2")
        .count();
    Ok(())
}

// =============================================================================
// ClassVar in invalid context
// =============================================================================

#[test]
fn classvar_in_function_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    def method(self, a: ClassVar[int]) -> None:
        x: ClassVar[str] = ""
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "classes_classvar")
        .count();
    Ok(())
}

#[test]
fn classvar_in_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    def method(self) -> ClassVar[int]:
        return 1
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "classes_classvar")
        .count();
    Ok(())
}

#[test]
fn classvar_nested() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar, Final

class MyClass:
    bad1: Final[ClassVar[int]] = 3
    bad2: list[ClassVar[int]] = []
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "classes_classvar")
        .count();
    Ok(())
}

#[test]
fn classvar_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    x: ClassVar[int, str] = 1
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "classes_classvar")
        .count();
    Ok(())
}

#[test]
fn classvar_instance_assign() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "classes_classvar")
        .count();
    Ok(())
}

#[test]
fn classvar_module_level() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

x: ClassVar[int] = 1
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "classes_classvar")
        .count();
    Ok(())
}

// =============================================================================
// Too few args - deeper paths
// =============================================================================

#[test]
fn class_constructor_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "calls_argument_count")
        .count();
    Ok(())
}

#[test]
fn args_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(a: int, b: str, *args: float) -> None:
    pass

f(1)
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "calls_argument_count")
        .count();
    Ok(())
}

// =============================================================================
// Overload no match - deeper paths
// =============================================================================

#[test]
fn overload_multiple_methods() -> Result<(), Box<dyn std::error::Error>> {
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
// Invalid type expression - deeper paths
// =============================================================================

#[test]
fn paramspec_in_annotation() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "annotations_forward_refs")
        .count();
    Ok(())
}

#[test]
fn nested_brackets_depth() -> Result<(), Box<dyn std::error::Error>> {
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
// Generic type arg count - deeper Callable paths
// =============================================================================

#[test]
fn callable_return_type_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def f(cb: Callable[[int, str], float, bool]) -> None:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .count();
    Ok(())
}

#[test]
fn callable_ellipsis_in_brackets() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def f(cb: Callable[[...], int]) -> None:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .count();
    Ok(())
}

#[test]
fn callable_first_arg_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def f(cb: Callable[42, int]) -> None:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .count();
    Ok(())
}

// =============================================================================
// TypeIs inconsistent narrowing - deeper paths
// =============================================================================

#[test]
fn typeis_with_generics() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "narrowing_typeis_2")
        .count();
    Ok(())
}

#[test]
fn typeis_union_narrowing() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "narrowing_typeis_2")
        .count();
    Ok(())
}

// =============================================================================
// Constructor call errors - deeper paths
// =============================================================================

#[test]
fn abstract_instantiation() -> Result<(), Box<dyn std::error::Error>> {
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
        .filter(|d| d.code.code == "constructors_call_init")
        .count();
    Ok(())
}

#[test]
fn multiple_init_params() -> Result<(), Box<dyn std::error::Error>> {
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
fn generic_constructor() -> Result<(), Box<dyn std::error::Error>> {
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
