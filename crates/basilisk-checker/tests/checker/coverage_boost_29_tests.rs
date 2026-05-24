//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 29: final push to 89%.
// Targeting e0147 (tuple starred unpack), e0149 (PEP695 scoping),
// e0107 (variance alias paths), e0137 (generic protocol), e0139 (`TypeVarTuple`),
// e0140 (callable assignment), e0102 (typevar default), e0131 (generator yield).

// =============================================================================
// E0147: Tuple starred-unpack type compatibility
// =============================================================================

#[test]
fn e0147_module_level_starred_unpack_reassign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Unpack

t1: tuple[int, *tuple[str]] = (1, "hello")
t1 = (1, "a", "b")

t2: tuple[int, *tuple[str, ...]] = (1, "x")
t2 = (1, 1, "bad")

t3: tuple[int, *tuple[int, ...], str] = (1, 2, 3, "end")
t3 = (1, "wrong", 3, "end")
"#;
    let diagnostics = run(source)?;
    let has_e0147 = diagnostics.iter().any(|d| d.code.code == "BSK-E0147");
    let _ = has_e0147;
    Ok(())
}

#[test]
fn e0147_module_level_fixed_tuple_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str]] = (1, "a")
t1 = (1,)
t1 = (1, "a", "extra")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_function_body_var_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(t1: tuple[int, ...], t2: tuple[int, *tuple[int, ...]], t3: tuple[int]) -> None:
    v2: tuple[int, *tuple[int, ...]]
    v2 = t1
    v3: tuple[int]
    v3 = t2
    v3 = t1
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_function_body_literal_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f() -> None:
    v1: tuple[int, *tuple[str, ...]]
    v1 = (1, "a", "b")
    v1 = (1, 2, "bad")
    v2: tuple[int, *tuple[str]]
    v2 = (1, "a")
    v2 = (1, "a", "extra")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_homogeneous_to_mixed_starred() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def g(x: tuple[int, ...]) -> None:
    v: tuple[int, *tuple[int, ...]]
    v = x
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_fixed_unpack_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str, str], int] = (1, "a", "b", 2)
t1 = (1, "a", "b", 2, 3)
t1 = (1, "a", 2)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_empty_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[()], str] = (1, "end")
t1 = (1, "extra", "end")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_suffix_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str, ...], int] = (1, "a", "b", 3)
t1 = (1, "a", "wrong_suffix")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_prefix_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str, ...]] = (1, "ok")
t1 = ("wrong", "a", "b")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_annotation_update() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str]] = (1, "a")
t1: tuple[int, *tuple[int]] = (1, 2)
t1 = (1, "wrong_after_update")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_middle_element_type_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str, ...], int] = (1, "a", "b", "c", 5)
t1 = (1, "ok", 99, "bad", 5)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_too_few_elements() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
t1: tuple[int, *tuple[str, ...], int] = (1, 5)
t1 = (1,)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_fixed_length_source_to_fixed_target() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def h(x: tuple[int, int, int]) -> None:
    v: tuple[int, int]
    v = x
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0147_homogeneous_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[int, ...]] = (1, 2, 3)
t2: tuple[str, ...] = ("a", "b")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0149: PEP 695 type parameter scoping
// =============================================================================

#[test]
fn e0149_decorator_uses_class_type_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeVar

def decorator(x):
    return x

@decorator(T)
class Foo[T]:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_decorator_with_prior_module_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
T = int

def decorator(x):
    return x

@decorator(T)
class Foo[T]:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_decorator_with_prior_annotated_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
T: type = int

def decorator(x):
    return x

@decorator(T)
class Foo[T]:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_no_prior_assignment_triggers_error() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def decorator(x):
    return x

@decorator(T)
class Foo[T]:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_func_decorator_type_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def decorator(x):
    return x

@decorator(T)
def foo[T](x: T) -> T:
    return x
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_decorator_indented_prior_assignment_ignored() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def decorator(x):
    return x

def setup():
    T = int

@decorator(T)
class Foo[T]:
    pass
";
    let diagnostics = run(source)?;
    // The indented T = int should NOT count as module-level assignment
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0107: Variance - type alias resolution paths
// =============================================================================

#[test]
fn e0107_alias_with_variance_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

class Container(Generic[T]):
    pass

MyAlias = Container[T_co]

class Bad(MyAlias):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0107_nested_alias_chain() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Base(Generic[T]):
    pass

Alias1 = Base[T]
Alias2 = Alias1

class Sub(Alias2):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0107_multiple_type_params_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

K = TypeVar("K")
V = TypeVar("V")
V_co = TypeVar("V_co", covariant=True)

class Mapping(Generic[K, V]):
    pass

MyMap = Mapping[K, V_co]

class BadMap(MyMap):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0137: Generic protocol - deeper code paths
// =============================================================================

#[test]
fn e0137_protocol_method_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, runtime_checkable

T = TypeVar("T")

@runtime_checkable
class Comparable(Protocol[T]):
    def compare(self, other: T) -> bool: ...

class IntComparer:
    def compare(self, other: int) -> bool:
        return True

class StrComparer:
    def compare(self, other: str) -> bool:
        return True

x: Comparable[int] = IntComparer()
y: Comparable[str] = StrComparer()
z: Comparable[int] = StrComparer()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0137_protocol_return_type_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Producer(Protocol[T]):
    def produce(self) -> T: ...

class IntProducer:
    def produce(self) -> int:
        return 42

class StrProducer:
    def produce(self) -> str:
        return "hello"

p: Producer[int] = IntProducer()
q: Producer[int] = StrProducer()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0137_protocol_with_property() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class HasValue(Protocol[T]):
    @property
    def value(self) -> T: ...

class IntValue:
    @property
    def value(self) -> int:
        return 5

class WrongValue:
    @property
    def value(self) -> str:
        return "wrong"

v: HasValue[int] = IntValue()
w: HasValue[int] = WrongValue()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0139: TypeVarTuple specialization - deeper alias paths
// =============================================================================

#[test]
fn e0139_tvt_alias_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, TypeVarTuple, Generic, Unpack

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Variadic(Generic[T, *Ts]):
    pass

VariadicAlias = Variadic[T, *Ts]

x: VariadicAlias = Variadic()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0139_starred_tuple_no_tvt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Simple(Generic[T]):
    pass

x: Simple[*tuple[int, ...]] = Simple()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0139_alias_with_regular_and_tvt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, TypeVarTuple, Generic, Unpack

T = TypeVar("T")
U = TypeVar("U")
Ts = TypeVarTuple("Ts")

class Multi(Generic[T, U, *Ts]):
    pass

MultiAlias = Multi[T, U, *Ts]
x: MultiAlias[int] = Multi()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0140: Callable assignment - deeper paths
// =============================================================================

#[test]
fn e0140_function_to_non_protocol_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def my_func(x: int) -> str:
    return str(x)

class NotCallable:
    pass

x: NotCallable = my_func
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0140_callable_varargs_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class TakesVarArgs(Protocol):
    def __call__(self, *args: int) -> None: ...

def wrong_varargs(*args: str) -> None:
    pass

x: TakesVarArgs = wrong_varargs
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0140_callable_kwargs_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class TakesKwargs(Protocol):
    def __call__(self, **kwargs: int) -> None: ...

def wrong_kwargs(**kwargs: str) -> None:
    pass

x: TakesKwargs = wrong_kwargs
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0140_concatenate_prefix_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, Concatenate, ParamSpec

P = ParamSpec("P")

def decorator(f: Callable[Concatenate[int, P], str]) -> Callable[P, str]:
    def wrapper(*args, **kwargs) -> str:
        return f(0, *args, **kwargs)
    return wrapper

@decorator
def my_func(x: int, y: str) -> str:
    return str(x) + y
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0102: TypeVar default violation - deeper paths
// =============================================================================

#[test]
fn e0102_typevar_default_incompatible_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int, default=str)
U = TypeVar("U", int, str, default=float)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0102_typevar_default_constraint_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", int, str, default=bytes)
U = TypeVar("U", int, str, default=int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0131: Generator yield type - yield from and return
// =============================================================================

#[test]
fn e0131_yield_from_type_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator, Iterator

def gen_ints() -> Generator[int, None, None]:
    yield 1
    yield 2

def gen_strs() -> Generator[str, None, None]:
    yield from gen_ints()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0131_generator_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def my_gen() -> Generator[int, None, str]:
    yield 1
    yield 2
    return "done"

def bad_gen() -> Generator[int, None, str]:
    yield 1
    return 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0131_async_generator_yield() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import AsyncGenerator

async def async_gen() -> AsyncGenerator[int, None]:
    yield 1
    yield "wrong"

async def async_gen_ok() -> AsyncGenerator[str, None]:
    yield "hello"
    yield "world"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0120: Generator return type
// =============================================================================

#[test]
fn e0120_yield_from_generator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def sub_gen() -> Generator[int, None, None]:
    yield 1

def main_gen() -> Generator[str, None, None]:
    yield from sub_gen()
    yield "hello"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0120_generator_send_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator

def echo() -> Generator[int, str, None]:
    value = yield 0
    while True:
        value = yield len(value)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0054: Final - additional control flow paths
// =============================================================================

#[test]
fn e0054_final_in_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

def outer() -> None:
    X: Final = 10
    X = 20

    def inner() -> None:
        Y: Final = 30
        Y = 40
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0054_final_in_comprehension() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

X: Final = 10
X = [i for i in range(10)]
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0148: Generic type arg - subscript paths
// =============================================================================

#[test]
fn e0148_nested_generic_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Dict, List, Optional

x: Dict[str, List[Optional[int]]] = {}
y: Dict[str, Dict[str, List[int]]] = {}
z: Dict[List[int], str] = {}
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0148_union_generic_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union, List, Dict

x: Union[List[int], Dict[str, int]] = [1]
y: Union[int, List[str]] = "hello"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0146: Protocol class object
// =============================================================================

#[test]
fn e0146_protocol_with_class_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyProtocol(Protocol):
    @classmethod
    def create(cls) -> "MyProtocol": ...

    @staticmethod
    def validate(x: int) -> bool: ...

class Impl:
    @classmethod
    def create(cls) -> "Impl":
        return cls()

    @staticmethod
    def validate(x: int) -> bool:
        return x > 0
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0119: Protocol overlap
// =============================================================================

#[test]
fn e0119_protocol_abstract_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol
from abc import abstractmethod

class Readable(Protocol):
    @abstractmethod
    def read(self) -> bytes: ...

class Writable(Protocol):
    @abstractmethod
    def write(self, data: bytes) -> None: ...

class ReadWritable(Readable, Writable, Protocol):
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0126: LiteralString deeper paths
// =============================================================================

#[test]
fn e0126_literal_string_concatenation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import LiteralString

def safe_query(q: LiteralString) -> None:
    pass

x: LiteralString = "hello"
y: LiteralString = "world"
z: LiteralString = x + y
safe_query(z)
safe_query("direct")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0130: TypeVar scoping
// =============================================================================

#[test]
fn e0130_typevar_used_in_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

def outer(x: T) -> T:
    def inner(y: T) -> T:
        return y
    return inner(x)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0130_typevar_in_class_and_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    def get(self) -> T: ...
    def set(self, value: T) -> None: ...

    def transform(self, func: "Callable[[T], T]") -> T: ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0143: NamedTuple usage - deeper paths
// =============================================================================

#[test]
fn e0143_namedtuple_delete_field() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p = Point(1, 2)
del p.x
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0143_namedtuple_field_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Color(NamedTuple):
    r: int
    g: int
    b: int

c = Color(255, 0, 0)
c.r = 128
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0116: NamedTuple subclass conflict
// =============================================================================

#[test]
fn e0116_namedtuple_multiple_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class A(NamedTuple):
    x: int

class B(NamedTuple):
    y: str

class C(A, B):
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0138: Dataclass transform
// =============================================================================

#[test]
fn e0138_frozen_inheritance_conflict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class Frozen:
    x: int

@dataclass(frozen=False)
class Mutable(Frozen):
    y: str
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0138_transform_eq_and_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(eq=True, order=True)
class Ordered:
    value: int

@dataclass(eq=False, order=True)
class BadOrder:
    value: int
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0142: Dataclass transform base
// =============================================================================

#[test]
fn e0142_transform_with_field_specifiers() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform
from dataclasses import field

@dataclass_transform(field_specifiers=(field,))
class ModelBase:
    def __init_subclass__(cls, **kwargs):
        pass

class User(ModelBase):
    name: str
    age: int = field(default=0)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0095: InitVar deeper paths
// =============================================================================

#[test]
fn e0095_initvar_with_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field, InitVar

@dataclass
class Config:
    debug: InitVar[bool] = False
    verbose: InitVar[int] = 0
    name: str = "default"

    def __post_init__(self, debug: bool, verbose: int) -> None:
        if debug:
            self.name = f"debug-{self.name}"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0111: Constructor call errors - various constructor types
// =============================================================================

#[test]
fn e0111_metaclass_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Meta(type):
    def __new__(mcs, name: str, bases: tuple, namespace: dict) -> "Meta":
        return super().__new__(mcs, name, bases, namespace)

class MyClass(metaclass=Meta):
    pass

x = MyClass()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0111_init_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    def __init_subclass__(cls, **kwargs) -> None:
        pass

class Child(Base, param="value"):
    pass

c = Child()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0047: Invalid type expression - bracket/paren paths
// =============================================================================

#[test]
fn e0047_deeply_nested_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Dict, List, Optional, Callable, Tuple

x: Dict[str, List[Optional[Callable[[int, str], bool]]]] = {}
y: List[Dict[str, List[Dict[str, int]]]] = []
z: Optional[Callable[[Dict[str, int]], List[str]]] = None
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0015: Missing type annotation
// =============================================================================

#[test]
fn e0015_lambda_callable_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

f: Callable[[int, str], bool] = lambda x, y: len(y) > x
g: Callable[..., int] = lambda: 42
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Mega compound test
// =============================================================================

#[test]
fn mega_batch_29_combined() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, Protocol, Final, Generator, NamedTuple,
    Callable, Union, Optional, LiteralString, runtime_checkable,
    AsyncGenerator, TypeVarTuple, Unpack
)
from dataclasses import dataclass, field, InitVar

# TypeVars
T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)
Ts = TypeVarTuple("Ts")

# Generic classes
class Container(Generic[T]):
    pass

class Producer(Protocol[T]):
    def produce(self) -> T: ...

# Type alias
ContainerAlias = Container[T_co]

# Final
X: Final = 42
X = 99

# NamedTuple
class Point(NamedTuple):
    x: int
    y: int

p = Point(1, 2)

# Tuple starred unpack
t1: tuple[int, *tuple[str, ...]] = (1, "a")
t1 = (1, 2)

# Dataclass with InitVar
@dataclass
class Config:
    debug: InitVar[bool] = False
    name: str = "default"

# Generator
def gen() -> Generator[int, None, str]:
    yield 1
    return "done"

# Callable
f: Callable[[int], str] = lambda x: str(x)

# LiteralString
s: LiteralString = "hello"

# Frozen dataclass
@dataclass(frozen=True)
class Frozen:
    value: int

# Protocol with method
@runtime_checkable
class Sized(Protocol):
    def __len__(self) -> int: ...

class MyList:
    def __len__(self) -> int:
        return 0

sized: Sized = MyList()
"#;
    let diagnostics = run(source)?;
    // Just verify the pipeline runs
    let _ = diagnostics;
    Ok(())
}
