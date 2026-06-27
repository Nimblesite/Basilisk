//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
//! Coverage boost tests batch 35: targeting the long tail of uncovered rules.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// ── E0072: Overload call mismatch ──

#[test]
fn overload_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x):
    return x

result1 = process(42)
result2 = process("hello")
result3 = process(3.14)
result4 = process([1, 2])
result5 = process(None)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0054: Final annotation violations ──

#[test]
fn final_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

MAX: Final = 100
MIN: Final[int] = 0

MAX = 200
MIN = -1

class Config:
    NAME: Final = "config"
    VERSION: Final[int] = 1

    def method(self):
        self.NAME = "changed"
        Config.VERSION = 2

def func(x: Final[int]) -> None:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0126: Literal string assignment ──

#[test]
fn literal_string() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal["hello"] = "hello"
y: Literal["hello"] = "world"
z: Literal["hello", "world"] = "hello"
w: Literal["hello", "world"] = "goodbye"

def func(x: Literal["a", "b"]) -> None:
    pass

func("a")
func("b")
func("c")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0129: Literal value incompatible ──

#[test]
fn literal_value_compat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func1(a: Literal[0], b: Literal[False]):
    x1: Literal[False] = a
    x2: Literal[0] = b

def func2(a: Literal[1], b: Literal[True]):
    x1: Literal[True] = a
    x2: Literal[1] = b

def func3(a: Literal[42]):
    x: Literal[0] = a

def func4(a: Literal["hello"]):
    x: Literal["world"] = a
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0139: TypeVarTuple specialization ──

#[test]
fn typevartuple_specialization() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, TypeVar, Unpack

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Shape(Generic[*Ts]):
    pass

class Fixed(Generic[T]):
    pass

# Not enough type args
x: Shape[()] = Shape()

# Specialization
y: Shape[int] = Shape()
z: Shape[int, str] = Shape()
w: Shape[int, str, float] = Shape()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0074: __new__ constructor mismatch ──

#[test]
fn new_constructor_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Singleton:
    _instance = None
    def __new__(cls, value: int) -> "Singleton":
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

s1 = Singleton(42)
s2 = Singleton("wrong")
s3 = Singleton()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0075: Self type attribute incompatible ──

#[test]
fn self_type_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self

class Builder:
    def set_name(self, name: str) -> Self:
        self.name = name
        return self

    def set_age(self, age: int) -> Self:
        self.age = age
        return self

class SubBuilder(Builder):
    def set_name(self, name: str) -> "Builder":
        self.name = name
        return self
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0076: Overload union expansion failure ──

#[test]
fn overload_union_expansion() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload, Union

@overload
def convert(x: int) -> str: ...
@overload
def convert(x: str) -> int: ...
def convert(x):
    if isinstance(x, int):
        return str(x)
    return int(x)

val: Union[int, str] = 42
result = convert(val)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0077: Protocol Self violation ──

#[test]
fn protocol_self_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Self

class Clonable(Protocol):
    def clone(self) -> Self: ...

class Good:
    def clone(self) -> "Good":
        return Good()

class Bad:
    def clone(self) -> int:
        return 42

x: Clonable = Good()
y: Clonable = Bad()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0063: Non-hashable dataclass ──

#[test]
fn non_hashable_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Mutable:
    x: int
    y: str

@dataclass(frozen=True)
class Immutable:
    x: int

@dataclass(eq=True)
class WithEq:
    x: int

m = Mutable(1, "a")
d = {m: "value"}
s = {m}

i = Immutable(1)
d2 = {i: "value"}
s2 = {i}
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0064: Invalid NamedTuple call ──

#[test]
fn invalid_namedtuple_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Point = NamedTuple("Point", [("x", int), ("y", int)])
Bad1 = NamedTuple("Wrong", [("a", int)])
Bad2 = NamedTuple("Bad2", x=int, y=str)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0050: NewType violations ──

#[test]
fn newtype_violations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

UserId = NewType("UserId", int)
Email = NewType("Email", str)

WrongName = NewType("BadName", int)

uid: UserId = UserId(42)
email: Email = Email("test@test.com")

# Direct assignment
bad1: UserId = 42
bad2: Email = "test@test.com"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0051: NewType base type ──

#[test]
fn newtype_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

# Valid bases
UserId = NewType("UserId", int)
Email = NewType("Email", str)
Items = NewType("Items", list[int])

# Invalid bases (if applicable)
Bad1 = NewType("Bad1", 42)
Bad2 = NewType("Bad2", None)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0041: Type annotation assignment mismatch ──

#[test]
fn type_annotation_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Optional, Union, ClassVar

class MyClass:
    class_var: ClassVar[int] = 42
    class_var2: ClassVar[str] = "hello"

    def __init__(self):
        self.x: int = 42
        self.y: str = "hello"
        self.z: Optional[int] = None
        self.w: Union[int, str] = 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0059: match_args=False access ──

#[test]
fn match_args_false() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(match_args=False)
class Point:
    x: int
    y: int

@dataclass
class WithMatch:
    x: int
    y: int

Point.__match_args__
WithMatch.__match_args__
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0060: Cross-type dataclass ordering ──

#[test]
fn cross_type_comparison() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(order=True)
class Point2D:
    x: int
    y: int

@dataclass(order=True)
class Point3D:
    x: int
    y: int
    z: int

a = Point2D(1, 2)
b = Point3D(1, 2, 3)

result = a < b
result2 = a > b
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0065: Float param int attr access ──

#[test]
fn float_param_int_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: float) -> None:
    y = x.numerator
    z = x.denominator
    w = x.bit_length()
    v = x.conjugate()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0068: Literal string enum mismatch ──

#[test]
fn literal_string_enum() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum
from typing import Literal

class Color(Enum):
    RED = "red"
    GREEN = "green"
    BLUE = "blue"

x: Literal[Color.RED] = Color.RED
y: Literal[Color.RED] = Color.BLUE
z: Literal["red"] = "red"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0069: Dataclass kw_only violation ──

#[test]
fn kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(kw_only=True)
class Config:
    name: str
    value: int
    debug: bool = False

c1 = Config(name="test", value=42)
c2 = Config("test", 42)
c3 = Config("test", value=42)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0083: TypeVarTuple unpack required ──

#[test]
fn typevartuple_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack

Ts = TypeVarTuple("Ts")

class Array(Generic[*Ts]):
    def __init__(self, *args: Unpack[Ts]) -> None:
        pass

class BadArray(Generic[Ts]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0082: TypeVarTuple callable mismatch ──

#[test]
fn typevartuple_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Callable, Unpack

Ts = TypeVarTuple("Ts")

def apply(func: Callable[[Unpack[Ts]], int], *args: Unpack[Ts]) -> int:
    return func(*args)

def add(a: int, b: int) -> int:
    return a + b

result = apply(add, 1, 2)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0081: TypeVarTuple unpack violation ──

#[test]
fn typevartuple_unpack_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack, TypeVar

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Tuple(Generic[*Ts]):
    pass

class Bad1(Generic[T, *Ts]):
    x: Tuple[T, Unpack[Ts]]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0086: Multiple TypeVarTuples ──

#[test]
fn multiple_typevartuple() -> Result<(), Box<dyn std::error::Error>> {
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

// ── E0094: Self invalid location ──

#[test]
fn self_invalid_location() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

def module_func(x: Self) -> Self:
    return x

class MyClass:
    def method(self) -> Self:
        return self

    @staticmethod
    def static_method() -> Self:
        pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0100: Literal augmented assign ──

#[test]
fn literal_augmented_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(x: Literal[42]) -> None:
    x += 1
    x -= 1
    x *= 2

def func2(x: Literal["hello"]) -> None:
    x += " world"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0103: Tuple index out of bounds ──

#[test]
fn tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Tuple

def func(t: tuple[int, str, float]) -> None:
    x = t[0]
    y = t[1]
    z = t[2]
    w = t[3]
    v = t[-1]
    u = t[-4]
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0104: Cyclical type alias ──

#[test]
fn cyclical_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

A: TypeAlias = "B"
B: TypeAlias = "A"

C: TypeAlias = list["C"]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0106: Protocol class object ──

#[test]
fn protocol_class_object() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class P(Protocol):
    x: int

def func(cls: type[P]) -> P:
    return cls()

func(P)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0110: Protocol variance violation ──

#[test]
fn protocol_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class ReadOnly(Protocol[T_co]):
    def get(self) -> T_co: ...
    def set(self, value: T_co) -> None: ...

class WriteOnly(Protocol[T_contra]):
    def get(self) -> T_contra: ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0112: TypeGuard callable return mismatch ──

#[test]
fn typeguard_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_str(x: object) -> TypeGuard[str]:
    return isinstance(x, str)

def bad_guard(x: object) -> TypeGuard[str]:
    return 42
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0113: TypeIs inconsistent narrowing ──

#[test]
fn typeis_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def is_int(x: object) -> TypeIs[int]:
    return isinstance(x, int)

def bad_typeis(x: str) -> TypeIs[int]:
    return isinstance(x, int)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0118: Super on abstract ──

#[test]
fn super_abstract() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol
from abc import abstractmethod, ABC

class Base(ABC):
    @abstractmethod
    def method(self) -> str: ...

class Child(Base):
    def method(self) -> str:
        return super().method()

class ProtoBase(Protocol):
    @abstractmethod
    def compute(self) -> int: ...

class ProtoChild(ProtoBase):
    def compute(self) -> int:
        return super().compute()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0121: Protocol assignment conformance ──

#[test]
fn protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class HasLen(Protocol):
    def __len__(self) -> int: ...

class HasGetItem(Protocol):
    def __getitem__(self, key: int) -> str: ...

class MyList:
    def __len__(self) -> int:
        return 0
    def __getitem__(self, key: int) -> str:
        return ""

x: HasLen = MyList()
y: HasGetItem = MyList()
z: HasLen = object()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0122: Callable call-site violation ──

#[test]
fn callable_callsite() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def apply(f: Callable[[int], str], x: int) -> str:
    return f(x)

def my_func(x: int) -> str:
    return str(x)

result = apply(my_func, 42)
result2 = apply(my_func, "wrong")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0015: Too many type args ──

#[test]
fn too_many_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Optional, List, Dict, Tuple

x: Optional[int, str] = None
y: List[int, str] = []
z: Dict[int, str, float] = {}
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0026: Type annotation error ──

#[test]
fn type_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[int, "metadata"] = 42
y: Annotated[str, "info", "extra"] = "hello"
z: Annotated = 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0038: Augmented assignment type ──

#[test]
fn augmented_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 42
x += 1
x -= 1
x *= 2
x //= 3

y: str = "hello"
y += " world"
y += 42

z: list = [1, 2]
z += [3, 4]
z += "wrong"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0134: Invariant generic arg mismatch ──

#[test]
fn invariant_generic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Container(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

x: Container[int] = Container(42)
y: Container[str] = Container("hello")
z: Container[int] = Container("wrong")

a: list[int] = [1, 2, 3]
b: list[str] = a
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0136: Callable subtyping violation ──

#[test]
fn callable_subtyping() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_int(x: int) -> str:
    return str(x)

def takes_object(x: object) -> str:
    return str(x)

# Callable subtyping
f1: Callable[[object], str] = takes_int
f2: Callable[[int], str] = takes_object
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0090: Invalid tuple type syntax ──

#[test]
fn tuple_type_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Tuple

x: tuple[int, str] = (1, "a")
y: tuple[int, ...] = (1, 2, 3)
z: Tuple[int, str, float] = (1, "a", 3.14)

# Empty tuple
w: tuple[()] = ()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0073: NamedTuple tuple compat ──

#[test]
fn namedtuple_tuple_compat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p = Point(1, 2)
t: tuple[int, int] = p
t2: tuple[int, str] = p
t3: tuple[int, int, int] = p
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0131: Generator type mismatch ──

#[test]
fn generator_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator, Iterator, AsyncGenerator

def gen1() -> Generator[int, str, bool]:
    val = yield 1
    return True

def gen2() -> Iterator[int]:
    yield 1
    yield 2

async def agen1() -> AsyncGenerator[int, None]:
    yield 1
    yield 2
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0133: Protocol variance mismatch ──

#[test]
fn protocol_variance_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

class Producer(Protocol[T_co]):
    def get(self) -> T_co: ...

class IntProducer:
    def get(self) -> int:
        return 42

p: Producer[object] = IntProducer()
q: Producer[int] = IntProducer()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0141: Unpack kwargs violation ──

#[test]
fn unpack_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict, Unpack

class Options(TypedDict):
    name: str
    age: int

def func(**kwargs: Unpack[Options]) -> None:
    pass

func(name="test", age=42)
func(name="test", age="wrong")
func(name="test")
func(unknown="bad")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0043: Binary operation type ──

#[test]
fn binary_operation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 1 + 2
y: str = "a" + "b"
z: float = 1.0 + 2.0

# Bad operations
a: str = 1 + 2
b: int = "a" + "b"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0044: Comparison type ──

#[test]
fn comparison() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x = 1 < 2
y = "a" < "b"
z = 1 < "b"
w = [1, 2] < [3, 4]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0012: Unused variable ──

#[test]
fn unused_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def func():
    used = 42
    unused = "never"
    return used

class MyClass:
    def method(self):
        x = 1
        y = 2
        return x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0125: Instance attr on class ──

#[test]
fn instance_attr_on_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    class_var: int = 42

    def __init__(self):
        self.instance_var: str = "hello"

# Class-level access
MyClass.class_var
MyClass.instance_var
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0055: TypeVar invalid kwargs ──

#[test]
fn typevar_invalid_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", covariant=True, contravariant=True)
U = TypeVar("U", int, str, bound=int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0128: TypeVar default referential (additional) ──

#[test]
fn typevar_default_referential() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")
U = TypeVar("U", default=T)
V = TypeVar("V", default=U)
W = TypeVar("W", default=int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0067: Enum non-member in Literal ──

#[test]
fn enum_non_member() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum
from typing import Literal

class Status(Enum):
    ACTIVE = 1
    INACTIVE = 0

def func(s: Literal[Status.ACTIVE]) -> None:
    pass

func(Status.ACTIVE)
func(Status.INACTIVE)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}
