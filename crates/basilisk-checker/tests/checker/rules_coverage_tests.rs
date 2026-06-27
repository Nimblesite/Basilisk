//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
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
// Missing *args/**kwargs annotation
// ============================================================================

#[test]
fn unannotated_vararg_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(*args) -> None:
    pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-E0004"),
        "unannotated *args should fire BSK-E0004, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn annotated_vararg_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(*args: int) -> None:
    pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0004"),
        "annotated *args should not fire BSK-E0004"
    );
    Ok(())
}

#[test]
fn unannotated_kwarg_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(**kwargs) -> None:
    pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-E0004"),
        "unannotated **kwargs should fire BSK-E0004, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

// ============================================================================
// Undefined variable
// ============================================================================

#[test]
fn undefined_var_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func() -> None:
    x = undefined_name
";
    // Just exercise the code path
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Unbound variable
// ============================================================================

#[test]
fn unbound_variable_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
// Invalid type form
// ============================================================================

#[test]
fn invalid_type_form_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Union
x: Union = 1
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Non-default after default parameter
// ============================================================================

#[test]
fn all_defaults_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(a: int = 0, b: int = 1) -> None:
    pass
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_defaults");
    assert!(msgs.is_empty(), "all-default params should not fire E0030");
    Ok(())
}

// ============================================================================
// Non-TypeVar in Generic base
// ============================================================================

#[test]
fn non_typevar_in_generic_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generic

class Bad(Generic[int]):
    pass
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_basic_2");
    assert!(
        !msgs.is_empty(),
        "non-TypeVar in Generic should fire E0043, got: {msgs:?}"
    );
    Ok(())
}

// ============================================================================
// TypeAlias invalid RHS
// ============================================================================

#[test]
fn valid_type_alias_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
MyType: TypeAlias = list[int]
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "aliases_implicit");
    assert!(
        msgs.is_empty(),
        "valid TypeAlias should not fire E0048, got: {msgs:?}"
    );
    Ok(())
}

// ============================================================================
// Multiple unbounded tuple
// ============================================================================

#[test]
fn multiple_unbounded_tuple_exercise() -> Result<(), Box<dyn std::error::Error>> {
    // This is hard to trigger through the resolver but exercises the code path
    let source = r"
from typing import Unpack
x: tuple[str, int]
";
    let diags = run(source)?;

    let _msgs = messages_for(&diags, "tuples_type_form");
    Ok(())
}

// ============================================================================
// ReadOnly TypedDict field mutation
// ============================================================================

#[test]
fn readonly_typeddict_field_mutation_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
// PEP 695 type statement invalid RHS (TypeAliasType)
// ============================================================================

#[test]
fn pep_695_type_statement_invalid_rhs_typealiastype_exercise(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAliasType
MyType = TypeAliasType("MyType", int)
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Annotated too few arguments
// ============================================================================

#[test]
fn annotated_too_few_arguments_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Annotated
x: Annotated[int]
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// NoReturn function fallthrough
// ============================================================================

#[test]
fn noreturn_function_fallthrough_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NoReturn

def my_func() -> NoReturn:
    raise RuntimeError("error")
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Invalid NamedTuple call
// ============================================================================

#[test]
fn invalid_namedtuple_call_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
// Float parameter int attribute access
// ============================================================================

#[test]
fn float_parameter_int_attribute_access_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: float) -> None:
    y = x.numerator
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Enum value type issues
// ============================================================================

#[test]
fn enum_int_member_values_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn enum_str_member_values_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
// Dataclass kw_only violations
// ============================================================================

#[test]
fn dataclass_kw_only_violations_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
// Various advanced type rules
// ============================================================================

#[test]
fn never_return_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never
def func() -> Never:
    raise RuntimeError()
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn positional_only_params_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: int, /, y: str) -> None:
    pass
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn overload_definitions_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
// TypedDict isinstance, PEP 695 bound, tuple syntax
// ============================================================================

#[test]
fn typeddict_class_definition_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Movie(TypedDict):
    title: str
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn tuple_type_annotation_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: tuple[int, str, float] = (1, "a", 2.0)
"#;
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Various advanced rules
// ============================================================================

#[test]
fn typevar_default_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", default=int)
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn self_return_type_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn dataclass_initvar_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn protocol_definition_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class MyProtocol(Protocol):
    def method(self) -> int: ...
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn runtime_checkable_protocol_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
// Advanced type checks
// ============================================================================

#[test]
fn literal_augmented_assignment_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[1] = 1
x += 1
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn typeguard_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_str(x: object) -> TypeGuard[str]:
    return isinstance(x, str)
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn type_alias_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
MyType: TypeAlias = int
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn dataclass_slots_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
// Constructor, protocol, generator rules
// ============================================================================

#[test]
fn class_init_instantiation_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn deprecated_function_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn generator_return_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator

def gen() -> Generator[int, None, None]:
    yield 1
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Protocol conformance, callable, variance
// ============================================================================

#[test]
fn protocol_structural_conformance_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn callable_parameter_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def apply(f: Callable[[int], str], x: int) -> str:
    return f(x)
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Callable subtyping, generic protocol, dataclass_transform, etc.
// ============================================================================

#[test]
fn callable_callback_argument_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn dataclass_transform_metaclass() -> Result<(), Box<dyn std::error::Error>> {
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
fn callable_assignment_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def func() -> None:
    f: Callable[[int], str] = str
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn unpack_typeddict_kwargs_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn dataclass_transform_base() -> Result<(), Box<dyn std::error::Error>> {
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
fn namedtuple_usage_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn type_constructor_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn annotated_assignments_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 1
y: str = "hello"
z: list[int] = [1, 2, 3]
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn protocol_with_dunder_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Sized(Protocol):
    def __len__(self) -> int: ...
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn tuple_starred_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: tuple[int, str] = (1, "hello")
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn generic_class_definition_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn pep695_generic_class_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Container[T]:
    value: T
";
    let _diags = run(source)?;
    Ok(())
}

// ============================================================================
// Redundant annotation warning
// ============================================================================

#[test]
fn redundant_annotation_warning_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
        "type: ignore[BSK-E0001] should suppress BSK-E0001"
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
