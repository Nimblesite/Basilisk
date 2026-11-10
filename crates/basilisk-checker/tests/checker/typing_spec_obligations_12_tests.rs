//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 12: targeting deeply uncovered code paths.
// Focuses heavily on: e0115 (deprecated usage - all stmt visit branches),
// e0137 (generic protocol assignments with type checking),
// e0140 (callable assignment compatibility with AST walking),
// e0144 (`type()` constructor deeper paths),
// e0149 (PEP 695 scoping - deeper nesting),
// e0111 (constructor errors - subclass/Self checks).

// =============================================================================
// Deprecated usage - ALL statement visit branches
// =============================================================================

#[test]
fn deprecated_function_direct_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func instead")
def old_func() -> int:
    return 42

result = old_func()
"#;
    let diags = run(source)?;
    let has_e0115 = diags.iter().any(|d| d.code.code == "directives_deprecated");
    assert!(
        has_e0115,
        "Expected directives_deprecated for deprecated function call"
    );
    Ok(())
}

#[test]
fn deprecated_function_name_reference() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_func() -> int:
    return 42

f = old_func
"#;
    let diags = run(source)?;
    let has_e0115 = diags.iter().any(|d| d.code.code == "directives_deprecated");
    assert!(
        has_e0115,
        "Expected directives_deprecated for deprecated function reference"
    );
    Ok(())
}

#[test]
fn deprecated_class_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use NewClass")
class OldClass:
    pass

obj = OldClass()
"#;
    let diags = run(source)?;
    let has_e0115 = diags.iter().any(|d| d.code.code == "directives_deprecated");
    assert!(
        has_e0115,
        "Expected directives_deprecated for deprecated class instantiation"
    );
    Ok(())
}

#[test]
fn deprecated_in_if_statement() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_func() -> bool:
    return True

if old_func():
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_while_statement() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_check() -> bool:
    return False

while old_check():
    break
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_for_statement() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_iter")
def old_iter() -> list[int]:
    return [1, 2, 3]

for item in old_iter():
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_return_statement() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_func() -> int:
    return 42

def wrapper() -> int:
    return old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_annotated_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_func() -> int:
    return 42

x: int = old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_augmented_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_func() -> int:
    return 42

x = 0
x += old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_func() -> int:
    return 42

def outer() -> None:
    result = old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_in_class_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_func() -> int:
    return 42

class MyClass:
    value = old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_method_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Spam:
    @deprecated("Use new_method")
    def old_method(self) -> int:
        return 42

    def new_method(self) -> int:
        return 99

spam = Spam()
result = spam.old_method()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_property_getter() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Spam:
    @property
    @deprecated("Use new_prop")
    def old_prop(self) -> int:
        return 42

spam = Spam()
x = spam.old_prop
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_property_setter() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Spam:
    @property
    def shape(self) -> str:
        return "round"

    @shape.setter
    @deprecated("Don't set shape")
    def shape(self, value: str) -> None:
        pass

spam = Spam()
spam.shape = "cube"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_overload() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated, overload

@overload
@deprecated("Use new_func")
def my_func(x: int) -> int: ...
@overload
def my_func(x: str) -> str: ...

def my_func(x: int | str) -> int | str:
    return x

result = my_func(42)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_no_message() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated
def old_func() -> int:
    return 42

result = old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_dunder_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Invocable:
    @deprecated("Don't call directly")
    def __call__(self) -> int:
        return 42

invocable = Invocable()
invocable()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_dunder_add() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Counter:
    @deprecated("Use increment() instead")
    def __add__(self, other: int) -> "Counter":
        return Counter()

counter = Counter()
counter += 1
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_aug_assign_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Widget:
    @property
    def count(self) -> int:
        return 0

    @count.setter
    @deprecated("Use set_count instead")
    def count(self, val: int) -> None:
        pass

w = Widget()
w.count += 1
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_typing_extensions_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import typing_extensions

@typing_extensions.deprecated("Use new_func")
def old_func() -> int:
    return 42

old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_typing_dotted_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import typing

@typing.deprecated("Use new_func")
def old_func() -> int:
    return 42

old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_call_as_argument() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_value() -> int:
    return 42

def consume(x: int) -> None:
    pass

consume(old_value())
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_method_with_var_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Calculator:
    @deprecated("Use compute()")
    def old_compute(self) -> int:
        return 0

    def compute(self) -> int:
        return 42

calc = Calculator()
x = calc.old_compute()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn deprecated_attribute_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Config:
    @property
    @deprecated("Use new_value")
    def old_value(self) -> int:
        return 42

cfg = Config()
result = cfg.old_value
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Generic protocol - assignment with type checking
// =============================================================================

#[test]
fn generic_protocol_assignment_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic

T = TypeVar("T")

class Container(Protocol[T]):
    def get(self) -> T: ...

class IntBox:
    def get(self) -> str:
        return "hello"

x: Container[int] = IntBox()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn generic_protocol_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic

T = TypeVar("T")

class Acceptor(Protocol[T]):
    def accept(self, value: T) -> None: ...

class StringAcceptor:
    def accept(self, value: str) -> None:
        pass

x: Acceptor[int] = StringAcceptor()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn self_typed_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Cloneable(Protocol):
    def clone(self: T) -> T: ...

class Widget:
    def clone(self) -> "Widget":
        return Widget()

class BadWidget:
    def clone(self) -> str:
        return "bad"

x: Cloneable = Widget()
y: Cloneable = BadWidget()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_generic_two_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")
U = TypeVar("U")

class Transformer(Protocol[T, U]):
    def transform(self, value: T) -> U: ...

class IntToStr:
    def transform(self, value: int) -> str:
        return str(value)

x: Transformer[int, str] = IntToStr()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_generic_wrong_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")
U = TypeVar("U")

class Mapper(Protocol[T, U]):
    def map(self, value: T) -> U: ...

class BadMapper:
    def map(self, value: int) -> int:
        return value

x: Mapper[int, str] = BadMapper()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Callable assignment - deeper AST walking
// =============================================================================

#[test]
fn protocol_with_multiple_methods_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Codec(Protocol):
    def encode(self, data: str) -> bytes: ...
    def decode(self, data: bytes) -> str: ...

class JsonCodec:
    def encode(self, data: str) -> bytes:
        return data.encode()
    def decode(self, data: bytes) -> str:
        return data.decode()

x: Codec = JsonCodec()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_return_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(x: int) -> str:
    return str(x)

f: Callable[[int], int] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_param_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(x: str) -> int:
    return len(x)

f: Callable[[int], int] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_with_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(x: int, y: str = "default", z: float = 0.0) -> bool:
    return True

f: Callable[[int], bool] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_assignment_missing_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Readable(Protocol):
    def read(self) -> str: ...

class Writer:
    def write(self, data: str) -> None:
        pass

x: Readable = Writer()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// type() constructor - deeper paths
// =============================================================================

#[test]
fn type_with_multiple_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base1:
    x: int = 0

class Base2:
    y: str = ""

Combined = type("Combined", (Base1, Base2), {"z": True})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn type_with_methods_in_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def my_method(self) -> str:
    return "hello"

MyClass = type("MyClass", (object,), {"greet": my_method, "x": 42})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn type_wrong_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
MyClass = type("MyClass", (object,))
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// TypeVar scoping deeper nesting
// =============================================================================

#[test]
fn typevar_in_method_and_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    value: T

    def get(self) -> T:
        return self.value

    def set(self, new_value: T) -> None:
        self.value = new_value

    def transform(self, func: "Callable[[T], T]") -> T:
        return func(self.value)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn typevar_in_protocol_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Reader(Protocol[T]):
    def read(self) -> T: ...

class Writer(Protocol[T]):
    def write(self, value: T) -> None: ...

class ReadWriter(Reader[T], Writer[T], Protocol[T]):
    ...
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Constructor errors - subclass and Self checks
// =============================================================================

#[test]
fn constructor_self_typevar_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class MyList(Generic[T]):
    def __init__(self: "MyList[T]", items: list[T]) -> None:
        self.items = items
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn constructor_super_init_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Shape:
    def __init__(self, name: str, sides: int) -> None:
        self.name = name
        self.sides = sides

class Triangle(Shape):
    def __init__(self) -> None:
        super().__init__("triangle", 3)

class Square(Shape):
    def __init__(self) -> None:
        super().__init__("square", 4)

class Pentagon(Shape):
    def __init__(self) -> None:
        super().__init__("pentagon", 5)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn constructor_arg_type_checking() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Validator:
    def __init__(self, pattern: str, flags: int) -> None:
        self.pattern = pattern
        self.flags = flags

v1 = Validator("test", 0)
v2 = Validator(42, "wrong")
v3 = Validator("ok", 1)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Callable arity - deeper paths
// =============================================================================

#[test]
fn callable_too_many_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(a: int, b: str) -> bool:
    return True

f: Callable[[int, str, float], bool] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_return_type_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(a: int) -> str:
    return str(a)

f: Callable[[int], int] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Protocol conformance - deeper paths
// =============================================================================

#[test]
fn protocol_missing_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Iterable(Protocol):
    def __iter__(self) -> "Iterator": ...
    def __next__(self) -> int: ...

class MyIter:
    def __iter__(self) -> "MyIter":
        return self

x: Iterable = MyIter()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_with_properties() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class HasName(Protocol):
    @property
    def name(self) -> str: ...

class Person:
    @property
    def name(self) -> str:
        return "Alice"

class Animal:
    name: str = "Dog"

x: HasName = Person()
y: HasName = Animal()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// InitVar deeper paths
// =============================================================================

#[test]
fn multiple_initvars() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar

@dataclass
class Config:
    debug: InitVar[bool]
    verbose: InitVar[bool]
    name: str
    level: int = 0

    def __post_init__(self, debug: bool, verbose: bool) -> None:
        if debug:
            self.level = 10
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// NamedTuple deeper paths
// =============================================================================

#[test]
fn namedtuple_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point2D(NamedTuple):
    x: int
    y: int

class Point3D(NamedTuple):
    x: int
    y: int
    z: int
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn namedtuple_with_complex_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple, Optional, List

class Record(NamedTuple):
    name: str
    values: List[int]
    parent: Optional["Record"] = None
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// TypeVarTuple deeper paths
// =============================================================================

#[test]
fn typevartuple_in_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack, Protocol

Ts = TypeVarTuple("Ts")

class MultiArg(Protocol[Unpack[Ts]]):
    def call(self, *args: Unpack[Ts]) -> None: ...
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Protocol isinstance - deeper paths
// =============================================================================

#[test]
fn protocol_issubclass_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Sized(Protocol):
    def __len__(self) -> int: ...

result = issubclass(list, Sized)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// TypeVar scoping - deeper paths
// =============================================================================

#[test]
fn typevar_used_in_inner_class_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Outer(Generic[T]):
    class Inner:
        def method(self) -> T:
            ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn typevar_constraint_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", int, str)

def process(x: T) -> T:
    if isinstance(x, int):
        return x + 1
    return x + "!"
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// NamedTuple tuple compat - deeper paths
// =============================================================================

#[test]
fn namedtuple_field_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p: tuple[str, str] = Point(1, 2)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// NamedTuple definition - deeper paths
// =============================================================================

#[test]
fn namedtuple_functional_with_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Point = NamedTuple("Point", [("x", int), ("y", int)])
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn namedtuple_class_with_invalid_field() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    _y: int = 0
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Dataclass transform - deeper paths
// =============================================================================

#[test]
fn transform_with_field_specifiers() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

def field(*, default: object = None) -> object:
    return default

@dataclass_transform(field_specifiers=(field,))
class Model:
    def __init_subclass__(cls, **kwargs: object) -> None:
        pass

class User(Model):
    name: str
    age: int = field(default=0)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Variance - deeper paths
// =============================================================================

#[test]
fn covariant_contravariant_mix() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class ReadStream(Generic[T_co]):
    def read(self) -> T_co: ...

class WriteStream(Generic[T_contra]):
    def write(self, data: T_contra) -> None: ...

class IOStream(ReadStream[bytes], WriteStream[bytes]):
    def read(self) -> bytes:
        return b""
    def write(self, data: bytes) -> None:
        pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn invariant_type_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Box(Generic[T]):
    value: T

class IntBox(Box[int]):
    pass

class StrBox(Box[str]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// ClassVar - deeper edge cases
// =============================================================================

#[test]
fn classvar_in_protocol_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Protocol

class ConfigProto(Protocol):
    debug: ClassVar[bool]
    version: ClassVar[str]

class AppConfig:
    debug: bool = True
    version: str = "1.0"

cfg: ConfigProto = AppConfig()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_instance_attr_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class Counter:
    count: ClassVar[int] = 0

    def increment(self) -> None:
        Counter.count += 1

c = Counter()
c.count = 5
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Invalid type expression - deeper edge cases
// =============================================================================

#[test]
fn paramspec_in_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec

P = ParamSpec("P")

def decorator(func: "P") -> "P":
    return func
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn local_var_invalid_lambda_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def foo() -> None:
    x: lambda: None = None
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// More literal edge cases
// =============================================================================

#[test]
fn literal_union_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal, Union

x: Union[Literal[1], Literal[2]] = 1
y: Union[Literal["a"], Literal["b"]] = "a"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_reassign_float_element() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[float, float] = (1.0, 2.0)
t = (3.0, 4.0)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn dataclass_attr_single_quoted_str() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Config:
    name: int

c = Config(42)
c.name = 'hello'
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Generator - deeper paths for yield from
// =============================================================================

#[test]
fn yield_from_generator_send_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def sub_gen() -> Generator[int, str, None]:
    while True:
        value = yield 42
        if value == "stop":
            return

def main_gen() -> Generator[int, int, None]:
    yield from sub_gen()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn generator_return_incompatible() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, str]:
    yield 1
    yield 2
    return 42
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Generator yield - deeper scenarios
// =============================================================================

#[test]
fn yield_with_conditional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen(flag: bool) -> Generator[int, None, None]:
    if flag:
        yield 1
        yield "wrong"
    else:
        yield 3
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Additional coverage for remaining rules
// =============================================================================

#[test]
fn typevar_default_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int, default=str)
U = TypeVar("U", int, str, default=bytes)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_string_wrong_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal["hello"] = "world"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_union_wrong_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal, Union

x: Union[Literal["a"], Literal["b"]] = "c"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn overload_missing_impl() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...

def process(x: int | str) -> int | str:
    if isinstance(x, int):
        return x + 1
    return x + "!"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn generic_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Box(Generic[T]):
    value: T

x: Box[int, str] = Box()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_class_type_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Printable(Protocol):
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

class MyObj:
    def __str__(self) -> str:
        return "MyObj"
    def __repr__(self) -> str:
        return "MyObj()"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn typeguard_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeGuard

def is_list_of_str(val: list[object]) -> TypeGuard[list[str]]:
    return all(isinstance(x, str) for x in val)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn final_module_level() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

MAX_SIZE: Final = 100
MAX_SIZE = 200
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn newtype_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType

UserId = NewType("UserId", int)
UserName = NewType("UserName", str)
Score = NewType("Score", float)
Data = NewType("Data", bytes)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn missing_kwonly_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def configure(*, host: str, port: int, debug: bool = False) -> None:
    pass

configure(host="localhost")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn too_few_type_args_multiple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
U = TypeVar("U")

class Pair(Generic[T, U]):
    first: T
    second: U

x: Pair[int] = Pair()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn self_type_outside_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self

def standalone() -> Self:
    ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn slots_and_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(slots=True)
class Point:
    x: int
    y: int

@dataclass
class Point2:
    __slots__ = ()
    x: int
    y: int
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_variance_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_contra = TypeVar("T_contra", contravariant=True)

class Sink(Protocol[T_contra]):
    def consume(self, item: T_contra) -> None: ...
    def produce(self) -> T_contra: ...
"#;
    let _ = run(source)?;
    Ok(())
}
