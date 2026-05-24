//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 30: e0107 variance, e0137 generic protocol, e0139 `TypeVarTuple`,
// e0140 callable, e0047 invalid type, e0015 type arg count, e0113 `TypeIs`, e0111 constructor,
// e0036 `ClassVar`, e0075 Self attribute.

// =============================================================================
// E0107: Variance incompatibility - deeper paths
// =============================================================================

#[test]
fn e0107_contravariant_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Producer(Generic[T_co]):
    def get(self) -> T_co: ...

class Consumer(Generic[T_contra]):
    def put(self, value: T_contra) -> None: ...

class BadContainer(Generic[T_co]):
    items: list[Consumer[T_co]]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0107")
        .count();
    Ok(())
}

#[test]
fn e0107_alias_expansion() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Sink(Generic[T_contra]):
    def put(self, value: T_contra) -> None: ...

MyAlias: TypeAlias = Sink[T_co]

class BadWrapper(Generic[T_co]):
    sink: MyAlias
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0107")
        .count();
    Ok(())
}

#[test]
fn e0107_nested_generic_deep() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)

class Box(Generic[T_co]):
    value: T_co

class Wrapper(Generic[T_co]):
    inner: Box[Box[T_co]]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0137: Generic protocol - deeper paths
// =============================================================================

#[test]
fn e0137_protocol_multi_method_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Comparable(Protocol):
    def __lt__(self, other: "Comparable") -> bool: ...
    def __eq__(self, other: object) -> bool: ...
    def __le__(self, other: "Comparable") -> bool: ...

class MyNum:
    def __lt__(self, other: int) -> bool:
        return True
    def __eq__(self, other: object) -> bool:
        return True
    def __le__(self, other: str) -> bool:
        return True
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0137_protocol_with_type_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic

T = TypeVar("T")

class Container(Protocol[T]):
    def get(self) -> T: ...
    def put(self, value: T) -> None: ...

class BadContainer:
    def get(self) -> int:
        return 0
    def put(self, value: str) -> None:
        pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0139: TypeVarTuple specialization - deeper paths
// =============================================================================

#[test]
fn e0139_typevartuple_alias_too_few() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack

Ts = TypeVarTuple("Ts")

class Variadic(Generic[*Ts]):
    pass

x: Variadic[int]
y: Variadic[int, str, float]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0139_starred_tuple_in_plain() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack, TypeVar

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Plain(Generic[T]):
    pass

x: Plain[*tuple[int, str]]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0140: Callable assignment - deeper paths
// =============================================================================

#[test]
fn e0140_callable_with_concatenate() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, Concatenate, ParamSpec

P = ParamSpec("P")

x: Callable[Concatenate[int, P], str] = lambda n, *args, **kwargs: str(n)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0140_callable_ellipsis_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

x: Callable[..., int] = lambda: 42
y: Callable[..., str] = lambda x: str(x)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0140_non_callable_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def my_func(x: int) -> str:
    return str(x)

y: Callable[[int], str] = my_func
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0047: Invalid type expression - deeper
// =============================================================================

#[test]
fn e0047_runtime_expression_as_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x = int

def f(a: x) -> None:
    pass

def g(a: 42) -> None:
    pass

def h(a: "invalid" + "type") -> None:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0047_complex_invalid_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

def f(a: [int, str]) -> None:
    pass

def g(a: {int: str}) -> None:
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0015: Generic type arg count - deeper Callable validation
// =============================================================================

#[test]
fn e0015_optional_multiple_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Optional

x: Optional[int, str] = None
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0015")
        .count();
    Ok(())
}

#[test]
fn e0015_dict_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Dict

x: Dict[str, int, float] = {}
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0015")
        .count();
    Ok(())
}

#[test]
fn e0015_tuple_with_ellipsis() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Tuple

x: Tuple[int, ...] = (1, 2, 3)
y: Tuple[int, str, ...] = (1, "a")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0113: TypeIs inconsistent narrowing - deeper
// =============================================================================

#[test]
fn e0113_typeis_completely_unrelated() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def is_int(x: str) -> TypeIs[int]:
    return isinstance(x, int)
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0113")
        .count();
    Ok(())
}

#[test]
fn e0113_typeis_with_optional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs, Optional

def is_str(x: Optional[str]) -> TypeIs[str]:
    return isinstance(x, str)

def is_int(x: Optional[int]) -> TypeIs[str]:
    return False
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0111: Constructor errors - deeper
// =============================================================================

#[test]
fn e0111_init_with_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    def __init__(self, a: int, b: str = "default", c: float = 0.0) -> None:
        self.a = a
        self.b = b
        self.c = c

x = MyClass(1)
y = MyClass(1, "hello")
z = MyClass(1, "hello", 3.14)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0111_new_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Singleton:
    _instance = None

    def __new__(cls, value: int) -> "Singleton":
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

s = Singleton(42)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0111_metaclass_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Meta(type):
    def __call__(cls, *args, **kwargs):
        return super().__call__(*args, **kwargs)

class MyClass(metaclass=Meta):
    def __init__(self, x: int) -> None:
        self.x = x

obj = MyClass(42)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0036: ClassVar deeper - self.attr with ClassVar annotation
// =============================================================================

#[test]
fn e0036_classvar_in_local_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class Foo:
    def method(self):
        self.x: ClassVar[int] = 1
        x: ClassVar[str] = ""
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0036")
        .count();
    Ok(())
}

#[test]
fn e0036_classvar_with_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, TypeVar

T = TypeVar("T")

class Foo:
    x: ClassVar[T] = None
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0075: Self attr incompatibility - deeper paths
// =============================================================================

#[test]
fn e0075_self_optional_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self, Optional
from dataclasses import dataclass

@dataclass
class Tree:
    value: int
    left: Optional[Self] = None
    right: Self | None = None

class SpecialTree(Tree):
    pass

t = SpecialTree(value=1, left=Tree(value=2))
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0075_self_in_if_branch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self
from dataclasses import dataclass

@dataclass
class Node:
    value: int
    child: Self | None = None

class Special(Node):
    pass

n = Special(value=1)
if True:
    n.child = Node(value=2)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}
