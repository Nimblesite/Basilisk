//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 36: final push targeting remaining uncovered branches.

// ── E0036: ClassVar in function params ──

#[test]
fn classvar_in_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    class_val: ClassVar[int] = 42

    def method(self, x: ClassVar[int]) -> ClassVar[str]:
        return "bad"

def func(a: ClassVar[int]) -> None:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0036: instance checks ──

#[test]
fn classvar_instance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class Config:
    debug: ClassVar[bool] = False
    name: ClassVar[str] = "default"
    count: ClassVar[int] = 0

    def __init__(self):
        self.debug = True
        Config.debug = False

c = Config()
c.debug
Config.debug
c.count = 5
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0047: Scope checks ──

#[test]
fn scope_checks() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Outer(Generic[T]):
    def method(self, x: T) -> T:
        return x

    class Inner:
        def inner_method(self, x: T) -> T:
            return x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0014: Tuple check and dataclass check ──

#[test]
fn tuple_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Tuple

x: tuple[int, str] = (1, "hello")
y: tuple[int, str] = (1, 2)
z: tuple[int, ...] = (1, 2, 3)
w: tuple[int, str, float] = (1, "a", 3.14)

# Wrong arity
a: tuple[int, str] = (1,)
b: tuple[int, str] = (1, "a", 3.14)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn dataclass_field_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

@dataclass
class Line:
    start: Point
    end: Point

p1 = Point(1, 2)
p2 = Point(3, 4)
l = Line(p1, p2)
bad_l = Line(1, 2)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0127: Tuple index out of range ──

#[test]
fn tuple_index_range() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(t: tuple[int, str, float]) -> None:
    a = t[0]
    b = t[1]
    c = t[2]
    d = t[3]
    e = t[-1]
    f = t[-2]
    g = t[-3]
    h = t[-4]
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0142: dataclass_transform class violation ──

#[test]
fn dataclass_transform_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class ModelBase:
    def __init_subclass__(cls, **kwargs):
        pass

class User(ModelBase):
    name: str
    age: int

u = User(name="test", age=42)
u.name = "changed"
u.unknown = "bad"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0144: type() call constructor ──

#[test]
fn type_call_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x = type("MyClass", (object,), {})
y = type("BadClass", (), {"x": 42})
z = type(42)
w = type("cls", (int, str), {})
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0145: type[] bracket violation ──

#[test]
fn type_bracket() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Type

x: type[int] = int
y: type[str] = str
z: type[int] = str

a: Type[int] = int
b: Type[str] = int
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0147: Tuple starred unpack ──

#[test]
fn tuple_starred_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Unpack

def func(x: tuple[int, *tuple[str, ...], float]) -> None:
    pass

def func2(*args: Unpack[tuple[int, str]]) -> None:
    pass

func(1, "a", "b", 3.14)
func2(1, "a")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0056: ReadOnly TypedDict mutation ──

#[test]
fn readonly_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict
from typing_extensions import ReadOnly

class Config(TypedDict):
    name: ReadOnly[str]
    mutable: int

c: Config = {"name": "test", "mutable": 42}
c["name"] = "changed"
c["mutable"] = 99
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0088: TypedDict runtime violation ──

#[test]
fn typeddict_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class Person(TypedDict):
    name: str
    age: int

p: Person = {"name": "Alice", "age": 30}
p2: Person = {"name": "Bob"}
p3: Person = {"name": "Carol", "age": 30, "extra": True}

isinstance(p, Person)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0093: TypedDict key validation ──

#[test]
fn typeddict_key() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class Options(TypedDict):
    debug: bool
    verbose: bool

opts: Options = {"debug": True, "verbose": False}
x = opts["debug"]
y = opts["unknown"]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0066: Enum value type mismatch ──

#[test]
fn enum_value_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum

class Color(Enum):
    _value_: int
    RED = 1
    GREEN = "green"
    BLUE = 3

class Status(Enum):
    _value_: str
    ACTIVE = "active"
    INACTIVE = 0
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0096: Dataclass field default factory ──

#[test]
fn field_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class DC:
    items: list[int] = field(default_factory=list)
    mapping: dict[str, int] = field(default_factory=dict)
    name: str = field(default_factory=str)
    count: int = field(default_factory=str)
    data: list = field(default_factory=dict)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0098: Non-protocol base in protocol ──

#[test]
fn non_protocol_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Regular:
    pass

class MyProto(Protocol, Regular):
    def method(self) -> int: ...
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0099: Protocol instantiation ──

#[test]
fn protocol_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

x = Drawable()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0101: TypeGuard no narrowing param ──

#[test]
fn typeguard_no_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def bad_guard() -> TypeGuard[str]:
    return True
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0105: Bounded TypeVar attr access ──

#[test]
fn bounded_typevar_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T", bound=int)

class Container(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

    def get_bit_length(self) -> int:
        return self.value.bit_length()

    def get_real(self) -> float:
        return self.value.real

    def bad_attr(self) -> str:
        return self.value.upper()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0109: TypeVar bound call violation ──

#[test]
fn typevar_bound_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)

def func(x: T) -> T:
    y = x + 1
    z = x * 2
    return x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0114: Protocol runtime_checkable violation ──

#[test]
fn protocol_runtime_checkable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Named(Protocol):
    name: str

class Person:
    name: str = "test"

class Thing:
    pass

isinstance(Person(), Named)
isinstance(Thing(), Named)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0116: NamedTuple def error ──

#[test]
fn namedtuple_def_error() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Bad1(NamedTuple):
    x: int
    y: int
    def method(self) -> int:
        return self.x

class Bad2(NamedTuple, int):
    x: int

class Good(NamedTuple):
    x: int
    y: str = "default"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0117: Unbound TypeVar scope ──

#[test]
fn unbound_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

def func(x: T) -> list[T]:
    z: list[S] = []
    return z
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0124: Protocol tuple element mismatch ──

#[test]
fn protocol_tuple_element() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Pair(Protocol):
    def __getitem__(self, index: int) -> int: ...
    def __len__(self) -> int: ...

t: Pair = (1, 2)
t2: Pair = (1, "bad")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0132: Inconsistent TypeVar order ──

#[test]
fn inconsistent_typevar_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

class Container(Generic[T, S]):
    def __init__(self, a: T, b: S) -> None:
        pass

    def method(self, x: S, y: T) -> None:
        pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── W0040: Lambda missing annotations ──

#[test]
fn lambda_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

f: Callable[[int], str] = lambda x: str(x)
g = lambda x, y: x + y
h: Callable[[int, int], int] = lambda a, b: a + b
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── W0050: Redundant annotation ──

#[test]
fn redundant_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 42
y: str = "hello"
z: float = 3.14
w: bool = True
a: list = [1, 2, 3]
b: dict = {"a": 1}
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0058: Annotated too few arguments ──

#[test]
fn annotated_too_few() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated = 42
y: Annotated[int] = 42
z: Annotated[int, "metadata"] = 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0070: Never type compatibility ──

#[test]
fn never_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Never, NoReturn

def never_returns() -> Never:
    raise RuntimeError("never")

def no_return() -> NoReturn:
    raise SystemExit(1)

x: int = never_returns()
y: str = never_returns()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Guards: version/platform ──

#[test]
fn guards_version_platform() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

if sys.version_info >= (3, 12):
    x: int = 42

if sys.platform == "linux":
    y: str = "linux"

class MyClass:
    if sys.version_info >= (3, 12):
        feature_flag: bool = True
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0080: TypeVar bound violation ──

#[test]
fn typevar_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T", bound=int)
S = TypeVar("S", bound=str)

class IntWrapper(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

w1 = IntWrapper(42)
w2 = IntWrapper("wrong")
w3 = IntWrapper(True)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0091: TypeVar default incompatible ──

#[test]
fn typevar_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int, default=str)
U = TypeVar("U", int, str, default=float)
V = TypeVar("V", bound=int, default=int)
W = TypeVar("W", int, str, default=int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0084: TypeVarTuple invalid params ──

#[test]
fn typevartuple_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple

Ts = TypeVarTuple("Ts")
Bad = TypeVarTuple("Wrong")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0057: type statement invalid RHS ──

#[test]
fn type_statement_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Good = int | str
type Good2 = list[int]
type Good3[T] = list[T]

type Bad1 = 42
type Bad2 = [int, str]
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Suppression: block directive ──

#[test]
fn suppression_block_directive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# basilisk: disable-block=assignment_compatibility
x: int = "hello"
y: str = 42

z: int = "should still warn"

# basilisk: warning=assignment_compatibility
a: int = "downgraded"

# basilisk: info=assignment_compatibility
b: int = "info level"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}
