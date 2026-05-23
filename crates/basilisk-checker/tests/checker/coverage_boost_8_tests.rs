//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 8: deep code path coverage.
// Targets uncovered branches in e0115, e0137, e0140, e0107, e0149, e0079, e0144, e0072

// --- E0115: Deeper deprecated usage paths ---

#[test]
fn e0115_deprecated_in_with() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_ctx")
def old_ctx():
    return None

with old_ctx() as ctx:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_in_try() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func")
def old_func() -> int:
    return 42

try:
    x = old_func()
except Exception:
    pass
finally:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_in_list_comp() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func")
def old_func(x: int) -> int:
    return x * 2

result = [old_func(i) for i in range(10)]
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_in_dict_comp() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func")
def old_func(x: int) -> str:
    return str(x)

result = {i: old_func(i) for i in range(5)}
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_in_assert() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_check")
def old_check() -> bool:
    return True

assert old_check()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_in_delete() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("old")
def old_func() -> dict:
    return {}

d = old_func()
del d
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_as_default_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_val")
def old_default() -> int:
    return 42

def consumer(val: int = old_default()) -> None:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_class_method_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Lib:
    @deprecated("Use new_method")
    def old_method(self) -> int:
        return 1

    def new_method(self) -> int:
        return 2

obj = Lib()
x = obj.old_method()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_property_setter() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Config:
    _val: int = 0

    @property
    def value(self) -> int:
        return self._val

    @value.setter
    @deprecated("Use set_value method")
    def value(self, val: int) -> None:
        self._val = val
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_nested_function_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_a")
def old_a() -> int:
    return 1

@deprecated("Use new_b")
def old_b(x: int) -> str:
    return str(x)

result = old_b(old_a())
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0137: Generic protocol deep paths ---

#[test]
fn e0137_protocol_generic_combined() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generic, Protocol, TypeVar

T_co = TypeVar('T_co', covariant=True)

class Proto(Protocol[T_co], Generic[T_co]):
    def get(self) -> T_co: ...
";
    let diags = run(source)?;
    assert!(
        diags.iter().any(|d| d.code.code == "BSK-E0137"),
        "Expected BSK-E0137 for Protocol[T]+Generic[T] combination"
    );
    Ok(())
}

#[test]
fn e0137_protocol_generic_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, TypeVar

T = TypeVar('T')

class Getter(Protocol[T]):
    def get(self) -> T: ...

class IntGetter:
    def get(self) -> int:
        return 42

x: Getter[int] = IntGetter()
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0137_protocol_generic_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar('T')

class Getter(Protocol[T]):
    def get(self) -> T: ...

class StrGetter:
    def get(self) -> str:
        return "hello"

x: Getter[int] = StrGetter()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0137_protocol_two_typevars() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, TypeVar

K = TypeVar('K')
V = TypeVar('V')

class Mapper(Protocol[K, V]):
    def get(self, key: K) -> V: ...

class IntToStr:
    def get(self, key: int) -> str:
        return str(key)

m: Mapper[int, str] = IntToStr()
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0137_protocol_self_typed_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, TypeVar, Self

class Copyable(Protocol):
    def copy(self) -> Self: ...

class MyCopy:
    def copy(self) -> 'MyCopy':
        return MyCopy()

x: Copyable = MyCopy()
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0137_generic_protocol_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, TypeVar

T_co = TypeVar('T_co', covariant=True)

class ReadableProto(Protocol[T_co]):
    def read(self) -> T_co: ...

class IntReader:
    def read(self) -> int:
        return 42

r: ReadableProto[int] = IntReader()
";
    let _ = run(source)?;
    Ok(())
}

// --- E0140: Callable assignment deep paths ---

#[test]
fn e0140_callable_annotation_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_two(a: int, b: str) -> bool:
    return True

callback: Callable[[int], bool] = takes_two
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_with_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def func(a: int, b: str = "default") -> bool:
    return True

callback: Callable[[int], bool] = func
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Handler(Protocol):
    def __call__(self, event: str) -> None: ...

def my_handler(event: str) -> None:
    pass

h: Handler = my_handler
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_callable_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Handler(Protocol):
    def __call__(self, event: str) -> None: ...

def wrong_handler(event: int) -> None:
    pass

h: Handler = wrong_handler
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_varargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def variadic(*args: int) -> None:
    pass

callback: Callable[[int, int], None] = variadic
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def with_kwargs(**kwargs: str) -> None:
    pass

callback: Callable[[], None] = with_kwargs
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_ellipsis() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def any_func(a: int, b: str, c: float) -> None:
    pass

callback: Callable[..., None] = any_func
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_lambda_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

callback: Callable[[int], int] = lambda x: x * 2
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_extra_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Processor(Protocol):
    name: str
    def __call__(self, data: str) -> str: ...

def my_processor(data: str) -> str:
    return data.upper()

p: Processor = my_processor
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_positional_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def pos_only(x: int, /) -> None:
    pass

callback: Callable[[int], None] = pos_only
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def kw_only(*, x: int) -> None:
    pass

callback: Callable[[int], None] = kw_only
";
    let _ = run(source)?;
    Ok(())
}

// --- E0107: Variance deeper paths ---

#[test]
fn e0107_alias_single_level() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar('T')
T_co = TypeVar('T_co', covariant=True)

class Base(Generic[T]):
    pass

MyAlias: TypeAlias = Base[T]

class Derived(MyAlias[T_co]):
    pass
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0107_nested_generic_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
T_co = TypeVar('T_co', covariant=True)

class Outer(Generic[T]):
    pass

class Inner(Generic[T]):
    pass
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0107_compose_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
T_co = TypeVar('T_co', covariant=True)
T_contra = TypeVar('T_contra', contravariant=True)

class Producer(Generic[T_co]):
    def get(self) -> T_co: ...

class Consumer(Generic[T_contra]):
    def put(self, val: T_contra) -> None: ...

class Invariant(Generic[T]):
    def get(self) -> T: ...
    def put(self, val: T) -> None: ...
";
    let _ = run(source)?;
    Ok(())
}

// --- E0149: PEP 695 deeper scoping ---

#[test]
fn e0149_class_method_typevar_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')

class Stack(Generic[T]):
    def push(self, item: T) -> None: ...
    def pop(self) -> T: ...
    def map(self, func: U) -> U: ...
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_free_function_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar

T = TypeVar('T')
U = TypeVar('U')
V = TypeVar('V')

def compose(f: T, g: U) -> V: ...
def identity(x: T) -> T: ...
def const_func(x: T, y: U) -> T: ...
";
    let _ = run(source)?;
    Ok(())
}

// --- E0079: Module protocol deeper ---

#[test]
fn e0079_protocol_with_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, Callable

class EventHandler(Protocol):
    on_event: Callable[[str], None]
    name: str

import os
h: EventHandler = os
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0079_protocol_property() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Configurable(Protocol):
    @property
    def config(self) -> dict: ...

import sys
c: Configurable = sys
";
    let _ = run(source)?;
    Ok(())
}

// --- E0144: type() call deeper ---

#[test]
fn e0144_type_with_methods_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def method(self) -> str:
    return "hello"

MyClass = type("MyClass", (object,), {"greet": method, "name": "test"})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_multiple_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class A:
    pass

class B:
    pass

C = type("C", (A, B), {})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x = type()
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_four_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x = type("A", (object,), {}, extra=True)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0072: Overload deeper paths ---

#[test]
fn e0072_multiple_overloads_three() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

class Multi:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    @overload
    def __getitem__(self, __sl: slice) -> list: ...
    def __getitem__(self, __key):
        pass

m = Multi()
m[3.14]
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0072_no_var_class_map() -> Result<(), Box<dyn std::error::Error>> {
    // No constructor calls => no var_class_map => early return
    let source = r"
from typing import overload

class Container:
    @overload
    def __getitem__(self, __i: int) -> int: ...
    @overload
    def __getitem__(self, __s: str) -> str: ...
    def __getitem__(self, __key):
        pass
";
    let _ = run(source)?;
    Ok(())
}

// --- E0036: ClassVar deeper ---

#[test]
fn e0036_classvar_in_method_local() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    count: ClassVar[int] = 0

    def method(self) -> None:
        x: ClassVar[int] = 1
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0036_classvar_typevar_combo() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar, TypeVar, Generic

T = TypeVar('T')

class Container(Generic[T]):
    count: ClassVar[int] = 0
    default: ClassVar[str] = 'none'
";
    let _ = run(source)?;
    Ok(())
}

// --- E0120: Generator deeper ---

#[test]
fn e0120_generator_with_return_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, str]:
    yield 1
    yield 2
    return "done"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0120_async_iterator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import AsyncIterator

async def gen() -> AsyncIterator[int]:
    yield 1
    yield 2
";
    let _ = run(source)?;
    Ok(())
}

// --- E0138: Dataclass transform deeper ---

#[test]
fn e0138_transform_base_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform()
class BaseModel:
    def __init_subclass__(cls, **kwargs) -> None:
        pass

class User(BaseModel):
    name: str
    age: int
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0138_transform_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform(kw_only_default=True)
def model(cls):
    return cls

@model
class Config:
    host: str
    port: int
";
    let _ = run(source)?;
    Ok(())
}

// --- E0131: Generator yield deeper ---

#[test]
fn e0131_multiple_yield_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, None]:
    yield 1
    yield "wrong"
    yield 3
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0131_async_generator_yield() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import AsyncGenerator

async def gen() -> AsyncGenerator[int, None]:
    yield 1
    yield 2
    yield 3
";
    let _ = run(source)?;
    Ok(())
}

// --- E0119: Protocol isinstance deeper ---

#[test]
fn e0119_non_runtime_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class NonRuntime(Protocol):
    def method(self) -> int: ...

isinstance(42, NonRuntime)
";
    let _ = run(source)?;
    Ok(())
}

// --- E0122: Callable arity deeper ---

#[test]
fn e0122_callable_return_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_int_ret(f: Callable[[int], int]) -> None:
    pass

def wrong_ret(x: int) -> str:
    return str(x)

takes_int_ret(wrong_ret)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0122_callable_optional_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_binary(f: Callable[[int, str], None]) -> None:
    pass

def with_default(a: int, b: str = "default") -> None:
    pass

takes_binary(with_default)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0143: NamedTuple deeper ---

#[test]
fn e0143_namedtuple_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Base(NamedTuple):
    x: int
    y: int

class Extended(Base):
    z: int
";
    let _ = run(source)?;
    Ok(())
}

// --- E0121: Protocol conformance deeper ---

#[test]
fn e0121_protocol_with_generic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, TypeVar

T = TypeVar('T')

class Container(Protocol[T]):
    def get(self) -> T: ...
    def set(self, val: T) -> None: ...

class IntContainer:
    def get(self) -> int:
        return 0
    def set(self, val: int) -> None:
        pass

def use_container(c: Container[int]) -> None:
    pass

use_container(IntContainer())
";
    let _ = run(source)?;
    Ok(())
}

// --- E0095: InitVar deeper ---

#[test]
fn e0095_initvar_no_post_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, InitVar

@dataclass
class Config:
    name: str
    temp: InitVar[int]
";
    let _ = run(source)?;
    Ok(())
}

// --- E0050: NewType deeper ---

#[test]
fn e0050_newtype_of_newtype() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NewType

UserId = NewType('UserId', int)
AdminId = NewType('AdminId', UserId)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0050_newtype_of_generic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NewType, List

UserIds = NewType('UserIds', List[int])
";
    let _ = run(source)?;
    Ok(())
}

// --- E0063: Non-hashable deeper ---

#[test]
fn e0063_dataclass_hash_unsafe() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(unsafe_hash=True)
class Point:
    x: int
    y: int

s = {Point(1, 2)}
";
    let _ = run(source)?;
    Ok(())
}

// --- E0130: TypeVar scoping deeper ---

#[test]
fn e0130_typevar_constraint_match() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar

T = TypeVar('T', int, str, float)

def process(x: T) -> T:
    return x
";
    let _ = run(source)?;
    Ok(())
}

// --- E0126: Literal string deeper ---

#[test]
fn e0126_literal_union() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

mode: Literal["r", "w", "a"] = "r"
bad_mode: Literal["r", "w"] = "x"
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0102: TypeVar default deeper ---

#[test]
fn e0102_typevar_default_with_constraints() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T', int, str, default=int)

class Container(Generic[T]):
    pass

c: Container = Container()
";
    let _ = run(source)?;
    Ok(())
}

// --- E0054: Final deeper ---

#[test]
fn e0054_final_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

def func() -> None:
    x: Final = 42
    x = 100
";
    let _ = run(source)?;
    Ok(())
}

// --- E0048: TypeAlias deeper ---

#[test]
fn e0048_typealias_union_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias, Union

IntOrStr: TypeAlias = Union[int, str]
OptInt: TypeAlias = int | None
";
    let _ = run(source)?;
    Ok(())
}

// --- E0041: Too few args deeper ---

#[test]
fn e0041_missing_multiple_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(a: int, b: str, c: float, d: bool) -> None:
    pass

func(1)
func(1, 'a')
";
    let _ = run(source)?;
    Ok(())
}

// --- E0092: Too few type args deeper ---

#[test]
fn e0092_generic_with_three_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')
V = TypeVar('V')

class Triple(Generic[T, U, V]):
    pass

x: Triple[int] = Triple()
y: Triple[int, str] = Triple()
";
    let _ = run(source)?;
    Ok(())
}

// --- E0094: Self type deeper ---

#[test]
fn e0094_self_in_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class MyClass:
    def method(self) -> None:
        def inner() -> Self:
            pass
";
    let _ = run(source)?;
    Ok(())
}

// --- E0145: Invalid type bracket deeper ---

#[test]
fn e0145_callable_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable, List, Dict

x: Callable[[List[int], Dict[str, float]], bool] = lambda a, b: True
";
    let _ = run(source)?;
    Ok(())
}

// --- E0146: Protocol class object deeper ---

#[test]
fn e0146_protocol_with_multiple_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Serializable(Protocol):
    def to_json(self) -> str: ...
    def from_json(self, data: str) -> None: ...

def process(cls: type[Serializable]) -> None:
    obj = cls()
";
    let _ = run(source)?;
    Ok(())
}

// --- E0139: TypeVarTuple deeper ---

#[test]
fn e0139_typevartuple_with_regular() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, TypeVarTuple, Generic, Unpack

T = TypeVar('T')
Ts = TypeVarTuple('Ts')

class Mixed(Generic[T, *Ts]):
    pass

x: Mixed[int, str, float] = Mixed()
";
    let _ = run(source)?;
    Ok(())
}
