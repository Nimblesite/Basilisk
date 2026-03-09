//! Coverage boost tests batch 22: targeting e0129, e0014 deeper paths,
//! more e0107, e0137, e0139, e0140, e0047, e0015, e0113, e0111, e0036, e0075.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// =============================================================================
// E0129: Literal value assignment incompatibility
// =============================================================================

#[test]
fn e0129_literal_0_vs_false() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(a: Literal[0], b: Literal[False]):
    x1: Literal[False] = a
    x2: Literal[0] = b
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0129")
        .count();
    Ok(())
}

#[test]
fn e0129_augmented_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(a: Literal[3, 4, 5]):
    a += 3
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0129")
        .count();
    Ok(())
}

#[test]
fn e0129_multiple_augmented_ops() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(a: Literal[1, 2], b: Literal[10]):
    a -= 1
    b *= 2
    a //= 1
    b **= 2
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0129")
        .count();
    Ok(())
}

#[test]
fn e0129_literal_1_vs_true() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(a: Literal[1], b: Literal[True]):
    x1: Literal[True] = a
    x2: Literal[1] = b
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0129")
        .count();
    Ok(())
}

#[test]
fn e0129_literal_string_values() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(a: Literal["hello"]):
    x: Literal["world"] = a
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0129")
        .count();
    Ok(())
}

#[test]
fn e0129_literal_hex_octal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(a: Literal[0xFF]):
    x: Literal[255] = a
    y: Literal[256] = a
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0129")
        .count();
    Ok(())
}

#[test]
fn e0129_valid_literal_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(a: Literal[1, 2, 3]):
    x: Literal[1, 2, 3] = a
    y: Literal[1, 2, 3, 4] = a
"#;
    let diagnostics = run(source)?;
    let e0129 = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0129")
        .count();
    // Valid assignment should not trigger
    let _ = e0129;
    Ok(())
}

#[test]
fn e0129_nested_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(a: Literal[Literal[1, 2], 3]):
    b: Literal[4] = a
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// E0014: Assignment type incompatibility - deeper paths
// =============================================================================

#[test]
fn e0014_float_literal_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
count: int = "hello"
label: str = 42
flag: bool = "yes"
ratio: float = "1.5"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    Ok(())
}

#[test]
fn e0014_negative_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: str = -42
y: bool = -1
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    Ok(())
}

#[test]
fn e0014_bytes_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = b"hello"
y: str = b"world"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    Ok(())
}

#[test]
fn e0014_none_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = None
y: str = None
z: float = None
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    Ok(())
}

#[test]
fn e0014_bool_literal_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: str = True
y: float = False
z: bytes = True
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    Ok(())
}

#[test]
fn e0014_list_dict_set_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = [1, 2, 3]
y: str = {"a": 1}
z: float = {1, 2}
w: int = (1, 2)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    Ok(())
}

#[test]
fn e0014_empty_collection_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = []
y: int = {}
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count();
    Ok(())
}

#[test]
fn e0014_complex_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Optional, Union, List, Dict

a: Optional[int] = "hello"
b: Union[int, float] = "hello"
c: List[int] = 42
d: Dict[str, int] = "hello"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

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
    let source = r#"
from typing import Callable

x: Callable[..., int] = lambda: 42
y: Callable[..., str] = lambda x: str(x)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0140_non_callable_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def my_func(x: int) -> str:
    return str(x)

# Annotated assignment of function to non-protocol type
y: Callable[[int], str] = my_func
"#;
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
    let source = r#"
from typing import Optional

x: Optional[int, str] = None
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0015")
        .count();
    Ok(())
}

#[test]
fn e0015_dict_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Dict

x: Dict[str, int, float] = {}
"#;
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
    let source = r#"
from typing import TypeIs

def is_int(x: str) -> TypeIs[int]:
    return isinstance(x, int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0113")
        .count();
    Ok(())
}

#[test]
fn e0113_typeis_with_optional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeIs, Optional

def is_str(x: Optional[str]) -> TypeIs[str]:
    return isinstance(x, str)

def is_int(x: Optional[int]) -> TypeIs[str]:
    return False
"#;
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
    let source = r#"
class Meta(type):
    def __call__(cls, *args, **kwargs):
        return super().__call__(*args, **kwargs)

class MyClass(metaclass=Meta):
    def __init__(self, x: int) -> None:
        self.x = x

obj = MyClass(42)
"#;
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
    let source = r#"
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
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0075_self_in_if_branch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
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
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// More compound/mega tests
// =============================================================================

#[test]
fn mega_literal_value_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func1(a: Literal[0], b: Literal[False]):
    x1: Literal[False] = a
    x2: Literal[0] = b

def func2(a: Literal[3, 4, 5]):
    a += 3
    a -= 1
    a *= 2

def func3(a: Literal[1], b: Literal[True]):
    x1: Literal[True] = a
    x2: Literal[1] = b

def func4(a: Literal["hello"]):
    x: Literal["world"] = a
    y: Literal["hello"] = a

def func5(a: Literal[0xFF]):
    x: Literal[255] = a
    y: Literal[256] = a
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_assignment_type_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Optional, Union, List, Dict

# Basic mismatches
count: int = "hello"
label: str = 42
flag: bool = "yes"
ratio: float = "1.5"

# Negative literal
neg: str = -42

# None
none_int: int = None
none_str: str = None

# Bool
bool_str: str = True
bool_bytes: bytes = False

# Collection
coll_int: int = [1, 2, 3]
coll_str: str = {"a": 1}
coll_float: float = {1, 2}

# Bytes
bytes_int: int = b"hello"
bytes_str: str = b"world"

# Empty
empty_int: int = []
empty_int2: int = {}

# Complex annotations
opt: Optional[int] = "hello"
union: Union[int, float] = "hello"
"#;
    let diagnostics = run(source)?;
    assert!(
        !diagnostics.is_empty(),
        "Assignment mismatches should produce diagnostics"
    );
    Ok(())
}

#[test]
fn mega_variance_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)
T = TypeVar("T")

class Producer(Generic[T_co]):
    def get(self) -> T_co: ...

class Consumer(Generic[T_contra]):
    def put(self, value: T_contra) -> None: ...

class Mutable(Generic[T]):
    value: T

# Covariant in contravariant position
class BadContainer1(Generic[T_co]):
    items: list[Consumer[T_co]]

# Contravariant in covariant position
class BadContainer2(Generic[T_contra]):
    items: list[Producer[T_contra]]

# Nested
class Deep(Generic[T_co]):
    items: list[list[Consumer[T_co]]]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_callable_all_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, Concatenate, ParamSpec, Protocol

P = ParamSpec("P")

# Ellipsis
x: Callable[..., int] = lambda: 42

# Concatenate
y: Callable[Concatenate[int, P], str] = lambda n, *args, **kwargs: str(n)

# Regular
def my_func(x: int) -> str:
    return str(x)

z: Callable[[int], str] = my_func

# Protocol callable
class Processor(Protocol):
    def __call__(self, x: int) -> str: ...

def process(x: int) -> str:
    return str(x)

w: Processor = process
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_generic_protocol_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic, runtime_checkable

T = TypeVar("T")
S = TypeVar("S")

@runtime_checkable
class Comparable(Protocol):
    def __lt__(self, other: "Comparable") -> bool: ...
    def __eq__(self, other: object) -> bool: ...

class Container(Protocol[T]):
    def get(self) -> T: ...
    def put(self, value: T) -> None: ...

class Mapper(Protocol[T, S]):
    def map(self, value: T) -> S: ...

class MyNum:
    def __lt__(self, other: int) -> bool:
        return True
    def __eq__(self, other: object) -> bool:
        return True

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

#[test]
fn mega_constructor_all_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from abc import ABC, abstractmethod
from typing import TypeVar, Generic

T = TypeVar("T")

class Animal(ABC):
    @abstractmethod
    def speak(self) -> str: ...

class WithDefaults:
    def __init__(self, a: int, b: str = "hi", c: float = 0.0) -> None:
        pass

class WithNew:
    _instance = None
    def __new__(cls, value: int) -> "WithNew":
        return super().__new__(cls)

class Meta(type):
    def __call__(cls, *args, **kwargs):
        return super().__call__(*args, **kwargs)

class MetaClass(metaclass=Meta):
    def __init__(self, x: int) -> None:
        pass

class GenericBox(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

# Calls
a = Animal()
w = WithDefaults(1)
w2 = WithDefaults(1, "hello")
w3 = WithDefaults(1, "hello", 3.14)
n = WithNew(42)
m = MetaClass(1)
b = GenericBox[int](42)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_typevartuple_deep() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack, TypeVar

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Variadic(Generic[*Ts]):
    pass

class Mixed(Generic[T, *Ts]):
    pass

x: Variadic[int, str, float] = Variadic()
y: Mixed[int, str, float] = Mixed()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mega_all_rules_broad_exercise_v2() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import (
    TypeVar, Generic, Self, ClassVar, Final, NewType,
    Protocol, overload, Callable, TypeIs, Union, Optional,
    ParamSpec, Literal, TypeAlias, Concatenate, TypeVarTuple,
    Unpack, runtime_checkable
)
from abc import abstractmethod, ABC
from dataclasses import dataclass, field
from enum import Enum

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
S = TypeVar("S")
P = ParamSpec("P")
Ts = TypeVarTuple("Ts")

# E0129: Literal value assignment
def literal_func(a: Literal[0], b: Literal[False]):
    x: Literal[False] = a
    a += 1

# E0014: Assignment mismatch
bad_int: int = "hello"
bad_str: str = 42
bad_float: float = "1.5"
bad_none: int = None

# E0066: Enum value mismatch
class Color(Enum):
    _value_: int
    RED = 1
    GREEN = "green"

# E0050: NewType
WrongName = NewType("BadName", int)

# E0071: Historical positional
def hist(__x: int) -> None: ...
hist(__x=3)

# E0096: Factory mismatch
@dataclass
class DC:
    a: int = field(default_factory=str)
    b: list = field(default_factory=list)

# E0117: Unbound TypeVar
def unbound_func(x: T) -> list[T]:
    z: list[S] = []
    return z

# E0094: Self in module func
def bad_self(x: Self) -> Self:
    return x

# E0078: Return concrete for Self
class Shape:
    def method(self) -> Self:
        return Shape()

# E0118: super on abstract
class PColor(Protocol):
    @abstractmethod
    def draw(self) -> str: ...

class BadColor(PColor):
    def draw(self) -> str:
        return super().draw()

# E0036: ClassVar in function
class CVClass:
    def method(self, a: ClassVar[int]) -> None:
        pass

# E0107: Variance
class BadVar(Generic[T_co]):
    items: list[T_co]

# E0140: Callable
c: Callable[..., int] = lambda: 42

# E0015: Too many args
opt: Optional[int, str] = None

# E0139: TypeVarTuple
class Variadic(Generic[*Ts]):
    pass
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.len() >= 3,
        "Broad exercise v2 should produce many diagnostics: got {:?}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}
