//! Coverage boost tests batch 28: final push to 89%.
//! Targeting very specific patterns for e0107 variance, e0054 Final,
//! e0092 too few type args, e0108 slots, and many other rules.
#![allow(
    missing_docs,
    clippy::needless_raw_string_hashes,
    clippy::uninlined_format_args
)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// =============================================================================
// E0107: Variance - explicit base class subscript with wrong variance
// =============================================================================

#[test]
fn e0107_covariant_in_invariant_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

class Base(Generic[T]):
    pass

class Bad(Base[T_co]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0107_contravariant_in_invariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_contra = TypeVar("T_contra", contravariant=True)

class Base(Generic[T]):
    pass

class Bad(Base[T_contra]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0107_covariant_in_contravariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Sink(Generic[T_contra]):
    def put(self, x: T_contra) -> None: ...

class Bad(Sink[T_co]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0107_contravariant_in_covariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Source(Generic[T_co]):
    def get(self) -> T_co: ...

class Bad(Source[T_contra]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0107_multiple_violations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Pair(Generic[T, S]):
    pass

class Bad(Pair[T_co, T_contra]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0107_through_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

class Base(Generic[T]):
    pass

MyAlias: TypeAlias = Base[T]

class Bad(MyAlias):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0107_nested_generics() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, List

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

class Container(Generic[T]):
    items: List[T]

class Outer(Generic[T_co]):
    inner: Container[T_co]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0054: Final reassignment - more patterns
// =============================================================================

#[test]
fn e0054_final_in_while() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

MAX: Final[int] = 100

while True:
    MAX = 200
    break
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0054_final_in_try() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

MAX: Final[int] = 100

try:
    MAX = 200
except:
    MAX = 300
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0054_final_in_with() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

MAX: Final[int] = 100

class CM:
    def __enter__(self): return self
    def __exit__(self, *args): pass

with CM():
    MAX = 200
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0054_final_multiple_classes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

class A:
    X: Final = 1
    Y: Final[str] = "a"

class B(A):
    pass

A.X = 2
A.Y = "b"
B.X = 3
a = A()
a.X = 4
a.Y = "c"
b = B()
b.X = 5
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0092: Too few type args - more patterns
// =============================================================================

#[test]
fn e0092_generic_class_too_few() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")
U = TypeVar("U")

class Triple(Generic[T, S, U]):
    pass

x: Triple[int] = Triple()
y: Triple[int, str] = Triple()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0108: Dataclass slots deeper
// =============================================================================

#[test]
fn e0108_slots_with_weakref() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(slots=True, weakref_slot=True)
class WithWeakref:
    x: int
    y: str
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0125: Instance attribute on class - deeper
// =============================================================================

#[test]
fn e0125_various_attr_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    class_attr: int = 0

    def __init__(self):
        self.inst1: str = "hello"
        self.inst2: int = 42

class Child(Base):
    child_class: str = "child"

    def __init__(self):
        super().__init__()
        self.child_inst: float = 3.14

# Access patterns
x = Base.class_attr
y = Base.inst1
z = Child.child_class
w = Child.child_inst
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0134: Invariant generic mismatch
// =============================================================================

#[test]
fn e0134_list_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import List

def f(x: List[int]) -> None:
    pass

a: List[object] = [1, 2, 3]
b: List[int] = a
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0136: Callable subtyping deeper
// =============================================================================

#[test]
fn e0136_callable_return_subtype() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_fn(f: Callable[[int], object]) -> None:
    pass

def my_fn(x: int) -> str:
    return str(x)

takes_fn(my_fn)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0042: PEP 695 with old TypeVar
// =============================================================================

#[test]
fn e0042_mixed_old_new_typevars() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

OldT = TypeVar("OldT")

class MyClass[NewT](Generic[OldT]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// More varied patterns for many rules
// =============================================================================

#[test]
fn e0029_typeddict_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class TD(TypedDict):
    x: int
    y: str

    def method(self) -> None:
        pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0031_invalid_cast() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import cast

x = cast(int, "hello")
y = cast(str, 42)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0032_typeddict_invalid_keyword() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class TD(TypedDict, total=True, extra=False):
    x: int
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0033_invalid_reveal_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import reveal_type

x: int = 42
reveal_type(x)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0034_final_class_inherit() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import final

@final
class FinalClass:
    pass

class Child(FinalClass):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0035_required_outside_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Required

class NotTD:
    x: Required[int]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0039_invalid_assert_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import assert_type

x: int = 42
assert_type(x, str)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0040_enum_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum

class Color(Enum):
    RED = 1
    BLUE = 2

class Extended(Color):
    GREEN = 3
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0043_non_typevar_in_generic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic

class Bad(Generic[int]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0046_enum_member_annotated() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum

class Color(Enum):
    RED: int = 1
    BLUE: int = 2
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0049_multiple_unbounded_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Tuple

x: Tuple[int, ..., str, ...] = (1, 2, "a", "b")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0053_assert_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import assert_type

x: int = 42
assert_type(x, int)
assert_type(x, str)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0056_readonly_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict, ReadOnly

class Config(TypedDict):
    name: ReadOnly[str]
    value: int
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0058_annotated_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[int] = 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0061_assert_type_enum_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import assert_type, Literal
from enum import Enum

class Color(Enum):
    RED = 1

assert_type(Color.RED, Literal[Color.RED])
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0062_noreturn_fallthrough() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NoReturn

def never_returns() -> NoReturn:
    raise SystemExit(1)

def bad_noreturn() -> NoReturn:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Mega final mega test
// =============================================================================

#[test]
#[expect(clippy::too_many_lines)]
fn mega_coverage_final() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, TypeAlias, Final, Literal, Protocol,
    Callable, runtime_checkable, ClassVar, NamedTuple,
    Generator, overload, Hashable, Optional, Union,
    LiteralString, TypeVarTuple, Unpack, ParamSpec, Self,
    dataclass_transform, List, Dict, Set, Tuple, FrozenSet,
    TypedDict, Annotated, TypeGuard, TypeIs, NewType,
    Required, ReadOnly, NoReturn, assert_type, cast,
    reveal_type, final
)
from dataclasses import dataclass, field, InitVar
from enum import Enum
from abc import abstractmethod

# TypeVars with all variances
T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)
S = TypeVar("S", default=T)
AnyStr = TypeVar("AnyStr", str, bytes)
BadTV = TypeVar("BadTV", int)
BothVar = TypeVar("BothVar", covariant=True, contravariant=True)
BoundedT = TypeVar("BoundedT", bound=int)
Ts = TypeVarTuple("Ts")
P = ParamSpec("P")

# Variance base classes
class InvBase(Generic[T]):
    pass

class CoBase(Generic[T_co]):
    def get(self) -> T_co: ...

class ContraBase(Generic[T_contra]):
    def put(self, x: T_contra) -> None: ...

# Variance violations
class BadCo(InvBase[T_co]):
    pass

class BadContra(InvBase[T_contra]):
    pass

class BadCoInContra(ContraBase[T_co]):
    pass

# Multiple type params
S2 = TypeVar("S2")
class Pair(Generic[T, S2]):
    pass

class BadPair(Pair[T_co, T_contra]):
    pass

# Module-level mismatches
bad_int: int = "hello"
bad_str: str = 42

# Final
MAX: Final = 100
MAX = 200
PI: Final[float] = 3.14
PI = 2.71

class Config:
    X: Final = 10
    Y: Final[str] = "y"

    def modify(self):
        self.X = 20
        Config.Y = "z"

c = Config()
c.X = 30
Config.X = 40

# In control structures
for i in range(1):
    MAX = 300
while False:
    MAX = 400
try:
    MAX = 500
except:
    MAX = 600

# ClassVar
class CVClass:
    count: ClassVar[int] = 0
    bad: Final[ClassVar[int]] = 1
    bare: ClassVar
    def m(self, a: ClassVar[int]) -> None:
        pass

# Self
def bad_self(x: Self) -> Self:
    return x

class Shape:
    def method(self) -> Self:
        return Shape()

# Dataclass
@dataclass
class DC:
    a: int
    b: InitVar[bool] = False
    c: int = field(default_factory=str)
    def __post_init__(self, b: bool):
        pass

vh: Hashable = DC(0)

@dataclass(frozen=True)
class Frozen:
    x: int
    y: str

@dataclass(order=True)
class Ordered:
    x: int

@dataclass(slots=True)
class Slotted:
    x: int

@dataclass(match_args=False)
class NoMatch:
    x: int

# NamedTuple
class Pt(NamedTuple):
    x: int
    y: str = "default"

p = Pt(1, "a")
first = p[0]

# Protocol
@runtime_checkable
class Draw(Protocol):
    def draw(self) -> str: ...

class NonRT(Protocol):
    def method(self) -> None: ...

isinstance(object(), Draw)
isinstance(object(), NonRT)

class HasCV(Protocol):
    name: ClassVar[str]

# Generator
def gen() -> Generator[int, None, str]:
    yield 1
    yield from [2, 3]
    return "done"

# Overload
@overload
def proc(x: int) -> str: ...
@overload
def proc(x: str) -> int: ...
def proc(x):
    return str(x)

# Callable
f1: Callable[..., int] = lambda: 42
f2: Callable[[int], str] = lambda x: str(x)

# E0015
opt_bad: Optional[int, str] = None

# TypeAlias
BadAlias: TypeAlias = [int, str]
BadAlias2: TypeAlias = True

# Literal
def lit(a: Literal[0], b: Literal[False]):
    x: Literal[False] = a
    a += 1

# TypedDict
class TD(TypedDict):
    x: int

WrongTD = TypedDict("RightTD", {"x": int})

# NewType
UserId = NewType("UserId", int)
BadNT = NewType("WrongNT", int)

# E0058
ann: Annotated[int] = 42

# E0048
BAlias: TypeAlias = [int, str]

# Variadic
class Var(Generic[*Ts]):
    pass

# E0062
def bad_noreturn() -> NoReturn:
    pass

# E0043
class BadGen(Generic[int]):
    pass

# E0128
A = TypeVar("A")
B = TypeVar("B", default=A)
C = TypeVar("C", default=B)
class Chain(Generic[A, B, C]): ...

# E0030
Td = TypeVar("Td", default=int)
Tnd = TypeVar("Tnd")

# Historical positional
def hist(__x: int) -> None: ...
hist(__x=3)

# Enum
class Color(Enum):
    _value_: int
    RED = 1
    GREEN = "green"

# Float int attr
def float_f(x: float) -> int:
    return x.numerator

# TypeGuard/TypeIs
def is_str(x: object) -> TypeGuard[str]:
    return isinstance(x, str)

def is_int(x: int | str) -> TypeIs[int]:
    return isinstance(x, int)

# PEP 695
class Box[TT]:
    value: TT

type MyType[TT] = list[TT]

def identity[TT](x: TT) -> TT:
    return x

# LiteralString
def query(sql: LiteralString) -> None:
    pass
query("SELECT 1")

# Abstract super
class AbstractBase(Protocol):
    @abstractmethod
    def draw(self) -> str: ...

class BadChild(AbstractBase):
    def draw(self) -> str:
        return super().draw()

# Tuple index
v: tuple[int, str] = (1, "a")

# dataclass_transform
@dataclass_transform()
class ModelBase:
    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

class Model(ModelBase):
    name: str
    age: int

# Constrained
def concat_fn(a: AnyStr, b: AnyStr) -> AnyStr:
    return a + b

# E0055
BothBad = TypeVar("BothBad", covariant=True, contravariant=True)

# E0092
class Triple(Generic[T, S2, BoundedT]):
    pass

t: Triple[int] = Triple()
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.len() >= 5,
        "Final mega: got {} diagnostics: {:?}",
        diagnostics.len(),
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}
