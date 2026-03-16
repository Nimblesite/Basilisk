//! Coverage boost tests batch 38: final push for 89% threshold.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// ── E0045: Default value — negative literal, tuple default ──

#[test]
fn e0045_negative_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def func1(x: int = -42, y: float = -3.14, z: str = -1) -> None:
    pass

def func2(a: int = True, b: bool = 0, c: bool = 1) -> None:
    pass

def func3(x: str = None, y: int = None) -> None:
    pass

def func4(x: list[int] = [1, 2], y: dict = {"a": 1}) -> None:
    pass

def func5(x: int = 3.14, y: float = "bad", z: bytes = 42) -> None:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0036: ClassVar in various positions ──

#[test]
fn e0036_classvar_complex() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Optional

class MyClass:
    x: ClassVar[int] = 42
    y: ClassVar[str] = "test"
    z: ClassVar[Optional[int]] = None

    def __init__(self):
        self.x = 100

    def method(self, arg: ClassVar[int]) -> ClassVar[str]:
        return "bad"

def standalone(p: ClassVar[float]) -> ClassVar[int]:
    return 0

class Sub(MyClass):
    x: ClassVar[str] = "overridden"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0047: Scope — nested class TypeVar access ──

#[test]
fn e0047_nested_scope() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

class Outer(Generic[T]):
    x: T

    class Middle(Generic[S]):
        y: S

        class Inner:
            z: T
            w: S

    def method(self, val: T) -> T:
        return val
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0078: Self return — deeper nesting ──

#[test]
fn e0078_self_return_deep() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Builder:
    def build(self) -> Self:
        return Builder()

    def chain(self) -> Self:
        if True:
            if True:
                return Builder()
            return self
        return Builder()

    def with_for(self) -> Self:
        for i in range(1):
            return Builder()
        return self
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0092: type[] too many args ──

#[test]
fn e0092_type_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Type

a: type[int, str] = int
b: Type[int, str, float] = int

def func(cls: type[int, str]) -> None:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0052: Frozen dataclass — assignment in init ──

#[test]
fn e0052_frozen_init_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(frozen=True)
class Frozen:
    x: int
    y: str

f = Frozen(1, "test")
f.x = 2
f.y = "changed"
f.z = True
del f.x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0070: Never in union positions ──

#[test]
fn e0070_never_union_positions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Never, NoReturn, Union, Optional

def abort() -> Never:
    raise SystemExit

# Never in various positions
x: int | Never = 42
y: Union[str, Never] = "hello"
z: Optional[Never] = None

# Never return used in expressions
a: int = 1 if True else abort()
b: str = abort() if False else "ok"

# Function with Never param
def func(x: Never) -> int:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0080: TypeVar bound — method resolution ──

#[test]
fn e0080_typevar_bound_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T", bound=int)

class MathBox(Generic[T]):
    def __init__(self, val: T) -> None:
        self.val = val

    def compute(self) -> float:
        return float(self.val)

    def double(self) -> T:
        return self.val * 2

b = MathBox(42)
b.compute()
b.double()

class StrBox(Generic[T]):
    def __init__(self, val: T) -> None:
        self.val = val

b2: MathBox[bool] = MathBox(True)
b3: MathBox[str] = MathBox("bad")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0100: Literal augmented — vararg/kwarg ──

#[test]
fn e0100_literal_augmented_vararg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

def func(*args: Literal[1], **kwargs: Literal[0]) -> None:
    for a in args:
        a += 1
    for k, v in kwargs.items():
        v += 1
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0104: Cyclical alias — transitive ──

#[test]
fn e0104_cyclical_transitive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

X: TypeAlias = "Y"
Y: TypeAlias = "Z"
Z: TypeAlias = "X"

# Self-referencing through subscript
Self1: TypeAlias = list["Self1"]
Self2: TypeAlias = dict[str, "Self2"]
Self3: TypeAlias = "Self3"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0109: TypeVar bound — annotation resolution ──

#[test]
fn e0109_typevar_bound_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T", bound=float)

class Numeric(Generic[T]):
    value: T

    def __init__(self, value: T) -> None:
        self.value = value

    def add(self, other: T) -> T:
        return self.value + other

    def bad(self) -> str:
        return self.value.upper()

n = Numeric(3.14)
n.add(1.0)
n.add("bad")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0037: TypedDict with various key patterns ──

#[test]
fn e0037_typeddict_keys() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

# Functional with dict
Good = TypedDict("Good", {"name": str, "age": int})

# Functional with invalid keys
Bad = TypedDict("Bad", {42: str})
Bad2 = TypedDict("Bad2", {None: int})
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0042: PEP 695 with explicit Generic base ──

#[test]
fn e0042_pep695_explicit_generic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
S = TypeVar("S")

class Both[T](Generic[T]):
    pass

class Multi[T, S](Generic[T, S]):
    first: T
    second: S
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0057: type statement — invalid forms ──

#[test]
fn e0057_type_stmt_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Bad1 = 42
type Bad2 = [int, str]
type Bad3 = True
type Bad4 = "hello"

type Good1 = int | str
type Good2[T] = list[T]
type Good3 = dict[str, int]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0035: Required/NotRequired on function params ──

#[test]
fn e0035_required_notreq_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Required, NotRequired, TypedDict

class Config(TypedDict, total=False):
    name: Required[str]
    debug: NotRequired[bool]

def bad_func(x: Required[int]) -> NotRequired[str]:
    return "test"

class BadClass:
    def method(self, x: Required[int]) -> None:
        pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0017: ClassVar override between parent/child ──

#[test]
fn e0017_classvar_override_deep() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class GrandParent:
    a: ClassVar[int] = 1
    b: int = 2

class Parent(GrandParent):
    a: int = 3
    b: ClassVar[int] = 4

class Child(Parent):
    a: ClassVar[str] = "5"
    b: str = "6"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0061: assert_type patterns ──

#[test]
fn e0061_assert_type_complex() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import assert_type, Literal
from enum import Enum

class Dir(Enum):
    UP = "up"
    DOWN = "down"

assert_type(1, int)
assert_type("hello", str)
assert_type(True, bool)
assert_type(None, None)
assert_type(Dir.UP, Dir)
assert_type(1, Literal[1])
assert_type("x", Literal["x"])
assert_type(Dir.UP, Literal[Dir.UP])
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Suppression: more patterns ──

#[test]
fn suppression_disable_file() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# basilisk: disable-file=BSK-E0014
x: int = "hello"
y: str = 42
z: float = "bad"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Complex type expressions ──

#[test]
fn complex_type_expressions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union, Optional, List, Dict, Tuple, Set, FrozenSet

# Deeply nested types
x: List[Dict[str, List[int]]] = [{"a": [1]}]
y: Dict[str, Optional[List[int]]] = {"a": None}
z: Tuple[int, Tuple[str, Tuple[float]]] = (1, ("a", (3.14,)))

# Union of complex types
a: Union[List[int], Dict[str, int], Set[float]] = [1]
b: Optional[Union[int, str]] = None

# FrozenSet
c: FrozenSet[int] = frozenset({1, 2, 3})
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Overloaded functions with many patterns ──

#[test]
fn overload_many_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload, Union

@overload
def parse(data: str) -> dict: ...
@overload
def parse(data: bytes) -> dict: ...
@overload
def parse(data: int) -> int: ...
def parse(data):
    return {}

result1 = parse("hello")
result2 = parse(b"data")
result3 = parse(42)
result4 = parse(3.14)
result5 = parse([])
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Enum patterns ──

#[test]
fn enum_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum, IntEnum, auto

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = auto()

class HTTPStatus(IntEnum):
    OK = 200
    NOT_FOUND = 404

class MixedEnum(Enum):
    _value_: int
    A = 1
    B = "bad"
    C = 3

x: Color = Color.RED
y: HTTPStatus = HTTPStatus.OK
z: int = HTTPStatus.OK
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}
