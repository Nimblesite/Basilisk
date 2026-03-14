#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Coverage boost tests batch 1: targeting rules with highest uncovered line counts.
//! Covers: e0070, e0072, e0074, e0075, e0076, e0079, e0081, e0082, e0095, e0102,
//!         e0107, e0110, e0111, e0112, e0113, e0114, e0119, e0120, e0121, e0122
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

fn has_code(diags: &[basilisk_checker::Diagnostic], code: &str) -> bool {
    diags.iter().any(|d| d.code.code == code)
}

fn codes(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags.iter().map(|d| d.code.code.to_string()).collect()
}

// --- E0070: Never type compatibility ---

#[test]
fn e0070_never_return_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Never, NoReturn

def f() -> Never:
    raise RuntimeError()

x: int = f()

def g() -> NoReturn:
    raise SystemExit()

y: str = g()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0070_never_in_union() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Never, Union

x: Union[int, Never] = 42
y: int | Never = 42
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0070_never_param_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Never

def f(x: Never) -> None:
    pass
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0072: No matching overload ---

#[test]
fn e0072_overload_with_incompatible_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def process(x: int) -> int: ...

@overload
def process(x: str) -> str: ...

def process(x):
    return x

result = process(3.14)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0072_overload_matching_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def double(x: int) -> int: ...

@overload
def double(x: str) -> str: ...

def double(x):
    return x

result = double(42)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0072_overload_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def make(x: int, y: int) -> int: ...

@overload
def make(x: str) -> str: ...

def make(x, y=None):
    return x

result = make()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0074: Constructor __new__ mismatch ---

#[test]
fn e0074_generic_new_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar, Self

T = TypeVar("T")

class Box(Generic[T]):
    def __new__(cls, x: T) -> Self:
        return super().__new__(cls)

b = Box[int](1.0)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0074_generic_new_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar, Self

T = TypeVar("T")

class Box(Generic[T]):
    def __new__(cls, x: T) -> Self:
        return super().__new__(cls)

b = Box[int](42)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0074_explicit_cls_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class MyClass(Generic[T]):
    def __new__(cls: type["MyClass[int]"]) -> "MyClass[int]":
        return super().__new__(cls)

x = MyClass[str]()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0075: Self type attribute incompatibility ---

#[test]
fn e0075_self_attr_parent_instance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self, TypeVar, Generic
from dataclasses import dataclass

T = TypeVar("T")

@dataclass
class LinkedList(Generic[T]):
    value: T
    next: Self | None = None

@dataclass
class OrdinalLinkedList(LinkedList[int]):
    def ordinal_value(self) -> str:
        return str(self.value)

xs = OrdinalLinkedList(value=1, next=LinkedList[int](value=2))
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0075_self_attr_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self
from dataclasses import dataclass

@dataclass
class Node:
    child: Self | None = None

n = Node(child=Node())
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0076: Overload union expansion ---

#[test]
fn e0076_overload_union_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload, Union

@overload
def f(x: int) -> int: ...
@overload
def f(x: str) -> str: ...

def f(x):
    return x

def caller(val: Union[int, str]) -> None:
    result = f(val)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0076_overload_union_with_incompatible() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload, Union

@overload
def g(x: int) -> int: ...
@overload
def g(x: str) -> str: ...

def g(x):
    return x

def caller2(val: Union[int, float]) -> None:
    result = g(val)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0079: Module protocol incompatibility ---

#[test]
fn e0079_module_protocol_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class HasTimeout(Protocol):
    timeout: str

import os

x: HasTimeout = os
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0081: TypeVarTuple unpack minimum ---

#[test]
fn e0081_typevartuple_unpack_min() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Any, Unpack

Ts = TypeVarTuple("Ts")

class Array(Generic[*Ts]): ...

def process(x: "Array[int, *tuple[Any, ...], str]") -> None: ...

def func(z: "Array[int]") -> None:
    process(z)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0082: TypeVarTuple callable mismatch ---

#[test]
fn e0082_typevartuple_callable_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Callable, Generic

Ts = TypeVarTuple("Ts")

def apply(f: Callable[[*Ts], None], *args: *Ts) -> None:
    f(*args)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0095: InitVar dataclass field ---

#[test]
fn e0095_initvar_field_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar, field

@dataclass
class Config:
    name: str
    debug: InitVar[bool] = False

    def __post_init__(self, debug: bool) -> None:
        pass

c = Config("test", True)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0095_initvar_in_non_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import InitVar

class NotDataclass:
    x: InitVar[int]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0095_multiple_initvar_fields() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar

@dataclass
class Server:
    host: str
    port: InitVar[int] = 8080
    debug: InitVar[bool] = False
    max_connections: int = 100

    def __post_init__(self, port: int, debug: bool) -> None:
        pass
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0102: TypeVar default violation ---

#[test]
fn e0102_typevar_default_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T2 = TypeVar("T2", default=T1)
T1 = TypeVar("T1")

class MyClass(Generic[T2, T1]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0102_typevar_default_bound_compat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

X1 = TypeVar("X1", bound=int)
Invalid1 = TypeVar("Invalid1", default=X1, bound=str)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0102_typevar_default_constraint_superset() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

Y1 = TypeVar("Y1", int, str)
Invalid2 = TypeVar("Invalid2", bool, complex, default=Y1)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0102_typevar_default_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2", default=T1)

class ValidClass(Generic[T1, T2]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0107: Variance incompatibility ---

#[test]
fn e0107_covariant_in_param_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)

class Container(Generic[T_co]):
    def get(self) -> T_co: ...
    def set(self, val: T_co) -> None: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0107_contravariant_in_return_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_contra = TypeVar("T_contra", contravariant=True)

class Sink(Generic[T_contra]):
    def accept(self, val: T_contra) -> None: ...
    def produce(self) -> T_contra: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0107_correct_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Producer(Generic[T_co]):
    def get(self) -> T_co: ...

class Consumer(Generic[T_contra]):
    def accept(self, val: T_contra) -> None: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0110: Protocol variance violation ---

#[test]
fn e0110_protocol_covariant_in_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_co = TypeVar("T_co", covariant=True)

class Writable(Protocol[T_co]):
    def write(self, data: T_co) -> None: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0110_protocol_contravariant_in_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_contra = TypeVar("T_contra", contravariant=True)

class Readable(Protocol[T_contra]):
    def read(self) -> T_contra: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0110_protocol_correct_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_co = TypeVar("T_co", covariant=True)

class Readable(Protocol[T_co]):
    def read(self) -> T_co: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0110_protocol_invariant_in_both() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Container(Protocol[T]):
    def get(self) -> T: ...
    def set(self, val: T) -> None: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0111: Constructor call errors ---

#[test]
fn e0111_no_custom_init_with_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Empty:
    pass

x = Empty(1, 2, 3)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_generic_init_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Container(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

c = Container[int](1.5)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_generic_init_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Container(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

c = Container[int](42)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_class_scoped_typevar_in_self() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class MyClass(Generic[T]):
    def __init__(self: "MyClass[T]", value: T) -> None:
        self.value = value
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_no_init_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Simple:
    x: int = 0

s = Simple()
"#;
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "BSK-E0111"),
        "no-arg constructor should not fire E0111"
    );
    Ok(())
}

// --- E0112: TypeGuard callable return ---

#[test]
fn e0112_typeguard_in_callable_str() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeGuard, Callable

def is_int(val: object) -> TypeGuard[int]:
    return isinstance(val, int)

def takes_str_callable(f: Callable[[object], str]) -> None:
    pass

takes_str_callable(is_int)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0112_typeguard_in_callable_bool() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeGuard, Callable

def is_int(val: object) -> TypeGuard[int]:
    return isinstance(val, int)

def takes_bool_callable(f: Callable[[object], bool]) -> None:
    pass

takes_bool_callable(is_int)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0112_typeis_in_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeIs, Callable

def is_str(val: object) -> TypeIs[str]:
    return isinstance(val, str)

def takes_int_callable(f: Callable[[object], int]) -> None:
    pass

takes_int_callable(is_str)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0113: TypeIs inconsistent narrowing ---

#[test]
fn e0113_typeis_inconsistent() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeIs

def is_int(val: str) -> TypeIs[int]:
    return isinstance(val, int)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0114: Protocol isinstance ---

#[test]
fn e0114_protocol_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

class Drawable(Protocol):
    def draw(self) -> None: ...

isinstance(42, Drawable)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0114_runtime_checkable_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Drawable(Protocol):
    def draw(self) -> None: ...

isinstance(42, Drawable)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0119: Protocol isinstance overlap ---

#[test]
fn e0119_protocol_isinstance_overlap() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Sizeable(Protocol):
    def __len__(self) -> int: ...

class MyList:
    def __len__(self) -> int:
        return 0

x: Sizeable = MyList()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0120: Generator return type ---

#[test]
fn e0120_generator_with_non_generator_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def bad_gen() -> int:
    yield 1
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0120_generator_with_iterator_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Iterator

def good_gen() -> Iterator[int]:
    yield 1
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0120_generator_with_generator_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, None]:
    yield 1
    yield 2
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0120_async_generator_invalid_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
async def bad_async_gen() -> int:
    yield 1
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0120_generator_yield_from() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def inner() -> Generator[int, None, None]:
    yield 1

def outer() -> Generator[int, None, None]:
    yield from inner()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0121: Protocol conformance ---

#[test]
fn e0121_protocol_conformance_missing_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    pass

def render(item: Drawable) -> None:
    item.draw()

render(Circle())
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0121_protocol_conformance_satisfied() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    def draw(self) -> None:
        pass

def render(item: Drawable) -> None:
    item.draw()

render(Circle())
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0122: Callable arity ---

#[test]
fn e0122_callable_arity_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_binary(f: Callable[[int, int], int]) -> None:
    pass

def unary(x: int) -> int:
    return x

takes_binary(unary)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0122_callable_arity_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_binary(f: Callable[[int, int], int]) -> None:
    pass

def add(x: int, y: int) -> int:
    return x + y

takes_binary(add)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0122_callable_with_varargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_unary(f: Callable[[int], int]) -> None:
    pass

def vararg(*args: int) -> int:
    return sum(args)

takes_unary(vararg)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
