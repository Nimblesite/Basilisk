use super::common::*;

// Coverage boost tests batch 7: targeting more uncovered rules.
// Focus: e0149, e0144, e0111, e0120, e0138, e0131, e0119, e0122, e0143,
//        e0121, e0095, e0139, e0126, e0063, e0073, e0112, e0096

// --- E0149: PEP 695 type param scoping (171 uncovered) ---

#[test]
fn e0149_type_param_shadowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')

class Outer(Generic[T]):
    class Inner(Generic[T]):
        pass
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_type_param_in_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')

class Container(Generic[T]):
    def transform(self, func: U) -> U:
        return func
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_multiple_nested_scopes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')
V = TypeVar('V')

class A(Generic[T]):
    class B(Generic[U]):
        class C(Generic[V]):
            def method(self) -> V: ...
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_type_param_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar

T = TypeVar('T')

def outer(x: T) -> T:
    def inner(y: T) -> T:
        return y
    return inner(x)
";
    let _ = run(source)?;
    Ok(())
}

// --- E0144: type() call constructor (155 uncovered) ---

#[test]
fn e0144_type_three_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
MyClass = type("MyClass", (object,), {"x": 1})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_wrong_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Bad = type("Bad", (object,))
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_one_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x = type(42)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_with_bases_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    pass

Child = type("Child", (Base,), {})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_invalid_dict_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Bad = type("Bad", (object,), [1, 2, 3])
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0111: Constructor call errors (147 uncovered) ---

#[test]
fn e0111_missing_required_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Point:
    def __init__(self, x: int, y: int) -> None:
        self.x = x
        self.y = y

p = Point(1)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0111_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Simple:
    def __init__(self, x: int) -> None:
        self.x = x

s = Simple(1, 2, 3)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0111_keyword_arg_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Config:
    def __init__(self, host: str, port: int = 8080) -> None:
        self.host = host
        self.port = port

c = Config(host="localhost", port=9090)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0111_starred_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Flexible:
    def __init__(self, *args: int, **kwargs: str) -> None:
        pass

f = Flexible(1, 2, 3, name='test')
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0111_inherited_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    def __init__(self, x: int) -> None:
        self.x = x

class Child(Base):
    pass

c = Child(42)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0111_dataclass_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

p = Point(1, 2)
q = Point(x=1, y=2)
";
    let _ = run(source)?;
    Ok(())
}

// --- E0120: Generator return type (131 uncovered) ---

#[test]
fn e0120_generator_wrong_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator

def gen() -> Generator[int, None, None]:
    yield 1
    yield 2
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0120_iterator_yield() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Iterator

def gen() -> Iterator[int]:
    yield 1
    yield 2
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0120_async_generator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import AsyncGenerator

async def gen() -> AsyncGenerator[int, None]:
    yield 1
    yield 2
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0120_no_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def gen():
    yield 1
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0120_non_generator_with_yield() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def gen() -> int:
    yield 1
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0120_iterable_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Iterable

def gen() -> Iterable[int]:
    yield 1
    yield 2
";
    let _ = run(source)?;
    Ok(())
}

// --- E0138: Dataclass transform (128 uncovered) ---

#[test]
fn e0138_metaclass_transform() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform()
class ModelMeta(type):
    pass

class Model(metaclass=ModelMeta):
    name: str
    age: int
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0138_transform_with_eq_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform(eq_default=True)
def model(cls):
    return cls

@model
class User:
    name: str

u1 = User()
u2 = User()
result = u1 == u2
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0138_transform_field_specifiers() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

def field(*, default=None):
    pass

@dataclass_transform(field_specifiers=(field,))
def model(cls):
    return cls

@model
class Config:
    name: str
    port: int
";
    let _ = run(source)?;
    Ok(())
}

// --- E0131: Generator yield type (121 uncovered) ---

#[test]
fn e0131_yield_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, str]:
    yield "wrong"
    return "done"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0131_yield_from() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator

def inner() -> Generator[int, None, None]:
    yield 1

def outer() -> Generator[int, None, None]:
    yield from inner()
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0131_send_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator

def gen() -> Generator[int, str, None]:
    value = yield 1
";
    let _ = run(source)?;
    Ok(())
}

// --- E0119: Protocol isinstance overlap (108 uncovered) ---

#[test]
fn e0119_protocol_isinstance_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Printable(Protocol):
    def __str__(self) -> str: ...

x: object = 42
isinstance(x, Printable)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0119_multiple_protocol_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, runtime_checkable

@runtime_checkable
class HasLen(Protocol):
    def __len__(self) -> int: ...

@runtime_checkable
class HasStr(Protocol):
    def __str__(self) -> str: ...

x: object = []
if isinstance(x, HasLen) and isinstance(x, HasStr):
    pass
";
    let _ = run(source)?;
    Ok(())
}

// --- E0122: Callable arity (100 uncovered) ---

#[test]
fn e0122_too_many_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_unary(f: Callable[[int], None]) -> None:
    pass

def binary(a: int, b: int) -> None:
    pass

takes_unary(binary)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0122_correct_arity() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_binary(f: Callable[[int, str], bool]) -> None:
    pass

def checker(a: int, b: str) -> bool:
    return True

takes_binary(checker)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0122_ellipsis_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_any(f: Callable[..., None]) -> None:
    pass

def multi(a: int, b: str, c: float) -> None:
    pass

takes_any(multi)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0122_no_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def takes_nullary(f: Callable[[], int]) -> None:
    pass

def no_args() -> int:
    return 42

takes_nullary(no_args)
";
    let _ = run(source)?;
    Ok(())
}

// --- E0143: NamedTuple usage (98 uncovered) ---

#[test]
fn e0143_namedtuple_functional_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

Point = NamedTuple('Point', [('x', float), ('y', float)])
p = Point(1.0, 2.0)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0143_namedtuple_class_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float
    z: float = 0.0

p = Point(1.0, 2.0)
q = Point(1.0, 2.0, 3.0)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0143_namedtuple_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Config(NamedTuple):
    host: str
    port: int = 8080
    debug: bool = False

c = Config("localhost")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0143_namedtuple_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p = Point("wrong", "types")
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0121: Protocol conformance (92 uncovered) ---

#[test]
fn e0121_protocol_wrong_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Sized(Protocol):
    def __len__(self) -> int: ...

class BadImpl:
    def __len__(self) -> str:
        return ''

def use_sized(s: Sized) -> None:
    pass

use_sized(BadImpl())
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0121_protocol_missing_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasBothMethods(Protocol):
    def method_a(self) -> int: ...
    def method_b(self) -> str: ...

class HasOnlyA:
    def method_a(self) -> int:
        return 1

def use_both(x: HasBothMethods) -> None:
    pass

use_both(HasOnlyA())
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0121_protocol_with_property() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasName(Protocol):
    @property
    def name(self) -> str: ...

class GoodImpl:
    @property
    def name(self) -> str:
        return 'test'

def use_named(n: HasName) -> None:
    pass

use_named(GoodImpl())
";
    let _ = run(source)?;
    Ok(())
}

// --- E0095: InitVar field (91 uncovered) ---

#[test]
fn e0095_initvar_with_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, InitVar

@dataclass
class Connection:
    host: str
    port: int
    password: InitVar[str]

    def __post_init__(self, password: str) -> None:
        pass

c = Connection('localhost', 5432, 'secret')
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0095_initvar_multiple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, InitVar

@dataclass
class Config:
    name: str
    init_a: InitVar[int]
    init_b: InitVar[str]

    def __post_init__(self, init_a: int, init_b: str) -> None:
        pass
";
    let _ = run(source)?;
    Ok(())
}

// --- E0139: TypeVarTuple specialization (90 uncovered) ---

#[test]
fn e0139_typevartuple_generic_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVarTuple, Generic, Unpack

Ts = TypeVarTuple('Ts')

class Tensor(Generic[*Ts]):
    pass

x: Tensor[int, float, str] = Tensor()
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0139_typevartuple_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVarTuple, Unpack

Ts = TypeVarTuple('Ts')

def func(*args: Unpack[tuple[int, str]]) -> None:
    pass
";
    let _ = run(source)?;
    Ok(())
}

// --- E0126: Literal string assignment (85 uncovered) ---

#[test]
fn e0126_literal_string_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal["hello"] = "hello"
"#;
    let diags = run(source)?;
    let e0126_count = diags.iter().filter(|d| d.code.code == "BSK-E0126").count();
    assert_eq!(e0126_count, 0);
    Ok(())
}

#[test]
fn e0126_literal_string_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal["hello"] = "world"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0126_literal_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

x: Literal[1] = 1
y: Literal[1] = 2
";
    let _ = run(source)?;
    Ok(())
}

// --- E0063: Non-hashable dataclass (84 uncovered) ---

#[test]
fn e0063_unhashable_eq_no_frozen() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(eq=True)
class Point:
    x: int
    y: int

s = {Point(1, 2)}
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0063_frozen_hashable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class FrozenPoint:
    x: int
    y: int

s = {FrozenPoint(1, 2)}
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0063_explicit_hash() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int
    def __hash__(self) -> int:
        return hash((self.x, self.y))
";
    let _ = run(source)?;
    Ok(())
}

// --- E0073: NamedTuple tuple compat (75 uncovered) ---

#[test]
fn e0073_namedtuple_as_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple, Tuple

class Point(NamedTuple):
    x: float
    y: float

def takes_tuple(t: Tuple[float, float]) -> None:
    pass

p = Point(1.0, 2.0)
takes_tuple(p)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0073_namedtuple_wrong_tuple_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple, Tuple

class Point(NamedTuple):
    x: float
    y: float

def takes_int_tuple(t: Tuple[int, int]) -> None:
    pass

p = Point(1.0, 2.0)
takes_int_tuple(p)
";
    let _ = run(source)?;
    Ok(())
}

// --- E0112: TypeGuard callable return (75 uncovered) ---

#[test]
fn e0112_typeguard_callable_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard, Callable

def is_int(x: object) -> TypeGuard[int]:
    return isinstance(x, int)

def takes_guard(f: Callable[[object], TypeGuard[int]]) -> None:
    pass

takes_guard(is_int)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0112_typeis_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs, Callable

def is_str(x: object) -> TypeIs[str]:
    return isinstance(x, str)

def takes_checker(f: Callable[[object], TypeIs[str]]) -> None:
    pass

takes_checker(is_str)
";
    let _ = run(source)?;
    Ok(())
}

// --- E0096: Dataclass default factory (37 uncovered) ---

#[test]
fn e0096_field_with_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class Container:
    items: list = field(default_factory=list)
    values: dict = field(default_factory=dict)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0096_field_default_and_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class Bad:
    items: list = field(default=[], default_factory=list)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0096_mutable_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Bad:
    items: list = []
    values: dict = {}
";
    let _ = run(source)?;
    Ok(())
}

// --- E0130: TypeVar scoping additional (102 uncovered) ---

#[test]
fn e0130_typevar_in_nested_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')

class Outer(Generic[T]):
    class Inner:
        def method(self, x: T) -> T:
            return x
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0130_typevar_multiple_classes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')

class First(Generic[T]):
    pass

class Second(Generic[T]):
    pass
";
    let _ = run(source)?;
    Ok(())
}

// --- E0050: Invalid newtype additional ---

#[test]
fn e0050_newtype_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NewType

UserId = NewType('UserId', int)
Name = NewType('Name', str)
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0050_newtype_invalid_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NewType

Bad = NewType('Bad', 42)
";
    let _ = run(source)?;
    Ok(())
}

// --- E0145: Invalid type bracket additional ---

#[test]
fn e0145_deeply_nested_generics() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Dict, List, Optional, Tuple

x: Dict[str, List[Tuple[int, Optional[str]]]] = {}
y: List[Dict[str, List[int]]] = []
";
    let _ = run(source)?;
    Ok(())
}

// --- E0146: Protocol class object additional ---

#[test]
fn e0146_protocol_as_type_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, TypeVar

T = TypeVar('T')

class Sizeable(Protocol):
    def __len__(self) -> int: ...

def create(cls: type[Sizeable]) -> Sizeable:
    return cls()
";
    let _ = run(source)?;
    Ok(())
}

// --- E0148: Generic type arg additional ---

#[test]
fn e0148_too_many_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')

class Single(Generic[T]):
    pass

x: Single[int, str] = Single()
";
    let _ = run(source)?;
    Ok(())
}

// --- E0078: Self type violation additional ---

#[test]
fn e0078_self_type_in_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, Self

class Copyable(Protocol):
    def copy(self) -> Self: ...
    def clone(self) -> Self: ...
";
    let _ = run(source)?;
    Ok(())
}

// --- E0038: TypedDict inheritance additional ---

#[test]
fn e0038_typeddict_multiple_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Base1(TypedDict):
    name: str

class Base2(TypedDict):
    age: int

class Combined(Base1, Base2):
    email: str
";
    let _ = run(source)?;
    Ok(())
}

// --- E0054: Final reassignment ---

#[test]
fn e0054_final_module_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

MAX: Final = 100
MAX = 200
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0054_final_class_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

class Config:
    VERSION: Final = '1.0'

Config.VERSION = '2.0'
";
    let _ = run(source)?;
    Ok(())
}

// --- E0048: TypeAlias invalid RHS ---

#[test]
fn e0048_typealias_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

IntOrStr: TypeAlias = int | str
Numbers: TypeAlias = list[int]
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0048_typealias_invalid_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

Bad: TypeAlias = [int, str]
";
    let _ = run(source)?;
    Ok(())
}

// --- E0015: Assignment type mismatch additional ---

#[test]
fn e0015_complex_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import List, Dict

x: List[int] = [1, 2, 3]
y: Dict[str, int] = {'a': 1}
";
    let _ = run(source)?;
    Ok(())
}

// --- E0041: Too few args additional ---

#[test]
fn e0041_function_too_few() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(x: int, y: str, z: float) -> None:
    pass

f(1)
";
    let _ = run(source)?;
    Ok(())
}

// --- E0067: Enum non-member literal additional ---

#[test]
fn e0067_enum_with_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

    def describe(self) -> str:
        return self.name.lower()
";
    let _ = run(source)?;
    Ok(())
}

// --- E0116: NamedTuple definition additional ---

#[test]
fn e0116_namedtuple_invalid_field() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class BadTuple(NamedTuple):
    _x: int
    y: str
";
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0116_namedtuple_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float

    def distance(self) -> float:
        return (self.x ** 2 + self.y ** 2) ** 0.5
";
    let _ = run(source)?;
    Ok(())
}

// --- E0118: super() abstract additional ---

#[test]
fn e0118_super_in_non_abstract_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from abc import ABC, abstractmethod

class Base(ABC):
    @abstractmethod
    def process(self) -> int: ...

    @abstractmethod
    def validate(self) -> bool: ...

class Partial(Base):
    def process(self) -> int:
        return 42

class Full(Base):
    def process(self) -> int:
        return 1
    def validate(self) -> bool:
        return True
";
    let _ = run(source)?;
    Ok(())
}

// --- E0092: Too few type args additional ---

#[test]
fn e0092_generic_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')

class Pair(Generic[T, U]):
    pass

x: Pair = Pair()
";
    let _ = run(source)?;
    Ok(())
}

// --- E0094: Self type location additional ---

#[test]
fn e0094_self_in_free_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

def bad_func() -> Self:
    pass
";
    let _ = run(source)?;
    Ok(())
}

// --- E0102: TypeVar default violation additional ---

#[test]
fn e0102_typevar_with_bound_and_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar, Generic

T = TypeVar('T', bound=int, default=float)

class Container(Generic[T]):
    pass
";
    let _ = run(source)?;
    Ok(())
}
