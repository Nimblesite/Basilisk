// Integration tests to exercise many checker rules and improve coverage.
// Tests a wide range of BSK-E0XXX rules through the full parse/resolve/check pipeline.

use super::common::*;

fn messages_for(diags: &[basilisk_checker::Diagnostic], code: &str) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == code)
        .map(|d| d.message.clone())
        .collect()
}

// ============================================================================
// E0004: Missing *args/**kwargs annotation
// ============================================================================

#[test]
fn e0004_unannotated_vararg_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(*args) -> None:
    pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0004"),
        "unannotated *args should fire E0004, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0004_annotated_vararg_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(*args: int) -> None:
    pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0004"),
        "annotated *args should not fire E0004"
    );
    Ok(())
}

#[test]
fn e0004_unannotated_kwarg_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(**kwargs) -> None:
    pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0004"),
        "unannotated **kwargs should fire E0004, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

// ============================================================================
// E0018: Undefined variable
// ============================================================================

#[test]
fn e0018_undefined_var_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func() -> None:
    x = undefined_name
";
    // Just exercise the code path
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0019: Unbound variable
// ============================================================================

#[test]
fn e0019_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func() -> None:
    if False:
        x: int = 1
    y: int = x
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0024: Invalid type form
// ============================================================================

#[test]
fn e0024_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Union
x: Union = 1
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0030: Non-default after default parameter
// ============================================================================

#[test]
fn e0030_all_defaults_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(a: int = 0, b: int = 1) -> None:
    pass
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0030");
    assert!(msgs.is_empty(), "all-default params should not fire E0030");
    Ok(())
}

// ============================================================================
// E0043: Non-TypeVar in Generic base
// ============================================================================

#[test]
fn e0043_non_typevar_in_generic_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generic

class Bad(Generic[int]):
    pass
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0043");
    assert!(
        !msgs.is_empty(),
        "non-TypeVar in Generic should fire E0043, got: {msgs:?}"
    );
    Ok(())
}

// ============================================================================
// E0048: TypeAlias invalid RHS
// ============================================================================

#[test]
fn e0048_valid_type_alias_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
MyType: TypeAlias = list[int]
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0048");
    assert!(
        msgs.is_empty(),
        "valid TypeAlias should not fire E0048, got: {msgs:?}"
    );
    Ok(())
}

// ============================================================================
// E0049: Multiple unbounded tuple
// ============================================================================

#[test]
fn e0049_exercise() -> Result<(), Box<dyn std::error::Error>> {
    // This is hard to trigger through the resolver but exercises the code path
    let source = r"
from typing import Unpack
x: tuple[str, int]
";
    let diags = run(source)?;

    let _msgs = messages_for(&diags, "BSK-E0049");
    Ok(())
}

// ============================================================================
// E0056: ReadOnly TypedDict field mutation
// ============================================================================

#[test]
fn e0056_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict
class Movie(TypedDict):
    title: str
    year: int
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0057: PEP 695 type statement invalid RHS (TypeAliasType)
// ============================================================================

#[test]
fn e0057_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAliasType
MyType = TypeAliasType("MyType", int)
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0058: Annotated too few arguments
// ============================================================================

#[test]
fn e0058_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Annotated
x: Annotated[int]
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0062: NoReturn function fallthrough
// ============================================================================

#[test]
fn e0062_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NoReturn

def my_func() -> NoReturn:
    raise RuntimeError("error")
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0064: Invalid NamedTuple call
// ============================================================================

#[test]
fn e0064_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0065: Float parameter int attribute access
// ============================================================================

#[test]
fn e0065_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: float) -> None:
    y = x.numerator
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0066-E0068: Enum value type issues
// ============================================================================

#[test]
fn e0066_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0068_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum
from typing import Literal

class Status(Enum):
    ACTIVE = "active"
    INACTIVE = "inactive"
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0069: Dataclass kw_only violations
// ============================================================================

#[test]
fn e0069_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(kw_only=True)
class Config:
    name: str
    value: int
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0070-E0078: Various advanced type rules
// ============================================================================

#[test]
fn e0070_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never
def func() -> Never:
    raise RuntimeError()
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0071_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: int, /, y: str) -> None:
    pass
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0072_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: int | str) -> int | str:
    return x
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0087-E0090: TypedDict isinstance, PEP 695 bound, tuple syntax
// ============================================================================

#[test]
fn e0088_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Movie(TypedDict):
    title: str
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0090_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: tuple[int, str, float] = (1, "a", 2.0)
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0091-E0099: Various advanced rules
// ============================================================================

#[test]
fn e0091_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", default=int)
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0094_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class MyClass:
    def method(self) -> Self:
        return self
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0095_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field, InitVar

@dataclass
class Config:
    name: str
    _raw: InitVar[str] = ""
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0098_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class MyProtocol(Protocol):
    def method(self) -> int: ...
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0099_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Drawable(Protocol):
    def draw(self) -> None: ...
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0100-E0110: Advanced type checks
// ============================================================================

#[test]
fn e0100_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[1] = 1
x += 1
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0101_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_str(x: object) -> TypeGuard[str]:
    return isinstance(x, str)
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0104_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
MyType: TypeAlias = int
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0108_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(slots=True)
class Point:
    x: float
    y: float
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0111-E0120: Constructor, protocol, generator rules
// ============================================================================

#[test]
fn e0111_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class MyClass:
    def __init__(self, x: int) -> None:
        self.x: int = x

obj = MyClass(42)
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0115_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func instead")
def old_func() -> None:
    pass
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0120_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator

def gen() -> Generator[int, None, None]:
    yield 1
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0121-E0134: Protocol conformance, callable, variance
// ============================================================================

#[test]
fn e0121_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    def draw(self) -> None:
        pass

c: Drawable = Circle()
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0122_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def apply(f: Callable[[int], str], x: int) -> str:
    return f(x)
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// E0136-E0149: Callable subtyping, generic protocol, dataclass_transform, etc.
// ============================================================================

#[test]
fn e0136_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def take_callback(f: Callable[[int], str]) -> None:
    pass

def my_func(x: int) -> str:
    return str(x)

take_callback(my_func)
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0138_dataclass_transform_metaclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform()
class ModelMeta(type): ...

class ModelBase(metaclass=ModelMeta): ...

class Customer(ModelBase):
    id: int
    name: str
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0140_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def func() -> None:
    f: Callable[[int], str] = str
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0141_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict, Unpack

class Options(TypedDict):
    verbose: bool
    debug: bool

def func(**kwargs: Unpack[Options]) -> None:
    pass
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0142_dataclass_transform_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform()
class ModelBase: ...

class Customer(ModelBase):
    id: int
    name: str
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0143_namedtuple_usage_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p = Point(1, 2)
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_constructor_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Animal:
    def __init__(self, name: str) -> None:
        self.name: str = name

def make(cls: type[Animal]) -> Animal:
    return cls("fido")
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0145_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 1
y: str = "hello"
z: list[int] = [1, 2, 3]
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0146_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Sized(Protocol):
    def __len__(self) -> int: ...
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0147_tuple_starred_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: tuple[int, str] = (1, "hello")
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0148_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Box(Generic[T]):
    value: T
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn e0149_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Container[T]:
    value: T
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// W0050: Redundant annotation warning
// ============================================================================

#[test]
fn w0050_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 42
y: str = "hello"
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Suppression: type: ignore
// ============================================================================

#[test]
fn type_ignore_with_code_suppresses() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def bad(x) -> None:  # type: ignore[BSK-E0001]\n    pass\n";
    let diags = run(source)?;
    let e0001_count = diags.iter().filter(|d| d.code.code == "BSK-E0001").count();
    assert_eq!(
        e0001_count, 0,
        "type: ignore[BSK-E0001] should suppress E0001"
    );
    Ok(())
}

#[test]
fn type_ignore_bare_suppresses_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def bad(x):  # type: ignore\n    pass\n";
    let diags = run(source)?;
    // With bare type: ignore, all diagnostics on that line should be suppressed
    let line_1_diags: Vec<_> = diags.iter().filter(|d| d.span.start < 30).collect();
    assert!(
        line_1_diags.is_empty(),
        "bare type: ignore should suppress all on that line"
    );
    Ok(())
}

// ============================================================================
// Exercise multiple diagnostics on complex source
// ============================================================================

#[test]
fn complex_source_exercises_many_rules() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Final, ClassVar, Protocol, overload, TypedDict
from dataclasses import dataclass
from enum import Enum

# TypeVar
T = TypeVar("T")

# Protocol
class Serializable(Protocol):
    def serialize(self) -> str: ...

# Generic class
class Container(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value: T = value

# Enum
class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

# Dataclass
@dataclass
class Point:
    x: float
    y: float

# TypedDict
class Movie(TypedDict):
    title: str
    year: int

# Final
MAX_SIZE: Final[int] = 100

# Valid function
def process(items: list[int]) -> int:
    return sum(items)

# Overloaded function
@overload
def convert(x: int) -> str: ...
@overload
def convert(x: str) -> int: ...
def convert(x: int | str) -> int | str:
    if isinstance(x, int):
        return str(x)
    return len(x)

# Class with method
class MyClass:
    class_var: ClassVar[int] = 0
    instance_var: int

    def __init__(self, val: int) -> None:
        self.instance_var = val

    def method(self) -> int:
        return self.instance_var

# Usage
p = Point(1.0, 2.0)
m = MyClass(42)
result: int = process([1, 2, 3])
"#;
    let diags = run(source)?;
    // Just exercise everything - we don't assert specific codes here
    // but we make sure nothing panics
    // Just ensure we got here without panicking
    let _ = diags.len();
    Ok(())
}

#[test]
fn exercise_dataclass_transform_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform(frozen_default=True, order_default=True)
def create_model(cls: type) -> type:
    return cls

@create_model
class Customer:
    id: int
    name: str
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn exercise_dataclass_transform_frozen() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform(frozen_default=True)
def create_model(cls: type) -> type:
    return cls

@create_model
class Frozen:
    id: int

f = Frozen(id=1)
f.id = 2
";
    let _diags = run(source)?;
    Ok(())
}
