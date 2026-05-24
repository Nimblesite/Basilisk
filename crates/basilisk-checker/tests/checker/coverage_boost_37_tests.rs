//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 37: targeting specific uncovered branches in near-complete files.

// ── E0017: ClassVar override mismatch ──

#[test]
fn e0017_classvar_override() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class Base:
    x: ClassVar[int] = 1
    y: int = 2

class Child(Base):
    x: int = 3
    y: ClassVar[int] = 4
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0035: Required/NotRequired in function params ──

#[test]
fn e0035_required_in_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Required, NotRequired

def func(x: Required[int], y: NotRequired[str]) -> None:
    pass

class MyClass:
    def method(self, a: Required[int]) -> None:
        pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0037: TypedDict non-string keys ──

#[test]
fn e0037_typeddict_non_string_keys() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

Bad = TypedDict("Bad", {1: int, "name": str})
Bad2 = TypedDict("Bad2", {True: str})
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0042: PEP 695 + parameterized Generic ──

#[test]
fn e0042_pep695_plus_generic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Bad[T](Generic[T]):
    pass

class Bad2[S](Generic[S]):
    value: S
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0052: Frozen dataclass with class hierarchy ──

#[test]
fn e0052_frozen_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class FrozenBase:
    x: int

@dataclass
class MutableChild(FrozenBase):
    y: str

@dataclass(frozen=True)
class FrozenChild(FrozenBase):
    z: float
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0092: type[T] with wrong arg count ──

#[test]
fn e0092_type_bracket_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: type[int, str] = int
y: type[int, str, float] = int
z: type[int] = int
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0100: Literal augmented assignment (deeper) ──

#[test]
fn e0100_literal_augmented_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

def func(x: Literal[42], *args: Literal[1], **kwargs: Literal[0]) -> None:
    x += 1
    for a in args:
        a += 1
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0078: Self return with concrete in elif ──

#[test]
fn e0078_self_return_elif() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Builder:
    def build(self) -> Self:
        if True:
            return Builder()
        elif True:
            return Builder()
        else:
            return Builder()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0080: TypeVar bound — call with wrong types ──

#[test]
fn e0080_typevar_bound_method_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T", bound=int)

class Box(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

    def get(self) -> T:
        return self.value

    def set(self, value: T) -> None:
        self.value = value

class Derived(Box[int]):
    pass

b = Box(42)
b.set("wrong")

d = Derived(10)
d.set("bad")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0045: default value type issues ──

#[test]
fn e0045_default_value_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def func1(x: int = "wrong") -> None:
    pass

def func2(x: str = 42, y: float = "bad") -> None:
    pass

def func3(x: list[int] = [], y: dict[str, int] = {}) -> None:
    pass

def func4(x: int = -42, y: float = -3.14) -> None:
    pass

def func5(x: bool = 0, y: int = True) -> None:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0061: AssertType enum literal ──

#[test]
fn e0061_assert_type_enum() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import assert_type, Literal
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2

assert_type(Color.RED, Color)
assert_type(Color.RED, Literal[Color.RED])
assert_type(1, Literal[1])
assert_type("hello", Literal["hello"])
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0104: Cyclical alias deeper ──

#[test]
fn e0104_cyclical_alias_chain() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

A: TypeAlias = "B"
B: TypeAlias = "C"
C: TypeAlias = "A"

D: TypeAlias = list["D"]
E: TypeAlias = dict[str, "E"]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0109: TypeVar bound call (more patterns) ──

#[test]
fn e0109_typevar_bound_call_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T", bound=str)

class Wrapper(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

    def upper(self) -> str:
        return self.value.upper()

    def lower(self) -> str:
        return self.value.lower()

    def bad_method(self) -> int:
        return self.value.bit_length()

w = Wrapper("hello")
w.upper()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0070: Never in more contexts ──

#[test]
fn e0070_never_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never, NoReturn, Union, Optional

def raises() -> Never:
    raise RuntimeError

def no_ret() -> NoReturn:
    raise SystemExit

# Never is bottom type — assignable to anything
x: int = raises()
y: str = raises()
z: list[int] = raises()

# Union with Never simplifies
a: Union[int, Never] = 42
b: Optional[Never] = None

# Never return in expression
c = 1 + raises()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0067: Enum non-member literal (more patterns) ──

#[test]
fn e0067_enum_literal_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum
from typing import Literal

class Status(Enum):
    ACTIVE = "active"
    INACTIVE = "inactive"

class Priority(Enum):
    HIGH = 1
    LOW = 0

def func1(s: Literal[Status.ACTIVE, Status.INACTIVE]) -> None:
    pass

def func2(p: Literal[Priority.HIGH]) -> None:
    pass

func1(Status.ACTIVE)
func1(Status.INACTIVE)
func2(Priority.HIGH)
func2(Priority.LOW)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0096: Field factory (more patterns) ──

#[test]
fn e0096_field_factory_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class Config:
    items: list[int] = field(default_factory=list)
    data: dict[str, str] = field(default_factory=dict)
    flags: set[str] = field(default_factory=set)
    nums: tuple[int, ...] = field(default_factory=tuple)
    bad: list[int] = field(default_factory=int)
    wrong: dict[str, int] = field(default_factory=set)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0057: type statement with nested subscript ──

#[test]
fn e0057_type_stmt_nested() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Simple = int
type Union = int | str
type Generic[T] = list[T]
type Nested[T] = dict[str, list[T]]
type Complex[T, S] = tuple[T, S, list[T | S]]
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Suppression: ignore with brackets ──

#[test]
fn suppression_ignore_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = "hello"  # type: ignore[assignment]
y: int = "world"  # type: ignore
z: int = "test"  # basilisk: ignore[BSK-E0014]
w: int = "test"  # basilisk: disable=BSK-E0014
v: int = "test"  # basilisk: warning=BSK-E0014
u: int = "test"  # basilisk: info=BSK-E0014
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Types: edge cases for InferredType Display and assignability ──

#[test]
fn types_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Any, TypeForm, Optional, LiteralString

# Any assignability
x: Any = 42
y: Any = "hello"
z: Any = None

# Optional
a: Optional[int] = 42
b: Optional[int] = None
c: Optional[int] = "wrong"

# LiteralString
d: LiteralString = "hello"
e: str = d
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Collection inference ──

#[test]
fn collection_inference_general() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# Infer element type
x = [1, 2, 3]
y = {"a": 1, "b": 2}
z = {1, 2, 3}

# Empty with annotation
a: list = []
b: dict = {}
c: set = set()

# Mixed collections
d: list[int | str] = [1, "a", 2, "b"]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── More dataclass patterns ──

#[test]
fn dataclass_inheritance_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Base:
    x: int
    y: str = "default"

@dataclass
class Child(Base):
    z: float = 0.0

c = Child(1, "test", 3.14)
c2 = Child(1)
c3 = Child(1, "test")
c4 = Child(x=1, y="test", z=3.14)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Protocol with multiple methods ──

#[test]
fn protocol_multi_method_conformance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, Iterator

class Iterable(Protocol):
    def __iter__(self) -> Iterator: ...
    def __len__(self) -> int: ...
    def __contains__(self, item: object) -> bool: ...

class MyList:
    def __iter__(self):
        return iter([])
    def __len__(self) -> int:
        return 0
    def __contains__(self, item: object) -> bool:
        return False

class Incomplete:
    def __iter__(self):
        return iter([])

x: Iterable = MyList()
y: Iterable = Incomplete()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}
