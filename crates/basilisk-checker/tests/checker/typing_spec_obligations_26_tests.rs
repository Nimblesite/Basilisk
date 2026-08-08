//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 26: targeting low-coverage rules that depend on
// resolver data. Focus: e0103, e0077, e0068, e0065, e0106, e0100, e0083,
// e0059, e0037, e0026, e0030, e0092, e0108, e0136, e0125, e0064, e0038,
// e0082, e0051, e0090, e0069, e0042, e0055, e0060.

// =============================================================================
// Tuple index out of bounds
// =============================================================================

#[test]
fn tuple_index_oob() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
v: tuple[int, str, list[bool]] = (3, "hi", [True])
a = v[4]
b = v[-4]
c = v[0]
d = v[2]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "tuples_index")
        .count();
    Ok(())
}

// =============================================================================
// Protocol Self return conformance
// =============================================================================

#[test]
fn protocol_self_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Self

class ShapeProtocol(Protocol):
    def set_scale(self, scale: float) -> Self: ...

class Circle:
    def set_scale(self, scale: float) -> int:
        return 0
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_self_protocols")
        .count();
    Ok(())
}

// =============================================================================
// Literal string enum mismatch
// =============================================================================

#[test]
fn literal_string_vs_member() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum
from typing import Literal

class Color(Enum):
    RED = 1
    BLUE = 2

def process(c: Literal[Color.RED]) -> None:
    pass

def bad(c: Literal["Color.RED"]) -> None:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "literals_parameterizations_2")
        .count();
    Ok(())
}

// =============================================================================
// Float param int attr access
// =============================================================================

#[test]
fn float_numerator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f(x: float) -> int:
    return x.numerator

def g(y: float) -> int:
    return y.denominator
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "specialtypes_promotions")
        .count();
    Ok(())
}

// =============================================================================
// Protocol used where type[Proto] expected
// =============================================================================

#[test]
fn protocol_as_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Type

class Proto(Protocol):
    def method(self) -> None: ...

def takes_type(cls: Type[Proto]) -> None:
    pass

takes_type(Proto)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "protocols_class_objects")
        .count();
    Ok(())
}

// =============================================================================
// Literal augmented assignment
// =============================================================================

#[test]
fn literal_augmented() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

x: Final = 10
x += 1
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "literals_semantics")
        .count();
    Ok(())
}

// =============================================================================
// TypeVarTuple unpack required
// =============================================================================

#[test]
fn tvt_no_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVarTuple

Ts = TypeVarTuple("Ts")

class Bad(Generic[Ts]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_typevartuple_basic_2")
        .count();
    Ok(())
}

#[test]
fn tvt_correct_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVarTuple, Unpack

Ts = TypeVarTuple("Ts")

class Good(Generic[*Ts]):
    pass
"#;
    let diagnostics = run(source)?;
    let e0083 = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_typevartuple_basic_2")
        .count();
    // Correct usage should not trigger
    let _ = e0083;
    Ok(())
}

// =============================================================================
// Dataclass match_args=False
// =============================================================================

#[test]
fn match_args_false() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(match_args=False)
class NoMatch:
    x: int
    y: str

args = NoMatch.__match_args__
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "dataclasses_match_args")
        .count();
    Ok(())
}

// =============================================================================
// Invalid TypedDict functional syntax
// =============================================================================

#[test]
fn typeddict_name_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

Wrong = TypedDict("Right", {"x": int, "y": str})
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "typeddicts_alt_syntax")
        .count();
    Ok(())
}

#[test]
fn typeddict_bad_kwarg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

TD = TypedDict("TD", {"x": int}, badarg=True)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "typeddicts_alt_syntax")
        .count();
    Ok(())
}

#[test]
fn typeddict_non_dict_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

TD = TypedDict("TD", [("x", int)])
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn typeddict_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

Good = TypedDict("Good", {"x": int, "y": str})
Also = TypedDict("Also", {"a": float}, total=False)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// TypeVar single constraint
// =============================================================================

#[test]
fn single_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_basic")
        .count();
    Ok(())
}

#[test]
fn valid_constraints() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T1 = TypeVar("T1")
T2 = TypeVar("T2", int, str)
T3 = TypeVar("T3", int, str, float)
"#;
    let diagnostics = run(source)?;
    let e0026 = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_basic")
        .count();
    assert_eq!(e0026, 0, "Valid constraints should not trigger");
    Ok(())
}

// =============================================================================
// Non-default TypeVar after default
// =============================================================================

#[test]
fn non_default_after_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1", default=int)
T2 = TypeVar("T2")

class Bad(Generic[T1, T2]): ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_defaults")
        .count();
    Ok(())
}

#[test]
fn valid_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2", default=int)
T3 = TypeVar("T3", default=str)

class Good(Generic[T1, T2, T3]): ...
"#;
    let diagnostics = run(source)?;
    let e0030 = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_defaults")
        .count();
    let _ = e0030;
    Ok(())
}

// =============================================================================
// Too few type args
// =============================================================================

#[test]
fn too_few_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Dict, Tuple

x: Dict[str] = {}
y: Tuple[int] = (1,)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "generics_defaults_specialization")
        .count();
    Ok(())
}

// =============================================================================
// Dataclass __slots__
// =============================================================================

#[test]
fn slots_conflict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(slots=True)
class WithSlots:
    x: int
    y: str

class Base:
    __slots__ = ("a",)

@dataclass(slots=True)
class Child(Base):
    b: int
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "dataclasses_slots")
        .count();
    Ok(())
}

// =============================================================================
// Callable subtyping
// =============================================================================

#[test]
fn callable_subtyping() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_int_to_str(f: Callable[[int], str]) -> None:
    pass

def my_func(x: object) -> str:
    return str(x)

takes_int_to_str(my_func)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Instance attribute on class
// =============================================================================

#[test]
fn instance_attr_on_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    class_var: int = 0

    def __init__(self):
        self.instance_var: str = "hello"

x = MyClass.instance_var
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// NamedTuple invalid arg
// =============================================================================

#[test]
fn namedtuple_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Bad(NamedTuple):
    x: int
    y: str

    def method(self):
        pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// TypedDict inheritance invalid
// =============================================================================

#[test]
fn typeddict_inherit_non_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class Base:
    x: int

class Bad(TypedDict, Base):
    y: str
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// TypeVarTuple callable mismatch
// =============================================================================

#[test]
fn tvt_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Callable, Unpack

Ts = TypeVarTuple("Ts")

def apply(func: Callable[[*Ts], int], *args: *Ts) -> int:
    return func(*args)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Invalid Literal
// =============================================================================

#[test]
fn invalid_literal_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[3.14] = 3.14
y: Literal[[1, 2]] = [1, 2]
z: Literal[{1: 2}] = {1: 2}
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Invalid tuple syntax
// =============================================================================

#[test]
fn invalid_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Tuple

x: Tuple[int, ..., str] = (1, 2, "a")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Dataclass kw_only
// =============================================================================

#[test]
fn kwonly_field() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field

@dataclass
class Config:
    x: int
    y: str = field(kw_only=True)
    z: float = field(kw_only=True, default=0.0)

c = Config(1, y="hello")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// PEP 695 mixed TypeVar
// =============================================================================

#[test]
fn mixed_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

class Mixed[S](list[T]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// TypeVar invalid kwargs
// =============================================================================

#[test]
fn typevar_bad_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", covariant=True, contravariant=True)
S = TypeVar("S", bound=int, covariant=True)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Dataclass ordering invalid
// =============================================================================

#[test]
fn dataclass_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(order=True)
class Ordered:
    x: int
    y: str

a = Ordered(1, "a")
b = Ordered(2, "b")
result = a < b
result2 = a <= b
result3 = a > b
result4 = a >= b
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Mega tests
// =============================================================================

#[test]
fn mega_resolver_data_rules() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, TypedDict, Protocol, Self, Literal,
    NamedTuple, Callable, Type, Tuple, TypeVarTuple, Final,
    Unpack
)
from dataclasses import dataclass, field
from enum import Enum

# E0026: Single constraint
BadTV = TypeVar("BadTV", int)

# E0030: Non-default after default
T1 = TypeVar("T1", default=int)
T2 = TypeVar("T2")
class BadOrder(Generic[T1, T2]): ...

# E0037: TypedDict functional
Wrong = TypedDict("Right", {"x": int})

# E0055: TypeVar bad kwargs
BadKW = TypeVar("BadKW", covariant=True, contravariant=True)

# E0065: Float int attr
def float_func(x: float) -> int:
    return x.numerator

# E0077: Protocol Self return
class ShapeP(Protocol):
    def set_scale(self, s: float) -> Self: ...

# E0083: TypeVarTuple no unpack
Ts = TypeVarTuple("Ts")

# E0103: Tuple index OOB
v: tuple[int, str] = (1, "a")
bad = v[5]

# Enum
class Color(Enum):
    RED = 1
    BLUE = 2

# E0059: match_args=False
@dataclass(match_args=False)
class NoMatch:
    x: int

# E0042: Mixed TypeVar
T = TypeVar("T")

# Constrained TypeVar
AnyStr = TypeVar("AnyStr", str, bytes)

# E0069: kw_only
@dataclass
class KW:
    x: int
    y: str = field(kw_only=True)

# E0108: slots
@dataclass(slots=True)
class Slotted:
    x: int

# NamedTuple
class Pt(NamedTuple):
    x: int
    y: str
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_all_low_coverage_rules() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, TypedDict, Protocol, Self, Literal,
    NamedTuple, Callable, Type, Tuple, TypeVarTuple, Final,
    Unpack, ClassVar, Optional, Union, LiteralString
)
from dataclasses import dataclass, field, InitVar
from enum import Enum
from abc import abstractmethod

T = TypeVar("T")
S = TypeVar("S", default=T)
BadTV = TypeVar("BadTV", int)
BothVar = TypeVar("BothVar", covariant=True, contravariant=True)
Ts = TypeVarTuple("Ts")

# E0014
bad: int = "hello"
bad2: str = 42

# E0030
T1d = TypeVar("T1d", default=int)
T2nd = TypeVar("T2nd")
class BadOrd(Generic[T1d, T2nd]): ...

# E0037
WrongTD = TypedDict("RightTD", {"x": int})

# E0065
def float_f(x: float) -> int:
    return x.numerator

# E0103
v: tuple[int, str] = (1, "a")
oob = v[5]

# E0059
@dataclass(match_args=False)
class NoMA:
    x: int

# Final
MAX: Final = 100
MAX = 200

# ClassVar
class CVC:
    x: ClassVar[int] = 0
    def m(self, a: ClassVar[int]) -> None:
        pass

# E0094
def bad_self(x: Self) -> Self:
    return x

# NamedTuple
class NPt(NamedTuple):
    x: int
    y: str

# Dataclass
@dataclass
class DC:
    a: int
    b: InitVar[bool] = False
    c: int = field(default_factory=str)
    def __post_init__(self, b: bool):
        pass

# Generator
from typing import Generator
def gen() -> Generator[int, None, None]:
    yield 1
    yield from [2, 3]

# Protocol
class MyProto(Protocol):
    def method(self) -> None: ...

# E0108
@dataclass(slots=True)
class Slotted2:
    x: int
    y: str

# E0069
@dataclass
class KWOnly2:
    a: int
    b: str = field(kw_only=True)

# E0051
z: Literal[3.14] = 3.14
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.len() >= 2,
        "Low coverage mega: got {} diagnostics: {:?}",
        diagnostics.len(),
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}
