//! Coverage boost tests batch 4: deeper coverage of complex rules.
//! Focuses on rules that re-parse source code and have complex control flow.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// --- Deep E0111: Constructor call errors ---

#[test]
fn e0111_multiple_init_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
U = TypeVar("U")

class Pair(Generic[T, U]):
    def __init__(self, first: T, second: U) -> None:
        self.first = first
        self.second = second

p = Pair[int, str](1, "hello")
q = Pair[int, str](1.0, "hello")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0111_inherited_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    def __init__(self, x: int) -> None:
        self.x = x

class Child(Base):
    pass

c = Child(42)
d = Child()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0111_init_with_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    def __init__(self, name: str = "default", value: int = 0) -> None:
        pass

m1 = MyClass()
m2 = MyClass("test")
m3 = MyClass("test", 42)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0115: Deprecated usage ---

#[test]
fn e0115_deprecated_class_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use NewBase instead")
class OldBase:
    pass

class Child(OldBase):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_with_message_format() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Removed in v2.0. Use new_api() instead.")
def old_api(x: int) -> int:
    return x

@deprecated("Will be removed")
class OldService:
    @deprecated("Use process_v2")
    def process(self) -> None:
        pass

old_api(1)
s = OldService()
s.process()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_property() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Config:
    @property
    @deprecated("Use get_value instead")
    def old_value(self) -> int:
        return 0

    def get_value(self) -> int:
        return 0
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0140: Callable assignment ---

#[test]
fn e0140_callable_with_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def greet(name: str, greeting: str = "Hello") -> str:
    return f"{greeting}, {name}!"

f: Callable[[str], str] = greet
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_lambda_to_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

f: Callable[[int], int] = lambda x: x * 2
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_with_extra_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Handler(Protocol):
    name: str
    def handle(self, x: int) -> str: ...

class ConcreteHandler:
    name: str = 'test'
    def handle(self, x: int) -> str:
        return str(x)

h: Handler = ConcreteHandler()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_ellipsis_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def any_func(*args: int) -> int:
    return sum(args)

f: Callable[..., int] = any_func
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def my_func(x: int, *, key: str) -> None:
    pass

f: Callable[[int], None] = my_func
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0130: TypeVar scoping ---

#[test]
fn e0130_typevar_correct_nested() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')

class Outer(Generic[T]):
    class Inner(Generic[U]):
        def method(self, x: U) -> U:
            return x
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0130_method_call_correct_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T')

class Container(Generic[T]):
    def append(self, val: T) -> None:
        pass
    def get(self) -> T: ...

x: Container[int] = Container()
x.append(42)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0147: Tuple starred unpack ---

#[test]
fn e0147_complex_starred_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x1: tuple[int, *tuple[str, ...]] = (1, "a", "b", "c")
x2: tuple[int, *tuple[str, ...], float] = (1, "a", "b", 3.14)
x3: tuple[str, *tuple[int, ...]] = ("hello", 1, 2, 3)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0147_starred_tuple_reassign_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[int, *tuple[str, ...]] = (1,)
t = (1, "a")
t = (1, "a", "b")
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0125: Instance attr on class ---

#[test]
fn e0125_instance_attr_with_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    x: int = 10
    y: str

MyClass.x
MyClass.y
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0125_type_call_attr_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar('T')

class Node(Generic[T]):
    label: T

n1 = Node()
type(n1).label
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0126/E0129: Literal assignments ---

#[test]
fn e0129_literal_multi_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[1, 2, 3] = 1
y: Literal[1, 2, 3] = 4
z: Literal["a", "b"] = "a"
w: Literal["a", "b"] = "c"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0129_literal_negative_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[-1] = -1
y: Literal[-1] = 1
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0128: TypeVar default referential ---

#[test]
fn e0128_complex_default_chain() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

A = TypeVar('A')
B = TypeVar('B', default=A)
C = TypeVar('C', default=B)

class Triple(Generic[A, B, C]): ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0128_default_with_constraints() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

X = TypeVar('X', int, str)
Y = TypeVar('Y', int, str, float, default=X)

class Pair(Generic[X, Y]): ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0128_default_with_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

P = TypeVar('P', bound=int)
Q = TypeVar('Q', bound=float, default=P)

class Numeric(Generic[P, Q]): ...
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0131: Generator types ---

#[test]
fn e0131_generator_with_send_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def accumulator() -> Generator[int, int, str]:
    total = 0
    while True:
        value = yield total
        total += value
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0131_iterable_yield() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Iterable

def items() -> Iterable[int]:
    yield 1
    yield 2
    yield 3
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0134: Invariant generic ---

#[test]
fn e0134_dict_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class IntDict(dict[str, int]): ...

def takes_object_dict(d: dict[str, object]) -> None: ...

def test(d: IntDict) -> None:
    takes_object_dict(d)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0134_list_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class IntList(list[int]): ...

def takes_object_list(lst: list[object]) -> None: ...

def test(il: IntList) -> None:
    takes_object_list(il)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0137: Generic protocol ---

#[test]
fn e0137_multi_typevar_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar('T')
U = TypeVar('U')

class BiFunc(Protocol[T, U]):
    def apply(self, x: T) -> U: ...

class IntToStr:
    def apply(self, x: int) -> str:
        return str(x)

f: BiFunc[int, str] = IntToStr()
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0120: Generator return type ---

#[test]
fn e0120_generator_no_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def gen():
    yield 1
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0120_async_generator_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import AsyncIterator

async def agen() -> AsyncIterator[int]:
    yield 1
    yield 2
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0142: Dataclass transform base ---

#[test]
fn e0142_metaclass_transform() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class ModelMeta(type):
    pass

class Model(metaclass=ModelMeta):
    pass

class User(Model):
    name: str
    age: int
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0143: NamedTuple complex ---

#[test]
fn e0143_namedtuple_with_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Config(NamedTuple):
    host: str
    port: int = 8080
    debug: bool = False

c1 = Config('localhost')
c2 = Config('localhost', 9090)
c3 = Config('localhost', 9090, True)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0143_namedtuple_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float

p = Point("not_a_float", 2.0)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0144: type() constructor ---

#[test]
fn e0144_type_with_bases_and_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    pass

MyClass = type("MyClass", (Base,), {"x": 1, "method": lambda self: None})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_wrong_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x = type("Foo", (object,))
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0074: Constructor __new__ ---

#[test]
fn e0074_new_with_multiple_typevars() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar, Self

T = TypeVar("T")
U = TypeVar("U")

class Pair(Generic[T, U]):
    def __new__(cls, first: T, second: U) -> Self:
        return super().__new__(cls)

p1 = Pair[int, str](1, "hello")
p2 = Pair[int, str](1.0, "hello")
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0110: Protocol variance ---

#[test]
fn e0110_protocol_init_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar('T')

class Factory(Protocol[T]):
    def __init__(self, val: T) -> None: ...
    def create(self) -> T: ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0110_protocol_multiple_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_co = TypeVar('T_co', covariant=True)

class MultiRead(Protocol[T_co]):
    def read(self) -> T_co: ...
    def peek(self) -> T_co: ...
    def __iter__(self) -> T_co: ...
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0119: Protocol isinstance overlap ---

#[test]
fn e0119_protocol_structural_subtyping() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class HasLength(Protocol):
    def __len__(self) -> int: ...

@runtime_checkable
class HasIter(Protocol):
    def __iter__(self): ...

class MyList:
    def __len__(self) -> int:
        return 0
    def __iter__(self):
        return iter([])

x: HasLength = MyList()
y: HasIter = MyList()
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0095: InitVar ---

#[test]
fn e0095_initvar_with_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar, field

@dataclass
class Builder:
    name: str
    items: list[int] = field(default_factory=list)
    validate: InitVar[bool] = True

    def __post_init__(self, validate: bool) -> None:
        if validate:
            pass
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0036: ClassVar ---

#[test]
fn e0036_classvar_in_local_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

def f() -> None:
    x: ClassVar[int] = 10
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0036_classvar_in_self_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    def __init__(self) -> None:
        self.x: ClassVar[int] = 10
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0108: Dataclass slots ---

#[test]
fn e0108_slots_with_manual_slots() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(slots=True)
class HasManualSlots:
    __slots__ = ('extra',)
    x: int
    y: int
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0109: TypeVar bound violation at call site ---

#[test]
fn e0109_bound_with_numeric_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar('T', bound=float)

def process(x: T) -> T:
    return x

a = process(42)
b = process(3.14)
c = process(True)
d = process('invalid')
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0102: TypeVar default violation ---

#[test]
fn e0102_complex_constraint_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T1 = TypeVar('T1', int, str, float)
T2 = TypeVar('T2', int, str, default=T1)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0102_default_with_numeric_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

N1 = TypeVar('N1', bound=bool)
N2 = TypeVar('N2', bound=complex, default=N1)
"#;
    let _ = run(source)?;
    Ok(())
}
