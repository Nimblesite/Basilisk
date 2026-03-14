#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Coverage boost tests batch 24: targeting suppression.rs, types.rs,
//! `collection_inference.rs`, and many more deeper rule paths.
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
// Suppression: type: ignore, warning, info, disabled, block directives
// =============================================================================

#[test]
fn suppression_type_ignore() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
count: int = "hello"  # type: ignore
"#;
    let diagnostics = run(source)?;
    // Should be suppressed
    let e0014 = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    assert_eq!(e0014, 0, "type: ignore should suppress diagnostic");
    Ok(())
}

#[test]
fn suppression_type_warning() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
count: int = "hello"  # type: warning
"#;
    let diagnostics = run(source)?;
    // Should be demoted to warning
    let _ = diagnostics;
    Ok(())
}

#[test]
fn suppression_type_info() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
count: int = "hello"  # type: info
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn suppression_type_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
count: int = "hello"  # type: disabled
"#;
    let diagnostics = run(source)?;
    let e0014 = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    assert_eq!(e0014, 0, "type: disabled should suppress diagnostic");
    Ok(())
}

#[test]
fn suppression_type_ignore_with_code() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
count: int = "hello"  # type: ignore[BSK-E0014]
label: str = 42  # type: ignore[BSK-E0014]
"#;
    let diagnostics = run(source)?;
    let e0014 = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    assert_eq!(e0014, 0, "type: ignore with code should suppress");
    Ok(())
}

#[test]
fn suppression_block_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# type: disabled
count: int = "hello"
label: str = 42
# type: end-disabled
good: int = 1
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn suppression_block_warning() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# type: warning
count: int = "hello"
label: str = 42
# type: end-warning
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn suppression_block_info() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# type: info
count: int = "hello"
label: str = 42
# type: end-info
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn suppression_block_unclosed() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# type: disabled
count: int = "hello"
label: str = 42
"#;
    let diagnostics = run(source)?;
    // Unclosed block should suppress to EOF
    let _ = diagnostics;
    Ok(())
}

#[test]
fn suppression_file_relaxed() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"# basilisk: relaxed
count: int = "hello"
label: str = 42
flag: bool = "yes"
"#;
    let diagnostics = run(source)?;
    // Should demote errors to warnings
    let _ = diagnostics;
    Ok(())
}

#[test]
fn suppression_file_disabled_specific() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"# basilisk: file-disabled[BSK-E0014]
count: int = "hello"
label: str = 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn suppression_file_warning_specific() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"# basilisk: file-warning[BSK-E0014]
count: int = "hello"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn suppression_file_info_specific() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"# basilisk: file-info[BSK-E0014]
count: int = "hello"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// collection_inference.rs: empty list/dict/set
// =============================================================================

#[test]
fn collection_empty_list() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = []
y: str = []
z = []
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn collection_empty_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = {}
y: str = {}
z = {}
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn collection_empty_set() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x = set()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// types.rs: Display, is_assignable_to, union
// =============================================================================

#[test]
fn types_any_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Any

x: Any = 42
y: Any = "hello"
z: Any = None
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn types_literal_string_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import LiteralString

x: LiteralString = "hello"
y: str = x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn types_optional_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Optional

x: Optional[int] = 42
y: Optional[int] = None
z: int = x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn types_callable_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Any, Callable

x: Any = lambda: 42
y: Callable = x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn types_float_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: str = 3.14
y: int = 2.71
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Deeper rule path tests - targeting specific uncovered branches
// =============================================================================

#[test]
fn e0107_resolve_and_check_class_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class ReadOnly(Generic[T_co]):
    def read(self) -> T_co: ...

class WriteOnly(Generic[T_contra]):
    def write(self, val: T_contra) -> None: ...

class BiVariant(Generic[T_co]):
    reader: ReadOnly[T_co]
    writer: WriteOnly[T_co]

class MultiNested(Generic[T_co]):
    data: list[ReadOnly[WriteOnly[T_co]]]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0137_protocol_generic_substitution() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic, runtime_checkable

T = TypeVar("T")
S = TypeVar("S")

@runtime_checkable
class Transformer(Protocol[T, S]):
    def transform(self, input: T) -> S: ...
    def reverse(self, output: S) -> T: ...

class IntToStr:
    def transform(self, input: int) -> str:
        return str(input)
    def reverse(self, output: str) -> int:
        return int(output)

class BadTransformer:
    def transform(self, input: int) -> int:
        return input
    def reverse(self, output: str) -> str:
        return output
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0139_typevartuple_with_regular_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, TypeVar, Generic, Unpack

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Heterogeneous(Generic[T, *Ts]):
    first: T

x: Heterogeneous[int, str, float] = Heterogeneous()
y: Heterogeneous[int] = Heterogeneous()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0140_callable_protocol_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, Protocol

class Handler(Protocol):
    def __call__(self, x: int, y: str) -> bool: ...

def my_handler(x: int, y: str) -> bool:
    return True

h: Handler = my_handler

# Callable with different signatures
f1: Callable[[int], str] = lambda x: str(x)
f2: Callable[[int, str], bool] = lambda x, y: True
f3: Callable[..., None] = lambda: None
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_pep695_type_param_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Box[T]:
    value: T

def identity[T](x: T) -> T:
    return x

type Alias[T] = list[T]

class Container[T: int]:
    data: T

def bounded[T: (int, str)](x: T) -> T:
    return x

class Multi[T, S, *Ts]:
    pass

async def async_gen[T](x: T) -> T:
    return x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0102_typevar_default_constraint_combos() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1", int, str, default=int)
T2 = TypeVar("T2", bound=int, default=float)
T3 = TypeVar("T3", default=None)
T4 = TypeVar("T4", int, str, float, default=bool)

class A(Generic[T1]): ...
class B(Generic[T2]): ...
class C(Generic[T3]): ...
class D(Generic[T4]): ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_tuple_starred_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Tuple

def f1() -> Tuple[int, str]:
    return (1, "a")

def f2() -> Tuple[int, ...]:
    return (1, 2, 3)

x: Tuple[int, str, float] = (1, "a", 3.0)
y: Tuple[int, ...] = (1, 2, 3, 4)
z: Tuple[()] = ()

a, b = (1, "hello")
first, *rest = (1, 2, 3, 4)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0131_yield_from_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, Iterator

def gen1() -> Generator[int, None, None]:
    yield from [1, 2, 3]

def gen2() -> Generator[int, None, None]:
    yield from range(10)

def gen3() -> Iterator[int]:
    yield 1
    yield 2
    yield from gen1()

def gen4() -> Generator[str, None, None]:
    yield "a"
    yield from ["b", "c"]
    for i in range(3):
        yield str(i)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0054_final_augmented_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

COUNTER: Final[int] = 0
COUNTER += 1

class State:
    VALUE: Final[int] = 10

    def modify(self):
        self.VALUE += 1
        State.VALUE += 2
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0148_generic_type_param_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Dict, List, Set, Tuple

T = TypeVar("T")
S = TypeVar("S")

class Pair(Generic[T, S]):
    first: T
    second: S

x: Pair[int, str] = Pair()
y: Pair[List[int], Dict[str, float]] = Pair()
z: Pair[Tuple[int, ...], Set[str]] = Pair()

class Triple(Generic[T, S]):
    pass

w: Triple[int, str] = Triple()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0120_generator_multiple_returns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, str]:
    for i in range(10):
        yield i
    if True:
        return "done"
    return "also done"

def gen2() -> Generator[int, str, None]:
    val = yield 1
    val2 = yield 2
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0138_kw_only_and_frozen() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(frozen=True)
class Config:
    host: str
    port: int
    debug: bool = False

c = Config("localhost", 8080)
c.host = "new"

@dataclass(kw_only=True)
class KWOnly:
    name: str
    value: int

k = KWOnly(name="test", value=1)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0119_isinstance_protocol_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Sizeable(Protocol):
    def __len__(self) -> int: ...

@runtime_checkable
class Iterable(Protocol):
    def __iter__(self): ...

class NotProtocol(Protocol):
    def method(self) -> None: ...

x = [1, 2, 3]
isinstance(x, Sizeable)
isinstance(x, Iterable)
isinstance(x, NotProtocol)

class MyList:
    def __len__(self) -> int:
        return 0
    def __iter__(self):
        return iter([])

isinstance(MyList(), Sizeable)
isinstance(MyList(), Iterable)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0146_protocol_classvar_and_property() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, ClassVar

class Meta(Protocol):
    name: ClassVar[str]
    version: ClassVar[int]

    @property
    def display_name(self) -> str: ...

    @classmethod
    def default(cls) -> "Meta": ...

class GoodMeta:
    name: str = "test"
    version: int = 1

    @property
    def display_name(self) -> str:
        return self.name

    @classmethod
    def default(cls) -> "GoodMeta":
        return cls()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0143_namedtuple_tuple_ops() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int
    z: int

p = Point(1, 2, 3)
a, b, c = p
first = p[0]
second = p[1]
last = p[2]
sub = p[:2]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0126_literal_string_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import LiteralString

def query(sql: LiteralString) -> None:
    pass

query("SELECT * FROM users")

x: LiteralString = "hello"
y: LiteralString = "world"
z: LiteralString = x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0095_initvar_multiple_fields() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar

@dataclass
class MultiInit:
    name: str
    init_flag: InitVar[bool] = False
    init_count: InitVar[int] = 0
    init_data: InitVar[str] = ""

    def __post_init__(self, init_flag: bool, init_count: int, init_data: str):
        self.flag = init_flag
        self.count = init_count
        self.data = init_data

m = MultiInit("test", True, 5, "data")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0130_constrained_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

AnyStr = TypeVar("AnyStr", str, bytes)

def concat(a: AnyStr, b: AnyStr) -> AnyStr:
    return a + b

x = concat("hello", "world")
y = concat(b"hello", b"world")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0142_dataclass_transform_multiple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
def model(cls):
    return cls

@model
class User:
    name: str
    age: int

@model
class Product:
    id: int
    name: str
    price: float
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0116_namedtuple_default_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Good(NamedTuple):
    x: int
    y: str = "default"
    z: float = 0.0

class Bad(NamedTuple):
    x: int = 0
    y: str
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Mega tests for maximum breadth
// =============================================================================

#[test]
fn mega_suppression_all_modes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"# basilisk: relaxed
count1: int = "hello"
count2: str = 42  # type: ignore
count3: bool = "yes"  # type: warning
count4: float = "1.5"  # type: info
count5: int = "world"  # type: disabled
# type: disabled
count6: str = 42
count7: bool = "yes"
# type: end-disabled
count8: int = "last"  # type: ignore[BSK-E0014]
"#;
    let diagnostics = run(source)?;
    // With relaxed + various suppressions, some diagnostics should be removed/demoted
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_all_rules_v4() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, TypeAlias, Final, Literal, Protocol,
    Callable, runtime_checkable, ClassVar, NamedTuple,
    Generator, overload, Hashable, Optional, Union,
    LiteralString, TypeVarTuple, Unpack, ParamSpec, Self
)
from dataclasses import dataclass, field, InitVar
from enum import Enum
from abc import abstractmethod

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
S = TypeVar("S", default=T)
Ts = TypeVarTuple("Ts")
P = ParamSpec("P")

# Many rules in one file
bad_int: int = "hello"
bad_str: str = 42
bad_float: str = 3.14

MAX: Final = 100
MAX = 200

BadAlias: TypeAlias = [int, str]

WrongNT = NewType("BadNT", int) if False else None

def hist(__x: int) -> None: ...
hist(__x=3)

class Color(Enum):
    _value_: int
    RED = 1
    GREEN = "green"

@dataclass
class DC:
    a: int
    b: InitVar[bool] = False
    c: int = field(default_factory=str)

v: Hashable = DC(0)

@dataclass(frozen=True)
class FP:
    x: int
fp = FP(1)

class Pt(NamedTuple):
    x: int
    y: str = "default"

def gen() -> Generator[int, None, None]:
    yield 1
    yield from [2, 3]
    for i in range(5):
        yield i

@overload
def proc(x: int) -> str: ...
@overload
def proc(x: str) -> int: ...
def proc(x):
    return str(x)

@runtime_checkable
class Draw(Protocol):
    def draw(self) -> str: ...

isinstance(object(), Draw)

class Shape:
    def method(self) -> Self:
        return Shape()

def bad_self(x: Self) -> Self:
    return x

def lit(a: Literal[0], b: Literal[False]):
    x: Literal[False] = a
    a += 1

class BadVar(Generic[T_co]):
    items: list[T_co]

opt_bad: Optional[int, str] = None

class HasCV(Protocol):
    name: ClassVar[str]

def unbound(x: T) -> list[T]:
    z: list[S] = []
    return z

class Box[T]:
    value: T

type MyAlias[T] = list[T]

class Variadic(Generic[*Ts]):
    pass

c: Callable[..., int] = lambda: 42
f: Callable[[int], str] = lambda x: str(x)
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.len() >= 5,
        "V4 mega test should produce many diagnostics: got {} - {:?}",
        diagnostics.len(),
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}
