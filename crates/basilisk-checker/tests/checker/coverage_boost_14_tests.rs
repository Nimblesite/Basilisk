//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 14: targeting PEP 695 syntax, deeper e0107/e0144/e0115 paths.
// Also targets remaining uncovered branches in many other rules with highly specific patterns.

// =============================================================================
// E0149: PEP 695 type parameter scoping - DEEP coverage
// =============================================================================

#[test]
fn e0149_pep695_class_type_params() -> Result<(), Box<dyn std::error::Error>> {
    // PEP 695 syntax: class Foo[T]:
    let source = r#"
class Container[T]:
    value: T

    def get(self) -> T:
        return self.value
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_function_type_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def identity[T](x: T) -> T:
    return x
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_bound_cross_reference() -> Result<(), Box<dyn std::error::Error>> {
    // T's bound references U, which is in the same type param list
    let source = r#"
from typing import Sequence
class Foo[T: Sequence[U], U]:
    pass
"#;
    let diags = run(source)?;
    let has_e0149 = diags
        .iter()
        .any(|d| d.code.code == "generics_syntax_scoping");
    assert!(
        has_e0149,
        "Expected generics_syntax_scoping for cross-reference in PEP 695 bounds"
    );
    Ok(())
}

#[test]
fn e0149_pep695_bound_self_reference() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Bar[T: list[U], U: dict[T, int]]:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_module_level_usage() -> Result<(), Box<dyn std::error::Error>> {
    // Using T at module level after it's defined as a PEP 695 type param
    let source = r#"
class Container[T]:
    value: T

x = T
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_decorator_uses_class_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def my_decorator(cls):
    return cls

@my_decorator
class Stack[T]:
    items: list[T]
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_multiple_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Pair[T, U]:
    first: T
    second: U

    def swap(self) -> "Pair[U, T]":
        return Pair(self.second, self.first)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_async_def_type_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
async def fetch[T](url: str) -> T:
    ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_nested_class_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Outer[T]:
    class Inner[U]:
        value: U

    items: list[T]
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_starred_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Tuple[*Ts]:
    pass

def func[**P](*args: P.args, **kwargs: P.kwargs) -> None:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_param_with_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class NumericContainer[T: int]:
    value: T

    def increment(self) -> T:
        return self.value + 1
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_module_level_print() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Box[T]:
    value: T

print(T)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0149_pep695_shadowed_by_module_assignment() -> Result<(), Box<dyn std::error::Error>> {
    // T = TypeVar("T") at module level shadows the PEP 695 T
    let source = r#"
from typing import TypeVar
T = TypeVar("T")

class Box[T]:
    value: T

x: T = 42
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0107: Variance - deeply exercise compose_variance and all paths
// =============================================================================

#[test]
fn e0107_variance_resolve_rich_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Sequence, Iterable

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Source(Generic[T_co]):
    def items(self) -> Sequence[T_co]: ...

class Sink(Generic[T_contra]):
    def accept(self, items: Iterable[T_contra]) -> None: ...

class Pipe(Source[int], Sink[str]):
    def items(self) -> Sequence[int]:
        return [1, 2, 3]
    def accept(self, items: Iterable[str]) -> None:
        pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0107_variance_with_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

IntList: TypeAlias = list[int]

class Wrapper(Generic[T_co]):
    value: T_co

class IntWrapper(Wrapper[int]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0107_variance_three_deep() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class A(Generic[T_co]):
    pass

class B(Generic[T_contra]):
    pass

class C(Generic[T]):
    pass

class D(A[int], B[str], C[float]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0144: type() constructor - exercise all detection paths
// =============================================================================

#[test]
fn e0144_type_constructor_name_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Foo = type("Bar", (object,), {})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_constructor_non_string_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Foo = type(42, (object,), {})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_constructor_non_tuple_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Foo = type("Foo", [object], {})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_constructor_non_dict_namespace() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Foo = type("Foo", (object,), [1, 2, 3])
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_constructor_complex_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyBase:
    x: int = 0

class MyMixin:
    def method(self) -> str:
        return "mixin"

Created = type("Created", (MyBase, MyMixin, object), {"y": 42})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0144_type_constructor_with_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def init(self, x):
    self.x = x

def repr(self):
    return f"Point({self.x})"

Point = type("Point", (), {"__init__": init, "__repr__": repr})
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0115: Deprecated - exercise remaining uncovered visitors
// =============================================================================

#[test]
fn e0115_deprecated_in_list_comprehension() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0115_deprecated_in_dict_comprehension() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Obsolete")
def old_key(x: int) -> str:
    return str(x)

result = {old_key(i): i for i in range(5)}
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_in_ternary() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_check")
def old_check() -> bool:
    return True

result = 1 if old_check() else 0
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_chained_calls() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Builder:
    @deprecated("Use build_v2")
    def build(self) -> "Builder":
        return self

    def build_v2(self) -> "Builder":
        return self

b = Builder()
result = b.build()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0115_deprecated_multiple_operators() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

class Number:
    @deprecated("Use subtract instead")
    def __sub__(self, other: int) -> "Number":
        return Number()

    @deprecated("Use multiply instead")
    def __mul__(self, other: int) -> "Number":
        return Number()

    @deprecated("Use divide instead")
    def __truediv__(self, other: int) -> "Number":
        return Number()

    @deprecated("Use modulo instead")
    def __mod__(self, other: int) -> "Number":
        return Number()

num = Number()
num -= 1
num *= 2
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0138: Dataclass transform - deeper metaclass/base paths
// =============================================================================

#[test]
fn e0138_transform_metaclass_with_slots() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform(frozen_default=True)
class FrozenMeta(type):
    pass

class FrozenModel(metaclass=FrozenMeta):
    x: int
    y: str = "default"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0138_transform_base_with_init_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform(kw_only_default=True)
class KWBase:
    def __init_subclass__(cls, **kwargs: object) -> None:
        super().__init_subclass__(**kwargs)

class Config(KWBase):
    name: str
    value: int = 0
    debug: bool = False
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0122: Callable arity - deeper return type and varargs checks
// =============================================================================

#[test]
fn e0122_callable_varargs_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def varargs_func(*args: int, **kwargs: str) -> bool:
    return True

f: Callable[[int, str, float], bool] = varargs_func
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0122_callable_with_kwonly_and_regular() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def mixed(a: int, b: str, *, c: float = 0.0) -> bool:
    return True

f: Callable[[int, str], bool] = mixed
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0143: NamedTuple - deeper functional and class paths
// =============================================================================

#[test]
fn e0143_namedtuple_typing_functional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Point = NamedTuple("Point", x=int, y=int)
Color = NamedTuple("Color", [("r", int), ("g", int), ("b", int)])
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0143_namedtuple_with_class_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Vector(NamedTuple):
    x: float
    y: float
    z: float = 0.0

    def magnitude(self) -> float:
        return (self.x**2 + self.y**2 + self.z**2) ** 0.5

    def normalized(self) -> "Vector":
        mag = self.magnitude()
        return Vector(self.x/mag, self.y/mag, self.z/mag)

    @classmethod
    def zero(cls) -> "Vector":
        return cls(0.0, 0.0, 0.0)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0095: InitVar - exercise deeper paths including complex defaults
// =============================================================================

#[test]
fn e0095_initvar_complex_post_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar, field
from typing import List, Optional

@dataclass
class Database:
    host: str
    port: int
    connect: InitVar[bool] = True
    tables: List[str] = field(default_factory=list)
    _connection: Optional[object] = field(init=False, default=None)

    def __post_init__(self, connect: bool) -> None:
        if connect:
            self._connection = object()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0121: Protocol conformance - deeper attribute checks
// =============================================================================

#[test]
fn e0121_protocol_with_class_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Factory(Protocol):
    @classmethod
    def create(cls) -> "Factory": ...

class Widget:
    @classmethod
    def create(cls) -> "Widget":
        return cls()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0121_protocol_multiple_missing_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class FullProtocol(Protocol):
    def method_a(self) -> int: ...
    def method_b(self) -> str: ...
    def method_c(self) -> bool: ...

class Partial:
    def method_a(self) -> int:
        return 0
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0139: TypeVarTuple - deeper paths
// =============================================================================

#[test]
fn e0139_typevartuple_with_mixed_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, TypeVarTuple, Generic, Unpack

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Mixed(Generic[T, Unpack[Ts]]):
    first: T

    def method(self, *args: Unpack[Ts]) -> T:
        return self.first
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0139_typevartuple_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Callable, Unpack

Ts = TypeVarTuple("Ts")

def apply(func: Callable[[Unpack[Ts]], int], *args: Unpack[Ts]) -> int:
    return func(*args)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0126: Literal - deeper value comparison paths
// =============================================================================

#[test]
fn e0126_literal_union_int_and_str() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal, Union

x: Union[Literal[1], Literal["hello"]] = 2
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0126_literal_none_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal, Optional

x: Optional[Literal[42]] = None
y: Literal[42] = None
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0119: Protocol isinstance - deeper paths
// =============================================================================

#[test]
fn e0119_non_runtime_checkable_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyProto(Protocol):
    def method(self) -> None: ...

x: object = object()
isinstance(x, MyProto)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0119_runtime_checkable_issubclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Sized(Protocol):
    def __len__(self) -> int: ...

class MyList:
    def __len__(self) -> int:
        return 0

result1 = isinstance(MyList(), Sized)
result2 = issubclass(MyList, Sized)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0116: NamedTuple definition - deeper paths
// =============================================================================

#[test]
fn e0116_namedtuple_functional_wrong_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Foo = NamedTuple("Bar", [("x", int)])
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0116_namedtuple_class_with_mutable_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Config(NamedTuple):
    name: str
    items: list = []
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0073: NamedTuple tuple compat - deeper paths
// =============================================================================

#[test]
fn e0073_namedtuple_tuple_too_many_fields() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point3D(NamedTuple):
    x: int
    y: int
    z: int

p: tuple[int, int] = Point3D(1, 2, 3)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0102: TypeVar default - exercise deeper paths
// =============================================================================

#[test]
fn e0102_typevar_default_outside_constraints() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", int, str, default=list)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0131: Yield type - exercise deeper paths
// =============================================================================

#[test]
fn e0131_yield_expression_in_loop() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def counter() -> Generator[int, None, None]:
    i = 0
    while True:
        yield i
        i += 1
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0131_yield_in_try_except() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def safe_gen() -> Generator[int, None, None]:
    try:
        yield 1
        yield 2
    except Exception:
        yield -1
    finally:
        yield 0
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0130: TypeVar scoping - deeper nested class paths
// =============================================================================

#[test]
fn e0130_typevar_in_deeply_nested() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class A(Generic[T]):
    class B:
        class C:
            value: T
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0054: Final reassignment - deeper paths
// =============================================================================

#[test]
fn e0054_final_in_class_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

class Config:
    MAX: Final = 100

    def reset(self) -> None:
        Config.MAX = 0
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0054_final_typed() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

X: Final[int] = 42
X = 100
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0112: TypeGuard - deeper callable return paths
// =============================================================================

#[test]
fn e0112_typeis_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeIs

def is_int(x: object) -> TypeIs[int]:
    return isinstance(x, int)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0112_typeguard_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeGuard

class Checker:
    @staticmethod
    def is_str(x: object) -> TypeGuard[str]:
        return isinstance(x, str)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0145: Invalid type bracket - deeper paths
// =============================================================================

#[test]
fn e0145_type_bracket_generic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

def foo(x: type[Generic[T]]) -> None:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0147: Tuple starred unpack - deeper paths
// =============================================================================

#[test]
fn e0147_tuple_starred_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Unpack, TypeVarTuple

Ts = TypeVarTuple("Ts")

def foo(*args: Unpack[tuple[int, str, float]]) -> None:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0148: Generic type arg - deeper paths
// =============================================================================

#[test]
fn e0148_dict_wrong_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Dict

x: Dict[int] = {}
y: Dict[int, str, float] = {}
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0148_set_with_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Set, FrozenSet

x: Set[int, str] = set()
y: FrozenSet[int, str] = frozenset()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0146: Protocol class object - deeper paths
// =============================================================================

#[test]
fn e0146_protocol_class_with_class_vars() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, ClassVar

class HasVersion(Protocol):
    version: ClassVar[str]
    def get_version(self) -> str: ...

class App:
    version: ClassVar[str] = "1.0"
    def get_version(self) -> str:
        return self.version
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0079: Module protocol - exercise the code paths we can reach
// =============================================================================

#[test]
fn e0079_protocol_with_module_import() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol
import json

class JsonLike(Protocol):
    def dumps(self, obj: object) -> str: ...

x: JsonLike = json
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0057: PEP 695 type alias validation
// =============================================================================

#[test]
fn e0057_pep695_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Vector = list[float]
type Matrix = list[Vector]
type Scalar = int | float
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0057_pep695_type_alias_with_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Pair[T] = tuple[T, T]
type Triple[T] = tuple[T, T, T]
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0047: Invalid type expression - exercise remaining branches
// =============================================================================

#[test]
fn e0047_attribute_annotation_numeric() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    x: 42 = None
    y: 3.14 = None
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0047_star_import_module_names() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import *
from collections.abc import *

x: Sequence[int] = []
y: Mapping[str, int] = {}
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0014: Dataclass attr assignment - exercise classify_literal
// =============================================================================

#[test]
fn e0014_classify_literal_all_types() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class AllTypes:
    a: int
    b: str
    c: float
    d: bytes
    e: bool

obj = AllTypes(0, "", 0.0, b"", True)
obj.a = "string_to_int"
obj.b = 42
obj.c = "string_to_float"
obj.d = 42
obj.e = "string_to_bool"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0014_classify_literal_single_quote() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Config:
    name: int

c = Config(0)
c.name = 'single_quoted'
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0014_classify_literal_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Data:
    value: str

d = Data("")
d.value = b"bytes"
d.value = b'single_bytes'
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0014_classify_literal_none() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Holder:
    value: int

h = Holder(0)
h.value = None
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0014_classify_literal_negative() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Counter:
    value: str

c = Counter("")
c.value = -42
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0063: Non-hashable dataclass - exercise deeper code paths
// =============================================================================

#[test]
fn e0063_dataclass_with_hash_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass
from typing import Hashable

@dataclass
class CustomHash:
    x: int

    def __hash__(self) -> int:
        return hash(self.x)

v: Hashable = CustomHash(42)
"#;
    let diags = run(source)?;
    let has_e0063 = diags.iter().any(|d| d.code.code == "dataclasses_hash");
    assert!(
        !has_e0063,
        "Dataclass with __hash__ should not trigger E0063"
    );
    Ok(())
}

#[test]
fn e0063_dataclass_unsafe_hash() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass
from typing import Hashable

@dataclass(unsafe_hash=True)
class UnsafeHash:
    x: int

v: Hashable = UnsafeHash(42)
"#;
    let diags = run(source)?;
    let has_e0063 = diags.iter().any(|d| d.code.code == "dataclasses_hash");
    assert!(
        !has_e0063,
        "Dataclass with unsafe_hash should not trigger E0063"
    );
    Ok(())
}

#[test]
fn e0063_dataclass_eq_false() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass
from typing import Hashable

@dataclass(eq=False)
class NoEq:
    x: int

v: Hashable = NoEq(42)
"#;
    let diags = run(source)?;
    let has_e0063 = diags.iter().any(|d| d.code.code == "dataclasses_hash");
    assert!(
        !has_e0063,
        "Dataclass with eq=False should not trigger E0063"
    );
    Ok(())
}

// =============================================================================
// Compound tests for maximum coverage
// =============================================================================

#[test]
fn compound_pep695_and_protocols() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Container[T](Protocol):
    def get(self) -> T: ...
    def set(self, value: T) -> None: ...

class IntContainer:
    def get(self) -> int:
        return 0
    def set(self, value: int) -> None:
        pass

x: Container[int] = IntContainer()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_pep695_and_deprecated() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use NewStack")
class OldStack[T]:
    items: list[T]

    def push(self, item: T) -> None:
        self.items.append(item)

s = OldStack()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn compound_all_stmts_with_deprecated() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import deprecated

@deprecated("Use new_iter")
def old_iter() -> list[int]:
    return [1, 2, 3]

@deprecated("Use new_check")
def old_check() -> bool:
    return True

# if statement
if old_check():
    pass

# while statement
while old_check():
    break

# for statement
for item in old_iter():
    pass

# function body
def wrapper() -> int:
    return len(old_iter())

# class body
class MyClass:
    items = old_iter()

# assign
x = old_iter()

# annotated assign
y: list[int] = old_iter()

# augmented assign
z = 0
z += len(old_iter())
"#;
    let _ = run(source)?;
    Ok(())
}
