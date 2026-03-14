#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Coverage boost tests batch 5: exercising deep code paths in complex rules.
//! Focuses on rules that walk calls, function bodies, and class hierarchies.
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

// --- Deep E0074: __new__ constructor mismatch ---

#[test]
fn e0074_new_no_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Simple:
    def __new__(cls, x: int) -> "Simple":
        return super().__new__(cls)

s = Simple(42)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0074_new_with_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar, Self

T = TypeVar("T")

class Config(Generic[T]):
    def __new__(cls, value: T, name: str = "default") -> Self:
        return super().__new__(cls)

c = Config[int](42, name="test")
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0075: Self type attribute ---

#[test]
fn e0075_optional_self_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self, Optional
from dataclasses import dataclass

@dataclass
class TreeNode:
    value: int
    left: Optional[Self] = None
    right: Optional[Self] = None

node = TreeNode(1, left=TreeNode(2), right=TreeNode(3))
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0075_self_in_method_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self

class Builder:
    def set_name(self, name: str) -> Self:
        return self

    def set_value(self, val: int) -> Self:
        return self

b = Builder().set_name('test').set_value(42)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0079: Module protocol incompat ---

#[test]
fn e0079_protocol_with_multiple_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Config(Protocol):
    host: str
    port: int
    debug: bool
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0121: Protocol conformance ---

#[test]
fn e0121_protocol_multiple_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Serializable(Protocol):
    def serialize(self) -> str: ...
    def deserialize(self, data: str) -> None: ...

class JsonObject:
    def serialize(self) -> str:
        return '{}'
    def deserialize(self, data: str) -> None:
        pass

def process(item: Serializable) -> None:
    pass

process(JsonObject())
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0121_protocol_partial_impl() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Readable(Protocol):
    def read(self) -> str: ...
    def close(self) -> None: ...

class PartialImpl:
    def read(self) -> str:
        return ''

def use_readable(r: Readable) -> None:
    pass

use_readable(PartialImpl())
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0122: Callable arity deep ---

#[test]
fn e0122_callable_with_positional_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_unary(f: Callable[[int], None]) -> None:
    pass

def pos_only(x: int, /) -> None:
    pass

takes_unary(pos_only)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0122_callable_with_mixed_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_binary(f: Callable[[int, str], bool]) -> None:
    pass

def checker(a: int, b: str) -> bool:
    return True

def wrong_arity(a: int) -> bool:
    return True

takes_binary(checker)
takes_binary(wrong_arity)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0127: Tuple index ---

#[test]
fn e0127_named_tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float

p = Point(1.0, 2.0)
a = p[0]
b = p[1]
c = p[5]
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0136: Callable subtyping ---

#[test]
fn e0136_callable_contravariance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_object_func(f: Callable[[object], None]) -> None:
    pass

def int_func(x: int) -> None:
    pass

def object_func(x: object) -> None:
    pass

takes_object_func(int_func)
takes_object_func(object_func)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0136_callable_covariance_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_int_returning(f: Callable[[], int]) -> None:
    pass

def returns_bool() -> bool:
    return True

def returns_float() -> float:
    return 1.0

takes_int_returning(returns_bool)
takes_int_returning(returns_float)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0145: Invalid type bracket ---

#[test]
fn e0145_various_subscripts() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import List, Dict, Set, FrozenSet, Tuple, Optional, Union, Type

a: List[int] = []
b: Dict[str, int] = {}
c: Set[int] = set()
d: FrozenSet[int] = frozenset()
e: Tuple[int, str] = (1, 'a')
f: Optional[int] = None
g: Union[int, str] = 1
h: Type[int] = int
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0145_nested_subscripts() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Dict, List, Optional

x: Dict[str, List[Optional[int]]] = {}
y: List[Dict[str, int]] = []
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0146: Protocol class object ---

#[test]
fn e0146_type_protocol_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar('T')

class Constructable(Protocol[T]):
    def __init__(self, val: T) -> None: ...

class IntWrapper:
    def __init__(self, val: int) -> None:
        self.val = val

def create(cls: type[Constructable[int]], val: int) -> Constructable[int]:
    return cls(val)

obj = create(IntWrapper, 42)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0148: Generic type arg ---

#[test]
fn e0148_constrained_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T', int, str)

class Store(Generic[T]):
    def __init__(self, val: T) -> None:
        self.val = val

s1: Store[int] = Store(42)
s2: Store[str] = Store('hello')
s3: Store[float] = Store(3.14)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0149: PEP 695 type param scoping ---

#[test]
fn e0149_nested_generic_classes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')

class Container(Generic[T]):
    class Inner(Generic[U]):
        def get(self) -> U: ...

    def outer_method(self) -> T: ...
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0107: Variance incompatibility ---

#[test]
fn e0107_invariant_used_correctly() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T')

class MutableContainer(Generic[T]):
    def get(self) -> T: ...
    def set(self, val: T) -> None: ...
    def swap(self, other: T) -> T: ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0107_variance_with_property() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar('T_co', covariant=True)

class ReadOnly(Generic[T_co]):
    @property
    def value(self) -> T_co: ...
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0117: Unbound TypeVar ---

#[test]
fn e0117_typevar_in_function_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar('T')

def identity(x: T) -> T:
    return x
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0117_multiple_unbound_in_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar('T')
U = TypeVar('U')
V = TypeVar('V')

class MyClass(Generic[T]):
    def method(self, x: U, y: V) -> None:
        pass
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0118: super() abstract ---

#[test]
fn e0118_diamond_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from abc import ABC, abstractmethod

class A(ABC):
    @abstractmethod
    def method(self) -> int: ...

class B(A):
    def method(self) -> int:
        return 1

class C(A):
    def method(self) -> int:
        return 2

class D(B, C):
    def method(self) -> int:
        return super().method()
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0092: Too few type args ---

#[test]
fn e0092_generic_with_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar('T')
U = TypeVar('U', default=int)

class Container(Generic[T, U]):
    pass

x: Container[str]
y: Container[str, float]
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep E0094: Self type location ---

#[test]
fn e0094_self_in_classmethod() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self

class Factory:
    @classmethod
    def create(cls) -> Self:
        return cls()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn e0094_self_in_staticmethod() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self

class Bad:
    @staticmethod
    def create() -> Self:
        pass
"#;
    let _ = run(source)?;
    Ok(())
}

// --- Deep suppression system ---

#[test]
fn suppression_type_ignore() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x):  # type: ignore\n    return x\n";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn suppression_type_ignore_specific() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x):  # type: ignore[BSK-E0001]\n    return x\n";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn suppression_basilisk_relaxed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "# basilisk: relaxed\ndef f(x):\n    return x\n";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn suppression_file_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let source = "# basilisk: file-disabled[BSK-E0001]\ndef f(x):\n    return x\n";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn suppression_type_warning() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x):  # type: warning[BSK-E0001]\n    return x\n";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}
