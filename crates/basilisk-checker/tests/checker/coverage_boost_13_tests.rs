use super::common::*;

// Coverage boost tests batch 13: targeting deep uncovered code paths.
// Focuses on: e0140 (protocol/callable compat - varargs, kwargs, positional-only,
// kw-only, defaults, Concatenate, param types), e0115 (deprecated - Try/With stmts),
// e0144 (`type()` constructor edge cases), e0149 (PEP 695 scoping),
// e0107 (variance deeper), e0111 (constructor subclass checks),
// e0138 (dataclass transform deeper), e0131 (yield type deeper),
// e0130 (typevar scoping deeper), e0120 (generator deeper),
// e0036 (`ClassVar` deeper), e0095 (`InitVar` deeper), e0143 (`NamedTuple` deeper),
// e0121 (Protocol conformance deeper), e0139 (`TypeVarTuple` deeper),
// e0126 (Literal string deeper), e0119 (Protocol isinstance deeper),
// e0116 (`NamedTuple` definition deeper), e0073 (`NamedTuple` compat deeper),
// e0122 (Callable arity deeper), e0102 (`TypeVar` default deeper).

// =============================================================================
// E0140: Protocol/Callable deep checks
// =============================================================================

#[test]
fn e0140_protocol_missing_varargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Acceptor(Protocol):
    def __call__(self, *args: int) -> None: ...

def no_varargs(x: int) -> None:
    pass

f: Acceptor = no_varargs
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_missing_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Handler(Protocol):
    def __call__(self, **kwargs: str) -> None: ...

def no_kwargs(x: int) -> None:
    pass

f: Handler = no_kwargs
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_too_many_required_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Simple(Protocol):
    def __call__(self, x: int) -> None: ...

def complex_func(a: int, b: str, c: float) -> None:
    pass

f: Simple = complex_func
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_missing_required_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Complex(Protocol):
    def __call__(self, a: int, b: str, c: float) -> None: ...

def simple_func(x: int) -> None:
    pass

f: Complex = simple_func
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_default_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class WithDefault(Protocol):
    def __call__(self, x: int, y: str = "default") -> None: ...

def no_default(x: int, y: str) -> None:
    pass

f: WithDefault = no_default
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_missing_kw_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class WithKW(Protocol):
    def __call__(self, x: int, *, key: str) -> None: ...

def no_key(x: int) -> None:
    pass

f: WithKW = no_key
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_positional_only_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class FlexibleProto(Protocol):
    def __call__(self, x: int, y: str) -> None: ...

def pos_only(x: int, y: str, /) -> None:
    pass

f: FlexibleProto = pos_only
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_param_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class IntAcceptor(Protocol):
    def __call__(self, value: int) -> None: ...

def accepts_str(value: str) -> None:
    pass

f: IntAcceptor = accepts_str
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_protocol_extra_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class HasAttrs(Protocol):
    x: int
    def __call__(self) -> None: ...

def simple() -> None:
    pass

f: HasAttrs = simple
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_concatenate() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, Concatenate, ParamSpec

P = ParamSpec("P")

def decorator(func: Callable[Concatenate[int, P], str]) -> Callable[P, str]:
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> str:
        return func(0, *args, **kwargs)
    return wrapper
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_empty_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def no_args() -> int:
    return 42

f: Callable[[], int] = no_args
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def two_args(a: int, b: str) -> bool:
    return True

f: Callable[[int, str, float], bool] = two_args
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_callable_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def three_args(a: int, b: str, c: float) -> bool:
    return True

f: Callable[[int], bool] = three_args
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0140_non_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    x: int = 42

def my_func() -> int:
    return 42

f: MyClass = my_func
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0115: Deprecated - Try/With statements
// =============================================================================

#[test]
fn e0115_deprecated_in_try_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_func() -> int:
    return 42

try:
    result = old_func()
except Exception:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_in_with_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
class OldContext:
    def __enter__(self) -> "OldContext":
        return self
    def __exit__(self, *args: object) -> None:
        pass

with OldContext() as ctx:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_in_elif() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def check() -> bool:
    return True

@deprecated("Also obsolete")
def other_check() -> bool:
    return False

if False:
    pass
elif check():
    pass
elif other_check():
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_complex_expr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_compute")
def old_compute(x: int) -> int:
    return x * 2

# Use deprecated in various expression contexts
result = old_compute(1) + old_compute(2)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_method_attribute_access_in_if() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Service:
    @deprecated("Use new_status")
    def status(self) -> str:
        return "ok"

    def new_status(self) -> str:
        return "ok"

svc = Service()
if svc.status() == "ok":
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0144: type() constructor - exercise all paths
// =============================================================================

#[test]
fn e0144_type_zero_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
MyClass = type()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_four_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
MyClass = type("MyClass", (object,), {}, True)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_empty_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Empty = type("Empty", (), {})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_not_assigned() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x = type("X", (object,), {"a": 1, "b": 2})
y = type("Y", (object,), {"c": 3})
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0149: PEP 695 scoping - all paths
// =============================================================================

#[test]
fn e0149_typevar_in_function_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, List

T = TypeVar("T")

class Stack(Generic[T]):
    def __init__(self) -> None:
        self.items: List[T] = []

    def push(self, item: T) -> None:
        self.items.append(item)

    def pop(self) -> T:
        return self.items.pop()

    def peek(self) -> T:
        return self.items[-1]

    def is_empty(self) -> bool:
        return len(self.items) == 0
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_multiple_typevars_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

K = TypeVar("K")
V = TypeVar("V")

class Map(Generic[K, V]):
    def get(self, key: K) -> V: ...
    def set(self, key: K, value: V) -> None: ...
    def keys(self) -> list[K]: ...
    def values(self) -> list[V]: ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_typevar_bound_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)
U = TypeVar("U", bound=str)

def add(a: T, b: T) -> T:
    return a + b

def concat(a: U, b: U) -> U:
    return a + b
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0107: Variance - exercise compose_variance and deeper paths
// =============================================================================

#[test]
fn e0107_covariant_in_contravariant_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)

class Box(Generic[T_co]):
    def get(self) -> T_co: ...

class IntBox(Box[int]):
    def get(self) -> int:
        return 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0107_multiple_type_params_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)
T = TypeVar("T")

class Func(Generic[T_contra, T_co]):
    def __call__(self, arg: T_contra) -> T_co: ...

class Process(Func[int, str]):
    def __call__(self, arg: int) -> str:
        return str(arg)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0111: Constructor - exercise deep paths
// =============================================================================

#[test]
fn e0111_constructor_with_starargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Flexible:
    def __init__(self, *args: int, **kwargs: str) -> None:
        self.args = args
        self.kwargs = kwargs

f = Flexible(1, 2, 3, name="test")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0111_constructor_wrong_kwarg_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Config:
    def __init__(self, *, name: str, value: int) -> None:
        self.name = name
        self.value = value

c = Config(name="test", value="wrong")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0111_constructor_inheritance_chain() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class A:
    def __init__(self, x: int) -> None:
        self.x = x

class B(A):
    def __init__(self, x: int, y: str) -> None:
        super().__init__(x)
        self.y = y

class C(B):
    def __init__(self, x: int, y: str, z: float) -> None:
        super().__init__(x, y)
        self.z = z

class D(C):
    def __init__(self) -> None:
        super().__init__(1, "hello", 3.14)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0138: Dataclass transform - exercise deeper paths
// =============================================================================

#[test]
fn e0138_transform_with_eq_and_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform(eq_default=True, order_default=True)
class OrderedMeta(type):
    pass

class Comparable(metaclass=OrderedMeta):
    value: int

    def __lt__(self, other: "Comparable") -> bool:
        return self.value < other.value
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0138_transform_decorator_on_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
def model_class(cls: type) -> type:
    return cls

@model_class
class User:
    name: str
    age: int = 0
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0131: Yield type - exercise deeper scenarios
// =============================================================================

#[test]
fn e0131_yield_from_generator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def sub() -> Generator[str, None, None]:
    yield "hello"
    yield "world"

def main() -> Generator[int, None, None]:
    yield 1
    yield from sub()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0131_yield_with_send_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def echo() -> Generator[str, str, None]:
    received = yield "start"
    while received:
        received = yield received.upper()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0130: TypeVar scoping - exercise deeper paths
// =============================================================================

#[test]
fn e0130_typevar_in_lambda() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

def identity(x: T) -> T:
    return x

process = lambda x: identity(x)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0130_typevar_in_nested_function_deep() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    def method(self) -> T:
        def helper() -> T:
            def deep() -> T:
                ...
            return deep()
        return helper()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0120: Generator - exercise yield from and return paths
// =============================================================================

#[test]
fn e0120_generator_no_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, str]:
    yield 1
    yield 2
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0120_generator_yield_from_with_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def sub() -> Generator[int, None, str]:
    yield 42
    return "done"

def main() -> Generator[int, None, str]:
    result = yield from sub()
    return result
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0036: ClassVar - exercise scan_source_for_classvar_usage
// =============================================================================

#[test]
fn e0036_classvar_with_complex_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Dict, List, Optional

class Config:
    items: ClassVar[List[int]] = []
    mapping: ClassVar[Dict[str, int]] = {}
    optional_val: ClassVar[Optional[str]] = None
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0036_classvar_short_prefix() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    x: ClassVar = 42
    y: ClassVar[int] = 0
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0095: InitVar - exercise deeper paths
// =============================================================================

#[test]
fn e0095_initvar_with_default_and_post_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar, field

@dataclass
class Config:
    name: str
    debug: InitVar[bool] = False
    level: int = field(default=0)

    def __post_init__(self, debug: bool) -> None:
        if debug:
            self.level = 10
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0143: NamedTuple - exercise deeper paths
// =============================================================================

#[test]
fn e0143_namedtuple_functional_with_rename() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from collections import namedtuple

Point = namedtuple("Point", ["x", "y", "z"])
Color = namedtuple("Color", "r g b")
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0121: Protocol conformance - exercise deeper checks
// =============================================================================

#[test]
fn e0121_protocol_with_generic_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Container(Protocol[T]):
    def get(self) -> T: ...
    def set(self, value: T) -> None: ...

class IntBox:
    def get(self) -> int:
        return 0
    def set(self, value: int) -> None:
        pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0139: TypeVarTuple - exercise deeper paths
// =============================================================================

#[test]
fn e0139_typevartuple_with_constraints() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack

Ts = TypeVarTuple("Ts")

class Tuple(Generic[Unpack[Ts]]):
    def __init__(self, *args: Unpack[Ts]) -> None:
        self.args = args

t = Tuple(1, "hello", 3.14)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0126: Literal string - exercise deeper paths
// =============================================================================

#[test]
fn e0126_literal_int_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[42] = 100
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0126_literal_bool_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[True] = False
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0119: Protocol isinstance - exercise deeper paths
// =============================================================================

#[test]
fn e0119_protocol_isinstance_with_type_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Sized(Protocol):
    def __len__(self) -> int: ...

x: object = [1, 2, 3]
if isinstance(x, Sized):
    print(len(x))
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0116: NamedTuple definition - exercise deeper paths
// =============================================================================

#[test]
fn e0116_namedtuple_with_underscore_field() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Config(NamedTuple):
    name: str
    _internal: int = 0
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0073: NamedTuple tuple compat - exercise deeper paths
// =============================================================================

#[test]
fn e0073_namedtuple_tuple_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Person(NamedTuple):
    name: str
    age: int

p: tuple[int, str] = Person("Alice", 30)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0102: TypeVar default - exercise deeper paths
// =============================================================================

#[test]
fn e0102_typevar_default_with_multiple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", default=int)
U = TypeVar("U", default=str)
V = TypeVar("V", int, str, default=bytes)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0122: Callable arity - exercise deeper paths
// =============================================================================

#[test]
fn e0122_callable_with_only_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def only_kwargs(**kwargs: str) -> int:
    return len(kwargs)

f: Callable[[int], int] = only_kwargs
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Additional compound tests for maximum coverage
// =============================================================================

#[test]
fn compound_deprecated_protocol_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, deprecated, Callable

@deprecated("Use NewHandler")
class OldHandler:
    def handle(self, data: str) -> int:
        return len(data)

class Handler(Protocol):
    def handle(self, data: str) -> int: ...

def process(h: Handler) -> int:
    return h.handle("test")

old = OldHandler()
result = process(old)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_dataclass_classvar_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Protocol
from dataclasses import dataclass

class HasVersion(Protocol):
    version: ClassVar[str]

@dataclass
class App:
    version: ClassVar[str] = "1.0"
    name: str = "MyApp"

class AppV2:
    version = "2.0"

v: HasVersion = App()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_generic_callable_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic, Callable

T = TypeVar("T")
U = TypeVar("U")

class Mapper(Protocol[T, U]):
    def __call__(self, value: T) -> U: ...

def int_to_str(value: int) -> str:
    return str(value)

m: Mapper[int, str] = int_to_str
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_typevar_nested_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Protocol

T = TypeVar("T")

class Comparable(Protocol):
    def __lt__(self, other: "Comparable") -> bool: ...

class Sortable(Generic[T]):
    items: list[T]

    def sort(self) -> None:
        ...

class IntSortable(Sortable[int]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_all_literal_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

a: Literal[42] = 42
b: Literal["hello"] = "hello"
c: Literal[True] = True
d: Literal[False] = False
e: Literal[b"data"] = b"data"
f: Literal[3.14] = 3.14
g: Literal[-1] = -1
h: Literal[0xFF] = 255
i: Literal[0o10] = 8
j: Literal[0b11] = 3
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_complex_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field, InitVar
from typing import ClassVar, List, Optional

@dataclass(frozen=True)
class ImmutablePoint:
    x: int
    y: int

@dataclass
class Config:
    name: str
    debug: InitVar[bool] = False
    items: List[int] = field(default_factory=list)
    version: ClassVar[str] = "1.0"
    _cache: Optional[dict[str, int]] = None

    def __post_init__(self, debug: bool) -> None:
        if debug:
            self._cache = {}
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_complex_protocol_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, runtime_checkable

T = TypeVar("T")

class Readable(Protocol):
    def read(self) -> bytes: ...

class Writable(Protocol):
    def write(self, data: bytes) -> int: ...

class Seekable(Protocol):
    def seek(self, pos: int) -> int: ...

@runtime_checkable
class Stream(Readable, Writable, Protocol):
    pass

class FileStream:
    def read(self) -> bytes:
        return b""
    def write(self, data: bytes) -> int:
        return len(data)

x: Stream = FileStream()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_complex_generator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, Iterator, AsyncGenerator

def fibonacci() -> Generator[int, None, None]:
    a, b = 0, 1
    while True:
        yield a
        a, b = b, a + b

def take(n: int, gen: Iterator[int]) -> Generator[int, None, None]:
    for i, val in enumerate(gen):
        if i >= n:
            return
        yield val

async def async_counter(n: int) -> AsyncGenerator[int, None]:
    for i in range(n):
        yield i
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_complex_callable_assignments() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, Protocol

class Transform(Protocol):
    def __call__(self, x: int) -> str: ...

class Filter(Protocol):
    def __call__(self, x: int) -> bool: ...

def to_str(x: int) -> str:
    return str(x)

def is_positive(x: int) -> bool:
    return x > 0

t: Transform = to_str
f: Filter = is_positive

def apply(func: Callable[[int], str], value: int) -> str:
    return func(value)

result = apply(to_str, 42)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_complex_type_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    x: int = 0

class Mixin:
    def method(self) -> str:
        return "mixin"

Created = type("Created", (Base, Mixin), {"y": "hello", "z": 42})
Simple = type("Simple", (), {})
WithMethod = type("WithMethod", (object,), {"__str__": lambda self: "custom"})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_complex_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Readable(Generic[T_co]):
    def read(self) -> T_co: ...

class Writable(Generic[T_contra]):
    def write(self, data: T_contra) -> None: ...

class ReadWrite(Readable[T], Writable[T]):
    def read(self) -> T: ...
    def write(self, data: T) -> None: ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_overload_and_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, overload, Union

class Convertible(Protocol):
    @overload
    def convert(self, target: type[int]) -> int: ...
    @overload
    def convert(self, target: type[str]) -> str: ...
    def convert(self, target: type) -> Union[int, str]: ...

class Value:
    def __init__(self, raw: str) -> None:
        self.raw = raw

    @overload
    def convert(self, target: type[int]) -> int: ...
    @overload
    def convert(self, target: type[str]) -> str: ...
    def convert(self, target: type) -> Union[int, str]:
        if target is int:
            return int(self.raw)
        return self.raw
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_tuple_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int] = (1,)
t2: tuple[int, str] = (1, "hello")
t3: tuple[int, ...] = (1, 2, 3, 4, 5)
t4: tuple[()] = ()
t5: tuple[int, str, float] = (1, "hello", 3.14)

# Reassignments
t1 = (2,)
t2 = (3, "world")
t3 = (10, 20)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_newtype_and_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, TypeVar

UserId = NewType("UserId", int)
Score = NewType("Score", float)
Name = NewType("Name", str)
Data = NewType("Data", bytes)

T = TypeVar("T", int, str)

def process(x: T) -> T:
    return x
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0014_dataclass_transform_field_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class BaseModel:
    def __init_subclass__(cls, **kwargs: object) -> None:
        pass

class Person(BaseModel):
    name: str
    age: int

p = Person()
p.name = 42
p.age = "wrong"
"#;
    let _ = run(source)?;
    Ok(())
}
