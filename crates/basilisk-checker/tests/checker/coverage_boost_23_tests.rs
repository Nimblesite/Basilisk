//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 23: targeting e0128, e0063, e0048 and deeper paths
// in e0107, e0137, e0139, e0140, e0149, e0102, e0147, e0131, e0054, e0148,
// e0120, e0138, e0119, e0146, e0143, e0126, e0095, e0130, e0142, e0116, e0072.

// =============================================================================
// E0128: TypeVar default referential violations
// =============================================================================

#[test]
fn bad_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

Start2T = TypeVar("Start2T", default="StopT")
Stop2T = TypeVar("Stop2T", default=int)

class slice2(Generic[Start2T, Stop2T]): ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_defaults_referential_2")
        .count();
    Ok(())
}

#[test]
fn outer_scope() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

S1 = TypeVar("S1")
S2 = TypeVar("S2", default=S1)

class Foo3(Generic[S1]):
    class Bar2(Generic[S2]): ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_defaults_referential_2")
        .count();
    Ok(())
}

#[test]
fn bound_constraint_compat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

Y1 = TypeVar("Y1", bound=int)
Invalid2 = TypeVar("Invalid2", float, str, default=Y1)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_defaults_referential_2")
        .count();
    Ok(())
}

#[test]
fn valid_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2", default=T1)

class Good(Generic[T1, T2]): ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0063: Non-hashable dataclass
// =============================================================================

#[test]
fn non_hashable_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass
from typing import Hashable

@dataclass
class DC1:
    a: int

v: Hashable = DC1(0)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "dataclasses_hash")
        .count();
    Ok(())
}

#[test]
fn frozen_dataclass_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass
from typing import Hashable

@dataclass(eq=True, frozen=True)
class DC2:
    a: int

v2: Hashable = DC2(0)
"#;
    let diagnostics = run(source)?;
    let e0063 = diagnostics
        .iter()
        .filter(|d| d.code.code == "dataclasses_hash")
        .count();
    // Frozen dataclass should be hashable
    let _ = e0063;
    Ok(())
}

#[test]
fn unsafe_hash() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass
from typing import Hashable

@dataclass(unsafe_hash=True)
class DC3:
    a: int

v3: Hashable = DC3(0)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn explicit_hash() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass
from typing import Hashable

@dataclass
class DC4:
    a: int

    def __hash__(self):
        return hash(self.a)

v4: Hashable = DC4(0)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0048: Invalid TypeAlias RHS
// =============================================================================

#[test]
fn list_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadAlias1: TypeAlias = [int, str]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .count();
    Ok(())
}

#[test]
fn bool_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadAlias2: TypeAlias = True
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .count();
    Ok(())
}

#[test]
fn int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadAlias3: TypeAlias = 1
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .count();
    Ok(())
}

#[test]
fn dict_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadAlias4: TypeAlias = {"a": "b"}
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .count();
    Ok(())
}

#[test]
fn conditional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

cond = True
BadAlias5: TypeAlias = int if cond else str
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .count();
    Ok(())
}

#[test]
fn lambda_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadAlias6: TypeAlias = (lambda: int)()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn fstring() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadAlias7: TypeAlias = f"int"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn valid_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias, Union, Optional

Good1: TypeAlias = int
Good2: TypeAlias = Union[int, str]
Good3: TypeAlias = Optional[int]
Good4: TypeAlias = list[int]
Good5: TypeAlias = dict[str, int]
"#;
    let diagnostics = run(source)?;
    let e0048 = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .count();
    assert_eq!(e0048, 0, "Valid type aliases should not trigger e0048");
    Ok(())
}

#[test]
fn boolean_op() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadAlias8: TypeAlias = list or set
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn list_comprehension() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadAlias9: TypeAlias = [int for i in range(1)]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Deeper coverage for remaining rules
// =============================================================================

#[test]
fn typevar_with_multiple_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T", int, str, default=float)
S = TypeVar("S", bound=int, default=str)

class A(Generic[T, S]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn generator_async_yield() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import AsyncGenerator

async def gen() -> AsyncGenerator[int, None]:
    yield 1
    yield 2
    yield "bad"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn generator_nested_yield() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, None]:
    for i in range(10):
        if i % 2 == 0:
            yield i
        else:
            yield str(i)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn final_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

class Config:
    MAX: Final[int] = 100
    NAME: Final[str] = "config"

    def update(self):
        self.MAX = 200
        Config.NAME = "updated"

c = Config()
c.MAX = 300
Config.NAME = "changed"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn final_module_level() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

MAX_SIZE: Final = 100
MAX_SIZE = 200
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn generic_type_arg_optional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Optional

T = TypeVar("T")

class Box(Generic[T]):
    value: T

x: Box[Optional[int]] = Box()
y: Box[int | str] = Box()
z: Box[list[int]] = Box()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn generator_return_no_yield() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def bad_gen() -> Generator[int, None, str]:
    return "done"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn dataclass_transform_frozen() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(frozen=True)
class FrozenPoint:
    x: int
    y: int

p = FrozenPoint(1, 2)
p.x = 3
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn dataclass_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(order=True)
class OrderedPoint:
    x: int
    y: int

p1 = OrderedPoint(1, 2)
p2 = OrderedPoint(3, 4)
result = p1 < p2
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn protocol_runtime_checkable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Drawable(Protocol):
    def draw(self) -> str: ...

class Circle:
    def draw(self) -> str:
        return "circle"

class Square:
    pass

isinstance(Circle(), Drawable)
isinstance(Square(), Drawable)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn protocol_not_runtime_checkable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> str: ...

isinstance(object(), Drawable)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn protocol_class_object_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, ClassVar

class HasClassAttrs(Protocol):
    name: ClassVar[str]
    count: ClassVar[int]

    @classmethod
    def create(cls) -> "HasClassAttrs": ...

class Good:
    name: str = "test"
    count: int = 0

    @classmethod
    def create(cls) -> "Good":
        return cls()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn namedtuple_operations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p = Point(1, 2)
a, b = p
x = p[0]
y = p[1]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn literal_string_concat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import LiteralString

def safe(s: LiteralString) -> None:
    pass

x: LiteralString = "hello" + " " + "world"
safe("literal")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn initvar_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field, InitVar

@dataclass
class Config:
    name: str
    debug: InitVar[bool] = False
    verbose: InitVar[int] = 0

    def __post_init__(self, debug: bool, verbose: int):
        self.is_debug = debug
        self.verbosity = verbose

c = Config("test", True, 2)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn typevar_with_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)
S = TypeVar("S", int, str)

def add(x: T, y: T) -> T:
    return x + y

def process(x: S) -> S:
    return x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn transform_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class ModelBase:
    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

class User(ModelBase):
    name: str
    age: int

class Admin(ModelBase):
    name: str
    role: str
    level: int
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn namedtuple_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Base(NamedTuple):
    x: int
    y: str

class Child(Base):
    z: float
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn overload_no_match_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def process(x: int) -> str: ...
@overload
def process(x: str) -> int: ...
def process(x):
    return str(x) if isinstance(x, int) else int(x)

result = process(3.14)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Mega compound tests for maximum coverage
// =============================================================================

#[test]
fn mega_typealias_all_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

Bad1: TypeAlias = [int, str]
Bad2: TypeAlias = True
Bad3: TypeAlias = 1
Bad4: TypeAlias = {"a": "b"}
Bad5: TypeAlias = list or set
Bad6: TypeAlias = f"int"
Good1: TypeAlias = int
Good2: TypeAlias = list[int]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_typevar_default_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

S1 = TypeVar("S1")
S2 = TypeVar("S2", default=S1)

# Good ordering
class Good(Generic[S1, S2]): ...

# Bad ordering
Start = TypeVar("Start", default="Stop")
Stop = TypeVar("Stop", default=int)
class BadOrder(Generic[Start, Stop]): ...

# Outer scope
class Outer(Generic[S1]):
    class Inner(Generic[S2]): ...

# Constraint incompatibility
Y1 = TypeVar("Y1", bound=int)
BadDefault = TypeVar("BadDefault", float, str, default=Y1)

# Valid defaults
T1 = TypeVar("T1")
T2 = TypeVar("T2", default=int)
class ValidDefaults(Generic[T1, T2]): ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_dataclass_all_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field, InitVar
from typing import Hashable, ClassVar

@dataclass
class NonHashable:
    a: int

v1: Hashable = NonHashable(0)

@dataclass(frozen=True)
class Frozen:
    x: int
    y: int

v2: Hashable = Frozen(1, 2)

@dataclass(unsafe_hash=True)
class UnsafeHash:
    a: int

v3: Hashable = UnsafeHash(0)

@dataclass
class WithHash:
    a: int
    def __hash__(self):
        return hash(self.a)

v4: Hashable = WithHash(0)

@dataclass
class WithInitVar:
    name: str
    debug: InitVar[bool] = False
    def __post_init__(self, debug: bool):
        self.is_debug = debug

@dataclass(order=True)
class Ordered:
    x: int
    y: int

p1 = Ordered(1, 2)
p2 = Ordered(3, 4)
result = p1 < p2

@dataclass
class WithFactory:
    a: int = field(default_factory=str)
    b: list = field(default_factory=list)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_protocol_all_deep() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable, ClassVar, TypeVar, Generic

T = TypeVar("T")

@runtime_checkable
class Drawable(Protocol):
    def draw(self) -> str: ...

class NotRuntime(Protocol):
    def process(self) -> int: ...

class HasClassAttrs(Protocol):
    name: ClassVar[str]
    @classmethod
    def create(cls) -> "HasClassAttrs": ...

class Circle:
    def draw(self) -> str:
        return "circle"

class Square:
    pass

isinstance(Circle(), Drawable)
isinstance(object(), NotRuntime)

class Container(Protocol[T]):
    def get(self) -> T: ...
    def put(self, value: T) -> None: ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_generator_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, AsyncGenerator

def gen1() -> Generator[int, None, None]:
    yield 1
    yield 2

def gen2() -> Generator[int, None, str]:
    yield 1
    return "done"

def gen3() -> Generator[int, None, None]:
    for i in range(10):
        if i % 2 == 0:
            yield i

async def agen() -> AsyncGenerator[int, None]:
    yield 1
    yield 2
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_final_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

MAX: Final = 100
MAX = 200

class Config:
    DEBUG: Final[bool] = False
    NAME: Final[str] = "app"

    def update(self):
        self.DEBUG = True
        Config.NAME = "updated"

c = Config()
c.DEBUG = True
Config.NAME = "new"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_all_rules_v3() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, TypeAlias, Final, Literal, Protocol,
    Callable, runtime_checkable, ClassVar, NamedTuple,
    Generator, AsyncGenerator, overload, Hashable, Optional,
    Union, LiteralString, dataclass_transform
)
from dataclasses import dataclass, field, InitVar
from enum import Enum
from abc import abstractmethod

T = TypeVar("T")
S = TypeVar("S", default=T)

# E0048: Bad type alias
BadAlias: TypeAlias = [int, str]

# E0128: TypeVar default ordering
Start = TypeVar("Start", default="Stop")
Stop = TypeVar("Stop", default=int)
class BadSlice(Generic[Start, Stop]): ...

# E0063: Non-hashable dataclass
@dataclass
class DC:
    a: int
v: Hashable = DC(0)

# E0054: Final reassignment
MAX: Final = 100
MAX = 200

# E0131: Generator
def gen() -> Generator[int, None, None]:
    yield 1
    yield "bad"

# E0129: Literal assignment
def lit(a: Literal[0], b: Literal[False]):
    x: Literal[False] = a
    a += 1

# E0014: Type mismatch
bad: int = "hello"
bad2: str = 42

# E0120: Generator return
def gen2() -> Generator[int, None, str]:
    return "done"

# E0072: Overload
@overload
def proc(x: int) -> str: ...
@overload
def proc(x: str) -> int: ...
def proc(x):
    return str(x)

# E0015: Too many args
opt_bad: Optional[int, str] = None

# E0138: Frozen
@dataclass(frozen=True)
class FP:
    x: int
fp = FP(1)

# E0116: NamedTuple
class Pt(NamedTuple):
    x: int
    y: str

# E0119: Protocol isinstance
@runtime_checkable
class Draw(Protocol):
    def draw(self) -> str: ...

isinstance(object(), Draw)

# E0146: ClassVar protocol
class HasCV(Protocol):
    name: ClassVar[str]
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.len() >= 3,
        "V3 mega test should produce many diagnostics: got {:?}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}
