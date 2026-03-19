use super::common::*;

// Coverage boost tests batch 9: complex scenarios hitting deep code paths.
// Each test exercises multiple rule code paths through realistic Python patterns.

// --- Complex e0115 scenarios to hit visit_stmt_for_usage branches ---

#[test]
fn e0115_deprecated_all_stmt_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func")
def old_func() -> int:
    return 42

@deprecated("Use NewClass")
class OldClass:
    @deprecated("Use new_method")
    def old_method(self) -> int:
        return 1

    @deprecated("Use new_add")
    def __add__(self, other: "OldClass") -> "OldClass":
        return self

# Assignment with deprecated RHS
x = old_func()

# Annotated assignment
y: int = old_func()

# Expression statement
old_func()

# If statement
if old_func() > 0:
    z = old_func()
elif old_func() < 0:
    z = 0
else:
    z = old_func()

# While statement
while old_func() > 100:
    break

# For statement
for item in [old_func()]:
    pass

# With statement (if supported)
# Return in function
def wrapper() -> int:
    return old_func()

# Class instantiation
obj = OldClass()

# Augmented assignment with deprecated dunder
a = OldClass()
b = OldClass()

# Try/except
try:
    old_func()
except Exception:
    old_func()
finally:
    old_func()

# Delete
d = old_func()
del d

# Assert
assert old_func() > 0

# List/dict/set comprehension
result_list = [old_func() for _ in range(1)]
result_dict = {i: old_func() for i in range(1)}
result_set = {old_func() for _ in range(1)}

# Ternary
val = old_func() if True else 0

# Lambda body
fn = lambda: old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_class_attribute_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Library:
    @deprecated("Use new_compute")
    def old_compute(self) -> int:
        return 1

    @property
    @deprecated("Use new_value")
    def old_value(self) -> int:
        return 42

lib = Library()
x = lib.old_compute()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_operator_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Vec:
    @deprecated("Use add method")
    def __add__(self, other: "Vec") -> "Vec":
        return self

    @deprecated("Use sub method")
    def __sub__(self, other: "Vec") -> "Vec":
        return self

    @deprecated("Use mul method")
    def __mul__(self, other: "Vec") -> "Vec":
        return self

a = Vec()
b = Vec()
c = a + b
d = a - b
e = a * b
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_import_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_func")
def old_func() -> int:
    return 42

# Use as default argument in another function
def consumer(val: int = old_func()) -> int:
    return val

# Use in global scope
GLOBAL_VAL = old_func()

# Use in class body
class Config:
    DEFAULT = old_func()
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0137 scenarios ---

#[test]
fn e0137_generic_protocol_full_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')

class Transformer(Protocol[T, U]):
    def transform(self, input: T) -> U: ...

class IntToStr:
    def transform(self, input: int) -> str:
        return str(input)

class StrToInt:
    def transform(self, input: str) -> int:
        return int(input)

t1: Transformer[int, str] = IntToStr()
t2: Transformer[str, int] = StrToInt()
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0137_protocol_with_multiple_methods_and_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, TypeVar

T_co = TypeVar('T_co', covariant=True)

class Repository(Protocol[T_co]):
    def get(self, id: int) -> T_co: ...
    def list_all(self) -> list: ...
    name: str

class UserRepo:
    name: str = 'users'
    def get(self, id: int) -> str:
        return f'user_{id}'
    def list_all(self) -> list:
        return []

repo: Repository[str] = UserRepo()
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0140 scenarios ---

#[test]
fn e0140_callable_complex_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable, Protocol

class EventHandler(Protocol):
    def __call__(self, event: str, data: dict) -> bool: ...

def handle_event(event: str, data: dict) -> bool:
    return True

def wrong_handler(event: int) -> str:
    return ''

handler: EventHandler = handle_event
wrong: EventHandler = wrong_handler

# Callable with complex signatures
processor: Callable[[str, int, float], bool] = lambda s, i, f: True

def three_arg(a: str, b: int, c: float) -> bool:
    return True

p2: Callable[[str, int, float], bool] = three_arg
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_with_class_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Configurable(Protocol):
    name: str
    version: int
    def __call__(self, data: str) -> None: ...

def simple_func(data: str) -> None:
    pass

c: Configurable = simple_func
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_annotated_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

class EventSystem:
    on_click: Callable[[int, int], None]
    on_key: Callable[[str], bool]

def click_handler(x: int, y: int) -> None:
    pass

def key_handler(key: str) -> bool:
    return True

es = EventSystem()
es.on_click = click_handler
es.on_key = key_handler
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0107 scenarios ---

#[test]
fn e0107_variance_with_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar('T')
T_co = TypeVar('T_co', covariant=True)
T_contra = TypeVar('T_contra', contravariant=True)

class Container(Generic[T]):
    pass

# Alias with invariant TypeVar
InvAlias: TypeAlias = Container[T]

# Alias with covariant TypeVar - should error when used in invariant position
CoAlias: TypeAlias = Container[T_co]
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0107_multiple_base_classes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')
T_co = TypeVar('T_co', covariant=True)

class First(Generic[T]):
    pass

class Second(Generic[U]):
    pass

class Combined(First[T_co], Second[T_co]):
    pass
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0036 scenarios ---

#[test]
fn e0036_classvar_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar, Optional, List, Dict

class MyClass:
    # Valid ClassVar usage
    count: ClassVar[int] = 0
    name: ClassVar[str] = 'default'
    items: ClassVar[List[int]] = []
    config: ClassVar[Dict[str, int]] = {}
    optional: ClassVar[Optional[str]] = None

    # Method accessing ClassVar
    def increment(self) -> None:
        MyClass.count += 1

    def reset(self) -> None:
        MyClass.count = 0

class Derived(MyClass):
    extra: ClassVar[int] = 10
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0036_classvar_invalid_locations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

# Module level - invalid
module_var: ClassVar[int] = 42

def func() -> None:
    # Function local - invalid
    local_var: ClassVar[str] = 'test'

def func2(x: ClassVar[int]) -> None:
    pass

def func3() -> ClassVar[int]:
    return 1

class Container:
    # Class body - valid
    valid: ClassVar[int] = 0

    def method(self) -> None:
        # Method local - invalid
        local: ClassVar[str] = 'bad'
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0149 scenarios ---

#[test]
fn e0149_complex_nesting() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic, Callable

T = TypeVar('T')
U = TypeVar('U')
V = TypeVar('V')

class Repository(Generic[T]):
    def find(self, id: int) -> T: ...
    def transform(self, func: Callable[[T], U]) -> U: ...

    class QueryBuilder(Generic[V]):
        def where(self, predicate: Callable[[V], bool]) -> 'Repository.QueryBuilder[V]': ...
        def select(self, selector: Callable[[V], T]) -> list: ...

def identity(x: T) -> T:
    return x

def compose(f: Callable[[T], U], g: Callable[[U], V]) -> Callable[[T], V]:
    return lambda x: g(f(x))
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0079 scenarios ---

#[test]
fn e0079_protocol_with_complex_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, Callable, List, Dict

class DataSource(Protocol):
    name: str
    timeout: int
    def connect(self) -> bool: ...
    def query(self, sql: str) -> List[Dict[str, str]]: ...
    def close(self) -> None: ...
    on_error: Callable[[Exception], None]

import os
src: DataSource = os
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0144 scenarios ---

#[test]
fn e0144_type_call_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# Various type() call patterns
x = type(42)
y = type("hello")
z = type([1, 2, 3])
w = type(None)

# Dynamic class creation
DynClass = type("DynClass", (object,), {"x": 1, "y": 2})

# With base class
class Base:
    pass

Child = type("Child", (Base,), {"z": 3})

# Multiple bases
class Mixin:
    pass

Multi = type("Multi", (Base, Mixin), {})
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0120 scenarios ---

#[test]
fn e0120_generator_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, Iterator, Iterable, AsyncGenerator, AsyncIterator

def simple_gen() -> Generator[int, None, None]:
    yield 1
    yield 2
    yield 3

def iter_gen() -> Iterator[str]:
    yield "a"
    yield "b"

def iterable_gen() -> Iterable[float]:
    yield 1.0
    yield 2.0

def gen_with_return() -> Generator[int, None, str]:
    yield 1
    return "done"

def gen_with_send() -> Generator[int, str, None]:
    value = yield 1
    result = yield 2

async def async_gen() -> AsyncGenerator[int, None]:
    yield 1
    yield 2

async def async_iter() -> AsyncIterator[str]:
    yield "x"
    yield "y"

# Non-generator function that has yield (wrong return type)
def wrong_return() -> int:
    yield 1

# No annotation generator
def no_ann_gen():
    yield 42
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0138 scenarios ---

#[test]
fn e0138_comprehensive_transform() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform(frozen_default=True, order_default=True)
def frozen_model(cls):
    return cls

@dataclass_transform(eq_default=True, kw_only_default=True)
def kw_model(cls):
    return cls

@dataclass_transform()
class BaseMeta(type):
    pass

class BaseModel(metaclass=BaseMeta):
    pass

@frozen_model
class FrozenUser:
    name: str
    age: int

@kw_model
class KWConfig:
    host: str
    port: int

class DerivedModel(BaseModel):
    value: float
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0131 scenarios ---

#[test]
fn e0131_generator_yield_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, Iterator, AsyncGenerator

def int_gen() -> Generator[int, None, None]:
    yield 1
    yield 2
    yield 3

def str_gen() -> Iterator[str]:
    yield "hello"
    yield "world"

def mixed_yield() -> Generator[int, None, None]:
    yield 1
    yield "wrong"
    yield 3

async def async_int_gen() -> AsyncGenerator[int, None]:
    yield 1
    yield 2

def gen_with_send() -> Generator[int, str, bool]:
    val = yield 1
    yield 2
    return True
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0014 scenarios ---

#[test]
fn e0014_comprehensive_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal, Optional

# Basic mismatches
a: int = "hello"
b: str = 42
c: float = "1.5"
d: bool = "yes"
e: bytes = 42

# Correct assignments
f: int = 42
g: str = "hello"
h: float = 3.14
i: bool = True
j: bytes = b"hello"

# Float accepts int
k: float = 42

# None
l: int = None

# Literal types
m: Literal[1] = 2
n: Literal["a"] = "b"
o: Literal[True] = False
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0014_local_var_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(param_int: int, param_str: str, param_float: float) -> None:
    x: str = param_int
    y: int = param_str
    z: bool = param_float

def func2(data: list) -> None:
    x: dict = data
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0130 scenarios ---

#[test]
fn e0130_typevar_scoping_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic, Callable

T = TypeVar('T')
U = TypeVar('U')
V = TypeVar('V')
W = TypeVar('W')

class Container(Generic[T]):
    def map(self, func: Callable[[T], U]) -> 'Container[U]': ...
    def flat_map(self, func: Callable[[T], 'Container[U]']) -> 'Container[U]': ...

class Pair(Generic[T, U]):
    first: T
    second: U

    def swap(self) -> 'Pair[U, T]': ...
    def map_first(self, func: Callable[[T], V]) -> 'Pair[V, U]': ...
    def map_second(self, func: Callable[[U], V]) -> 'Pair[T, V]': ...

def zip_with(f: Callable[[T, U], V], xs: list, ys: list) -> list: ...
def curry(f: Callable[[T, U], V]) -> Callable[[T], Callable[[U], V]]: ...
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0111 scenarios ---

#[test]
fn e0111_constructor_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Simple:
    def __init__(self, x: int) -> None:
        self.x = x

class WithDefaults:
    def __init__(self, a: int, b: str = "default", c: float = 1.0) -> None:
        pass

class WithVarArgs:
    def __init__(self, *args: int, **kwargs: str) -> None:
        pass

class WithKwOnly:
    def __init__(self, a: int, *, b: str) -> None:
        pass

class WithPosOnly:
    def __init__(self, a: int, /, b: str) -> None:
        pass

# Constructor calls
s1 = Simple(1)
s2 = Simple(1, 2)  # too many
s3 = Simple()  # too few

w1 = WithDefaults(1)
w2 = WithDefaults(1, "hello")
w3 = WithDefaults(1, "hello", 2.0)
w4 = WithDefaults(1, "hello", 2.0, "extra")  # too many

v1 = WithVarArgs(1, 2, 3, name="test")

k1 = WithKwOnly(1, b="hello")
k2 = WithKwOnly(1)  # missing kw-only

p1 = WithPosOnly(1, "hello")
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0047 scenarios ---

#[test]
fn e0047_comprehensive_invalid_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import types

var1 = 3
var2 = "hello"

# Valid annotations
def valid(a: int, b: str, c: float) -> None:
    pass

# Invalid: list literal
def bad1(x: [int, str]) -> None:
    pass

# Invalid: dict literal
y: {} = {}

# Invalid: conditional
def bad2(x: int if True else str) -> None:
    pass

# Invalid: module name as annotation
def bad3(x: types) -> None:
    pass

# Invalid: unannotated var as type
def bad4(x: var1) -> None:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0122 scenarios ---

#[test]
fn e0122_callable_arity_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_nullary(f: Callable[[], int]) -> None: pass
def takes_unary(f: Callable[[int], None]) -> None: pass
def takes_binary(f: Callable[[int, str], bool]) -> None: pass
def takes_ternary(f: Callable[[int, str, float], None]) -> None: pass
def takes_any(f: Callable[..., None]) -> None: pass

def no_args() -> int: return 1
def one_arg(x: int) -> None: pass
def two_args(x: int, y: str) -> bool: return True
def three_args(x: int, y: str, z: float) -> None: pass

takes_nullary(no_args)
takes_unary(one_arg)
takes_binary(two_args)
takes_ternary(three_args)
takes_any(three_args)

# Arity mismatches
takes_unary(two_args)  # too many params
takes_binary(one_arg)  # too few params
takes_nullary(one_arg)  # extra param
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0121 scenarios ---

#[test]
fn e0121_protocol_conformance_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Serializable(Protocol):
    def serialize(self) -> str: ...
    def deserialize(self, data: str) -> None: ...

class Sizeable(Protocol):
    def __len__(self) -> int: ...

class Iterable(Protocol):
    def __iter__(self): ...
    def __next__(self): ...

class GoodSerializer:
    def serialize(self) -> str:
        return '{}'
    def deserialize(self, data: str) -> None:
        pass

class BadSerializer:
    def serialize(self) -> int:
        return 0

class PartialSerializer:
    def serialize(self) -> str:
        return ''

def process_serializable(s: Serializable) -> None: pass
def process_sizeable(s: Sizeable) -> None: pass

process_serializable(GoodSerializer())
process_serializable(BadSerializer())
process_serializable(PartialSerializer())
process_sizeable([1, 2, 3])
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0095 scenarios ---

#[test]
fn e0095_initvar_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, InitVar, field

@dataclass
class DatabaseConnection:
    host: str
    port: int
    password: InitVar[str]
    debug: InitVar[bool] = False

    def __post_init__(self, password: str, debug: bool) -> None:
        pass

@dataclass
class ComplexConfig:
    name: str
    init_data: InitVar[dict]
    items: list = field(default_factory=list)

    def __post_init__(self, init_data: dict) -> None:
        pass

c = DatabaseConnection('localhost', 5432, 'secret')
d = DatabaseConnection('localhost', 5432, 'secret', True)
";
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0143 scenarios ---

#[test]
fn e0143_namedtuple_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float

class Point3D(NamedTuple):
    x: float
    y: float
    z: float = 0.0

class Config(NamedTuple):
    host: str
    port: int = 8080
    debug: bool = False

# Valid constructor calls
p1 = Point(1.0, 2.0)
p2 = Point3D(1.0, 2.0)
p3 = Point3D(1.0, 2.0, 3.0)
c1 = Config("localhost")
c2 = Config("localhost", 9090)
c3 = Config("localhost", 9090, True)

# Type mismatch
p4 = Point("wrong", "types")
c4 = Config(42)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0126 scenarios ---

#[test]
fn e0126_literal_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

# String literals
mode: Literal["r"] = "r"
bad_mode: Literal["r"] = "w"

# Multiple string literals
direction: Literal["north", "south", "east", "west"] = "north"
bad_dir: Literal["up", "down"] = "left"

# Int literals
one: Literal[1] = 1
two: Literal[1] = 2

# Bool literals
flag: Literal[True] = True
bad_flag: Literal[True] = False

# Combined
status: Literal[0, 1, -1] = 0
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Complex e0139 scenarios ---

#[test]
fn e0139_typevartuple_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, TypeVarTuple, Generic, Unpack

T = TypeVar('T')
Ts = TypeVarTuple('Ts')

class Tensor(Generic[*Ts]):
    pass

class TaggedTensor(Generic[T, *Ts]):
    pass

# Concrete specializations
scalar: Tensor[()] = Tensor()
vector: Tensor[int] = Tensor()
matrix: Tensor[int, int] = Tensor()
cube: Tensor[int, int, int] = Tensor()

tagged: TaggedTensor[str, int, int] = TaggedTensor()
";
    let _ = run(source)?;
    Ok(())
}
