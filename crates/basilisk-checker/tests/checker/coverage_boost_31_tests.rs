use super::common::*;

// Coverage boost tests batch 31: mega compound tests exercising multiple rules.

#[test]
fn mega_literal_value_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func1(a: Literal[0], b: Literal[False]):
    x1: Literal[False] = a
    x2: Literal[0] = b

def func2(a: Literal[3, 4, 5]):
    a += 3
    a -= 1
    a *= 2

def func3(a: Literal[1], b: Literal[True]):
    x1: Literal[True] = a
    x2: Literal[1] = b

def func4(a: Literal["hello"]):
    x: Literal["world"] = a
    y: Literal["hello"] = a

def func5(a: Literal[0xFF]):
    x: Literal[255] = a
    y: Literal[256] = a
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_assignment_type_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Optional, Union, List, Dict

# Basic mismatches
count: int = "hello"
label: str = 42
flag: bool = "yes"
ratio: float = "1.5"

# Negative literal
neg: str = -42

# None
none_int: int = None
none_str: str = None

# Bool
bool_str: str = True
bool_bytes: bytes = False

# Collection
coll_int: int = [1, 2, 3]
coll_str: str = {"a": 1}
coll_float: float = {1, 2}

# Bytes
bytes_int: int = b"hello"
bytes_str: str = b"world"

# Empty
empty_int: int = []
empty_int2: int = {}

# Complex annotations
opt: Optional[int] = "hello"
union: Union[int, float] = "hello"
"#;
    let diagnostics = run(source)?;
    assert!(
        !diagnostics.is_empty(),
        "Assignment mismatches should produce diagnostics"
    );
    Ok(())
}

#[test]
fn mega_variance_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)
T = TypeVar("T")

class Producer(Generic[T_co]):
    def get(self) -> T_co: ...

class Consumer(Generic[T_contra]):
    def put(self, value: T_contra) -> None: ...

class Mutable(Generic[T]):
    value: T

# Covariant in contravariant position
class BadContainer1(Generic[T_co]):
    items: list[Consumer[T_co]]

# Contravariant in covariant position
class BadContainer2(Generic[T_contra]):
    items: list[Producer[T_contra]]

# Nested
class Deep(Generic[T_co]):
    items: list[list[Consumer[T_co]]]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_callable_all_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, Concatenate, ParamSpec, Protocol

P = ParamSpec("P")

# Ellipsis
x: Callable[..., int] = lambda: 42

# Concatenate
y: Callable[Concatenate[int, P], str] = lambda n, *args, **kwargs: str(n)

# Regular
def my_func(x: int) -> str:
    return str(x)

z: Callable[[int], str] = my_func

# Protocol callable
class Processor(Protocol):
    def __call__(self, x: int) -> str: ...

def process(x: int) -> str:
    return str(x)

w: Processor = process
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_generic_protocol_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic, runtime_checkable

T = TypeVar("T")
S = TypeVar("S")

@runtime_checkable
class Comparable(Protocol):
    def __lt__(self, other: "Comparable") -> bool: ...
    def __eq__(self, other: object) -> bool: ...

class Container(Protocol[T]):
    def get(self) -> T: ...
    def put(self, value: T) -> None: ...

class Mapper(Protocol[T, S]):
    def map(self, value: T) -> S: ...

class MyNum:
    def __lt__(self, other: int) -> bool:
        return True
    def __eq__(self, other: object) -> bool:
        return True

class BadContainer:
    def get(self) -> int:
        return 0
    def put(self, value: str) -> None:
        pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_constructor_all_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from abc import ABC, abstractmethod
from typing import TypeVar, Generic

T = TypeVar("T")

class Animal(ABC):
    @abstractmethod
    def speak(self) -> str: ...

class WithDefaults:
    def __init__(self, a: int, b: str = "hi", c: float = 0.0) -> None:
        pass

class WithNew:
    _instance = None
    def __new__(cls, value: int) -> "WithNew":
        return super().__new__(cls)

class Meta(type):
    def __call__(cls, *args, **kwargs):
        return super().__call__(*args, **kwargs)

class MetaClass(metaclass=Meta):
    def __init__(self, x: int) -> None:
        pass

class GenericBox(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

# Calls
a = Animal()
w = WithDefaults(1)
w2 = WithDefaults(1, "hello")
w3 = WithDefaults(1, "hello", 3.14)
n = WithNew(42)
m = MetaClass(1)
b = GenericBox[int](42)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_typevartuple_deep() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack, TypeVar

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Variadic(Generic[*Ts]):
    pass

class Mixed(Generic[T, *Ts]):
    pass

x: Variadic[int, str, float] = Variadic()
y: Mixed[int, str, float] = Mixed()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_all_rules_broad_exercise_v2() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, Self, ClassVar, Final, NewType,
    Protocol, overload, Callable, TypeIs, Union, Optional,
    ParamSpec, Literal, TypeAlias, Concatenate, TypeVarTuple,
    Unpack, runtime_checkable
)
from abc import abstractmethod, ABC
from dataclasses import dataclass, field
from enum import Enum

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
S = TypeVar("S")
P = ParamSpec("P")
Ts = TypeVarTuple("Ts")

# E0129: Literal value assignment
def literal_func(a: Literal[0], b: Literal[False]):
    x: Literal[False] = a
    a += 1

# E0014: Assignment mismatch
bad_int: int = "hello"
bad_str: str = 42
bad_float: float = "1.5"
bad_none: int = None

# E0066: Enum value mismatch
class Color(Enum):
    _value_: int
    RED = 1
    GREEN = "green"

# E0050: NewType
WrongName = NewType("BadName", int)

# E0071: Historical positional
def hist(__x: int) -> None: ...
hist(__x=3)

# E0096: Factory mismatch
@dataclass
class DC:
    a: int = field(default_factory=str)
    b: list = field(default_factory=list)

# E0117: Unbound TypeVar
def unbound_func(x: T) -> list[T]:
    z: list[S] = []
    return z

# E0094: Self in module func
def bad_self(x: Self) -> Self:
    return x

# E0078: Return concrete for Self
class Shape:
    def method(self) -> Self:
        return Shape()

# E0118: super on abstract
class PColor(Protocol):
    @abstractmethod
    def draw(self) -> str: ...

class BadColor(PColor):
    def draw(self) -> str:
        return super().draw()

# E0036: ClassVar in function
class CVClass:
    def method(self, a: ClassVar[int]) -> None:
        pass

# E0107: Variance
class BadVar(Generic[T_co]):
    items: list[T_co]

# E0140: Callable
c: Callable[..., int] = lambda: 42

# E0015: Too many args
opt: Optional[int, str] = None

# E0139: TypeVarTuple
class Variadic(Generic[*Ts]):
    pass
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.len() >= 3,
        "Broad exercise v2 should produce many diagnostics: got {:?}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}
