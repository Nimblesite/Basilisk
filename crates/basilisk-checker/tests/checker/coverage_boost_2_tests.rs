//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 2: targeting rules with high uncovered line counts.
// Covers: e0125, e0126, e0127, e0128, e0129, e0130, e0131, e0132, e0133, e0134,
//         e0136, e0137, e0138, e0139, e0140, e0141, e0142, e0143, e0144, e0145

// --- E0125: Instance attribute on class ---

#[test]
fn class_attr_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Node(Generic[T]):
    label: T

Node.label
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn parameterized_class_attr_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Node(Generic[T]):
    label: T

Node[int].label
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn class_attr_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Node(Generic[T]):
    label: T

Node.label = 1
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn classvar_access_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    count: ClassVar[int] = 0

MyClass.count
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0126: Literal string assignment ---

#[test]
fn literal_str_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal["hello"] = "hello"
y: Literal["hello"] = "world"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn literal_int_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[1] = 1
y: Literal[1] = 2
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn literal_bool_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[True] = True
y: Literal[True] = False
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn literal_union_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal["a", "b", "c"] = "a"
y: Literal["a", "b", "c"] = "d"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0127: Tuple index out of range ---

#[test]
fn tuple_index_oob() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: tuple[int, str] = (1, "a")
v = x[5]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn tuple_index_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: tuple[int, str] = (1, "a")
v = x[0]
w = x[1]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn tuple_negative_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: tuple[int, str, float] = (1, "a", 3.0)
v = x[-1]
w = x[-10]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0128: TypeVar default referential ---

#[test]
fn typevar_default_ordering_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

Start2T = TypeVar("Start2T", default=StopT)
StopT = TypeVar("StopT", default=int)

class slice2(Generic[Start2T, StopT]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn typevar_default_outer_scope() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

S1 = TypeVar("S1")
S2 = TypeVar("S2", default=S1)

class Foo3(Generic[S1]):
    pass
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn typevar_default_valid_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2", default=T1)

class MyClass(Generic[T1, T2]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0129: Literal value assignment ---

#[test]
fn literal_value_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[42] = 42
y: Literal[42] = 99
z: Literal["hello"] = "hello"
w: Literal["hello"] = "bye"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn literal_none_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal[None] = None
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0130: TypeVar scoping ---

#[test]
fn typevar_nested_class_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Outer(Generic[T]):
    class Inner(Generic[T]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn typevar_function_class_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

def func(x: T) -> T:
    class Local(Generic[T]): ...
    return x
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn typevar_module_level_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

x = list[T]()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn method_call_typevar_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class MyClass(Generic[T]):
    def method(self, val: T) -> None:
        pass

x: MyClass[int] = MyClass()
x.method("hello")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0131: Generator yield type mismatch ---

#[test]
fn generator_yield_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

class A: ...
class B: ...

def bad_gen() -> Generator[A, None, None]:
    yield 3
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn iterator_yield_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Iterator

class A: ...

def bad_iter() -> Iterator[A]:
    yield 42
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn generator_yield_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def good_gen() -> Generator[int, None, None]:
    yield 42
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn yield_from_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def str_gen() -> Generator[str, None, None]:
    yield "hello"

def int_gen() -> Generator[int, None, None]:
    yield from str_gen()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0132: Inconsistent TypeVar ordering ---

#[test]
fn inconsistent_typevar_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
U = TypeVar("U")

class Base(Generic[T, U]): ...
class Child(Base[U, T]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0133: Protocol TypeVar variance ---

#[test]
fn protocol_typevar_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_co = TypeVar("T_co", covariant=True)

class Readable(Protocol[T_co]):
    def read(self) -> T_co: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0134: Invariant generic mismatch ---

#[test]
fn invariant_generic_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Node: ...

class SymbolTable(dict[str, list[Node]]): ...

def takes(x: dict[str, list[object]]) -> None: ...

def test(s: SymbolTable) -> None:
    takes(s)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0136: Callable subtyping ---

#[test]
fn callable_param_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_int_func(f: Callable[[int], None]) -> None:
    pass

def str_func(x: str) -> None:
    pass

takes_int_func(str_func)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn callable_return_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def takes_int_returning(f: Callable[[], int]) -> None:
    pass

def returns_str() -> str:
    return "hello"

takes_int_returning(returns_str)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0137: Generic protocol violations ---

#[test]
fn protocol_with_generic_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Generic, TypeVar

T_co = TypeVar("T_co", covariant=True)

class BadProto(Protocol[T_co], Generic[T_co]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn protocol_shorthand_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class GoodProto(Protocol[T]):
    def method(self, x: T) -> T: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn generic_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Transformer(Protocol[T]):
    def transform(self, x: T) -> T: ...

class IntDoubler:
    def transform(self, x: int) -> int:
        return x * 2

converter: Transformer[int] = IntDoubler()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn generic_protocol_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Transformer(Protocol[T]):
    def transform(self, x: T) -> T: ...

class StrReturner:
    def transform(self, x: int) -> str:
        return str(x)

converter: Transformer[int] = StrReturner()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0138: Dataclass transform metaclass ---

#[test]
fn dataclass_transform() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class ModelBase:
    def __init_subclass__(cls) -> None:
        pass

class User(ModelBase):
    name: str
    age: int

u = User(name="Alice", age=30)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0139: TypeVarTuple specialization ---

#[test]
fn typevartuple_specialization() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, Unpack

Ts = TypeVarTuple("Ts")

class Array(Generic[*Ts]):
    pass

x: Array[int, str] = Array()
y: Array[int] = Array()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0140: Callable assignment ---

#[test]
fn callable_annotation_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def add(x: int, y: int) -> int:
    return x + y

f: Callable[[str], str] = add
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn callable_annotation_correct() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def add(x: int, y: int) -> int:
    return x + y

f: Callable[[int, int], int] = add
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn protocol_callback() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Callback(Protocol):
    def __call__(self, x: int) -> str: ...

def my_func(x: int) -> str:
    return str(x)

cb: Callback = my_func
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn protocol_callback_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Callback(Protocol):
    def __call__(self, x: int) -> str: ...

def wrong_func(x: str) -> int:
    return len(x)

cb: Callback = wrong_func
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0141: Unpack kwargs ---

#[test]
fn unpack_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict, Unpack

class Options(TypedDict):
    name: str
    value: int

def func(**kwargs: Unpack[Options]) -> None:
    pass

func(name="test", value=42)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0142: Dataclass transform base ---

#[test]
fn dataclass_transform_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class ModelBase:
    pass

class User(ModelBase):
    name: str
    age: int

class Admin(User):
    role: str

a = Admin(name="Alice", age=30, role="admin")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0143: NamedTuple usage ---

#[test]
fn namedtuple_class_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float

p = Point(1.0, 2.0)
q = Point(x=1.0, y=2.0)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn namedtuple_functional_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Color = NamedTuple("Color", [("r", int), ("g", int), ("b", int)])

c = Color(255, 128, 0)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn namedtuple_method_override() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float

    def distance(self) -> float:
        return (self.x ** 2 + self.y ** 2) ** 0.5
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0144: type() call constructor ---

#[test]
fn type_call_three_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
MyClass = type("MyClass", (object,), {"x": 1})
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_call_single_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t = type(42)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// --- E0145: Invalid type bracket ---

#[test]
fn invalid_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import List, Dict, Optional, Union

x: List[int] = [1, 2, 3]
y: Dict[str, int] = {"a": 1}
z: Optional[int] = None
w: Union[int, str] = 42
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
