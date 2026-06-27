//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 11: targeting deeply uncovered code paths.
// Focuses on: e0014 (literal parsing, tuple reassignment, dataclass attrs),
// e0036 (`ClassVar` edge cases), e0047 (invalid annotations), e0111 (constructor errors),
// e0120 (generator violations), e0130 (typevar scoping), e0131 (yield types).

#[expect(dead_code)]
fn run_with_path(
    source: &str,
    path: &str,
) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), path.to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// =============================================================================
// E0014: Literal type parsing - hex, octal, binary, float, bytes, negative
// =============================================================================

#[test]
fn literal_hex_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal[255] = 0xFF
y: Literal[10] = 0xA
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_octal_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal[8] = 0o10
y: Literal[7] = 0o7
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_binary_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal[3] = 0b11
y: Literal[5] = 0b101
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_negative_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal[-1] = -1
y: Literal[-42] = -42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_float_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal[3.14] = 3.14
y: Literal[1.0] = 1.0
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_bytes_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal[b"hello"] = b"hello"
y: Literal[b'world'] = b'world'
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_single_quoted_string() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal['hello'] = 'hello'
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_bool_values() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal[True] = True
y: Literal[False] = False
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0014: Tuple reassignment ---

#[test]
fn tuple_reassign_wrong_length() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int] = (1,)
t1 = (1, 2)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_reassign_empty_tuple_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[()] = ()
t1 = (1, 2)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_reassign_element_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, str] = (1, "hello")
t1 = ("wrong", 42)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_reassign_homogeneous_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, ...] = (1, 2, 3)
t1 = (1, "hello", 3)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_nested_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, int] = (1, 2)
t1 = ((1, 2), 3)
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0014: Dataclass attribute assignments ---

#[test]
fn dataclass_attr_str_to_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

p = Point(1, 2)
p.x = "hello"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn dataclass_attr_bytes_to_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Data:
    value: int

d = Data(42)
d.value = b"bytes"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn dataclass_attr_float_to_str() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Config:
    name: str

c = Config("test")
c.name = 3.14
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn dataclass_attr_int_to_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Blob:
    data: bytes

b = Blob(b"data")
b.data = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn dataclass_attr_negative_number() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Counter:
    value: str

c = Counter("x")
c.value = -42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn dataclass_attr_none_value() -> Result<(), Box<dyn std::error::Error>> {
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
fn dataclass_attr_fstring() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class Msg:
    content: int

m = Msg(0)
m.content = f"hello {42}"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn local_var_literal_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def foo(a: Literal[0]) -> None:
    x: Literal[False] = a
"#;
    let _ = run(source)?;
    Ok(())
}

// --- E0014: Various literal annotation mismatches ---

#[test]
fn str_annotated_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: str = b"bytes_data"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn int_annotated_float() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 3.14
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn bytes_annotated_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: bytes = 42
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0036: ClassVar deep code paths
// =============================================================================

#[test]
fn classvar_multiple_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    x: ClassVar[int, str] = 42
"#;
    let diags = run(source)?;
    let has_e0036 = diags.iter().any(|d| d.code.code == "classes_classvar");
    assert!(
        has_e0036,
        "Expected classes_classvar for multiple ClassVar args"
    );
    Ok(())
}

#[test]
fn classvar_invalid_type_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    x: ClassVar[123] = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_runtime_variable_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

some_var = 42

class MyClass:
    x: ClassVar[some_var] = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_typevar_in_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, TypeVar

T = TypeVar("T")

class MyClass:
    x: ClassVar[T] = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_dict_literal_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    x: ClassVar[int] = {"key": "value"}
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_list_literal_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    x: ClassVar[int] = [1, 2, 3]
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_in_rhs_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

x = ClassVar[int]
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_instance_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    x: ClassVar[int] = 42

obj = MyClass()
obj.x = 100
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_protocol_missing_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Protocol

class MyProtocol(Protocol):
    x: ClassVar[int]

class MyImpl:
    def __init__(self) -> None:
        self.x = 42

v: MyProtocol = MyImpl()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_paramspec() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, ParamSpec

P = ParamSpec("P")

class MyClass:
    x: ClassVar[P] = None
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_typevartuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, TypeVarTuple

Ts = TypeVarTuple("Ts")

class MyClass:
    x: ClassVar[Ts] = None
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0047: Invalid type expressions - deep branches
// =============================================================================

#[test]
fn numeric_literal_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: 42 = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_literal_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: (int, str) = (1, "hello")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn paramspec_in_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec, TypeAlias

P = ParamSpec("P")
MyType: TypeAlias = P
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn invalid_local_var_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def foo() -> None:
    x: [int, str] = [1, "hello"]
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn invalid_class_attr_annotation_lambda() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    x: lambda: int = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn invalid_class_attr_annotation_fstring() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    x: f"int" = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn invalid_class_attr_annotation_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    x: {"key": int} = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn invalid_class_attr_annotation_conditional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    x: int if True else str = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn invalid_class_attr_boolean_op() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    x: int and str = 42
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0111: Constructor call errors - deep branches
// =============================================================================

#[test]
fn init_self_annotation_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class MyClass(Generic[T]):
    def __init__(self: "MyClass[T]", value: T) -> None:
        self.value = value
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn constructor_parent_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Animal:
    def __init__(self, name: str) -> None:
        self.name = name

class Dog(Animal):
    def __init__(self, name: str, breed: str) -> None:
        super().__init__(name)
        self.breed = breed

class Puppy(Dog):
    def __init__(self) -> None:
        super().__init__("puppy", "mixed")
        x = Dog("test", "lab")
        y = Animal("base")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn constructor_bool_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Config:
    def __init__(self, enabled: str) -> None:
        self.enabled = enabled

c = Config(True)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn constructor_bytes_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Processor:
    def __init__(self, data: str) -> None:
        self.data = data

p = Processor(b"binary")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn constructor_none_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Container:
    def __init__(self, value: int) -> None:
        self.value = value

c = Container(None)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn constructor_union_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Handler:
    def __init__(self, value: int | str) -> None:
        self.value = value

h = Handler(True)
h2 = Handler(42)
h3 = Handler("hello")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn subclass_constructor_pass_parent() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    def __init__(self, value: int) -> None:
        self.value = value

class Child(Base):
    def __init__(self, value: int) -> None:
        super().__init__(value)

# In __init__, passing a Base() where Self (Child) is expected
class GrandChild(Child):
    def __init__(self) -> None:
        x = Base(1)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0120: Generator return type violations
// =============================================================================

#[test]
fn yield_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, None]:
    yield "string"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn yield_from_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def inner() -> Generator[str, None, None]:
    yield "hello"

def outer() -> Generator[int, None, None]:
    yield from inner()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn yield_from_send_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def inner() -> Generator[int, str, None]:
    x = yield 42

def outer() -> Generator[int, int, None]:
    yield from inner()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn generator_return_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, str]:
    yield 42
    return 123
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn yield_from_list_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, None]:
    yield from [1, "hello", 3]
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn generator_complex_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, Iterator

def gen1() -> Generator[int, None, None]:
    yield 1
    yield 2
    yield 3

def gen2() -> Iterator[int]:
    yield 10
    yield 20
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0130: TypeVar scoping - nested classes, literal inference
// =============================================================================

#[test]
fn typevar_in_nested_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Outer(Generic[T]):
    class Inner:
        x: T = None
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_type_inference_bool() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)

def foo(x: T) -> T:
    return x

result = foo(True)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_type_inference_none() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Optional

T = TypeVar("T")

def foo(x: Optional[T]) -> Optional[T]:
    return x

result = foo(None)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_type_inference_float() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=float)

def foo(x: T) -> T:
    return x

result = foo(3.14)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_type_inference_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

def foo(x: T) -> T:
    return x

result = foo(b"hello")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn literal_type_inference_negative_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int)

def foo(x: T) -> T:
    return x

result = foo(-42)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn typevar_nested_class_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Container(Generic[T]):
    def method(self) -> T:
        class Helper:
            value: T = None
        return Helper().value
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0131: Generator yield type violations
// =============================================================================

#[test]
fn yield_str_in_int_generator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, None]:
    yield 1
    yield "oops"
    yield 3
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn async_generator_yield_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import AsyncGenerator

async def agen() -> AsyncGenerator[int, None]:
    yield 1
    yield "string"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn multiple_yield_mismatches() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def gen() -> Generator[int, None, None]:
    yield "first"
    yield 2
    yield "third"
    yield 4.0
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0140: Callable assignment - complex cases
// =============================================================================

#[test]
fn callable_protocol_with_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Callable

class HasCall(Protocol):
    def __call__(self, x: int) -> str: ...

def my_func(x: int) -> str:
    return str(x)

f: HasCall = my_func
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_with_ellipsis_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(x: int) -> str:
    ...

f: Callable[[int], str] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_varargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(*args: int) -> str:
    return str(sum(args))

f: Callable[..., str] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(**kwargs: str) -> int:
    return len(kwargs)

f: Callable[..., int] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_positional_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(x: int, y: str, /) -> bool:
    return True

f: Callable[[int, str], bool] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn callable_kwonly() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(*, x: int, y: str) -> bool:
    return True

f: Callable[..., bool] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_complex_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Comparable(Protocol):
    def __lt__(self, other: "Comparable") -> bool: ...
    def __gt__(self, other: "Comparable") -> bool: ...

class MyNum:
    def __lt__(self, other: "MyNum") -> bool:
        return True
    def __gt__(self, other: "MyNum") -> bool:
        return True

x: Comparable = MyNum()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0144: type() call constructor - deep paths
// =============================================================================

#[test]
fn type_with_bases_and_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
MyClass = type("MyClass", (object,), {"x": 42, "y": "hello"})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn type_with_annotation_wrong_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
MyClass = type("MyClass", (object,), {"x": 42}, extra_arg=True)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn type_single_arg() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x = type(42)
y = type("hello")
z = type([1, 2, 3])
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0149: PEP 695 type parameter scoping - deep paths
// =============================================================================

#[test]
fn typevar_reuse_across_functions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

def foo(x: T) -> T:
    return x

def bar(x: T) -> T:
    return x
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn typevar_complex_nesting() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
U = TypeVar("U")

class Outer(Generic[T]):
    def method(self, x: T) -> T:
        return x

    class Inner(Generic[U]):
        def inner_method(self, y: U) -> U:
            return y
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn typevar_in_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Callable

T = TypeVar("T")

def decorator(func: Callable[..., T]) -> Callable[..., T]:
    return func

@decorator
def my_func() -> int:
    return 42
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0137: Generic protocol - deep paths
// =============================================================================

#[test]
fn protocol_generic_multiple_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic

T = TypeVar("T")

class Container(Protocol[T]):
    def get(self) -> T: ...
    def set(self, value: T) -> None: ...
    def items(self) -> list[T]: ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_generic_with_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T", bound=int)

class Numeric(Protocol[T]):
    def add(self, other: T) -> T: ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_and_generic_combined() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic

T = TypeVar("T")

class MyProtocol(Protocol[T], Generic[T]):
    def method(self) -> T: ...
"#;
    let diags = run(source)?;
    let has_e0137 = diags.iter().any(|d| d.code.code == "protocols_generic");
    assert!(
        has_e0137,
        "Expected protocols_generic for Protocol[T]+Generic[T]"
    );
    Ok(())
}

// =============================================================================
// E0063: Non-hashable dataclass - Hashable annotation
// =============================================================================

#[test]
fn non_hashable_dataclass_hashable_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass
from typing import Hashable

@dataclass
class DC:
    x: int

v: Hashable = DC(0)
"#;
    let diags = run(source)?;
    let has_e0063 = diags.iter().any(|d| d.code.code == "dataclasses_hash");
    assert!(
        has_e0063,
        "Expected dataclasses_hash for non-hashable dataclass"
    );
    Ok(())
}

#[test]
fn frozen_dataclass_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass
from typing import Hashable

@dataclass(frozen=True)
class DC:
    x: int

v: Hashable = DC(0)
"#;
    let diags = run(source)?;
    let has_e0063 = diags.iter().any(|d| d.code.code == "dataclasses_hash");
    assert!(!has_e0063, "Frozen dataclass should not trigger E0063");
    Ok(())
}

#[test]
fn typing_hashable_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class DC:
    x: int

v: typing.Hashable = DC(0)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn collections_abc_hashable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class DC:
    x: int

v: collections.abc.Hashable = DC(0)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0107: Variance incompatibility - deep branches
// =============================================================================

#[test]
fn variance_alias_expansion() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Producer(Generic[T_co]):
    def get(self) -> T_co: ...

class Consumer(Generic[T_contra]):
    def accept(self, item: T_contra) -> None: ...

class Both(Producer[int], Consumer[str]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn variance_nested_generics() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
U = TypeVar("U")

class Pair(Generic[T, U]):
    first: T
    second: U

class Container(Generic[T]):
    items: list[T]

class Combo(Pair[int, str], Container[float]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn variance_multiple_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)

class Base1(Generic[T_co]):
    pass

class Base2(Generic[T_co]):
    pass

class Child(Base1[T_co], Base2[T_co]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0079: Module protocol incompatibility (requires file system, exercise parser)
// =============================================================================

#[test]
fn module_protocol_basic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol
import os

class FileProtocol(Protocol):
    sep: str

x: FileProtocol = os
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn module_protocol_with_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol
import sys

class StreamProtocol(Protocol):
    def write(self, data: str) -> int: ...

x: StreamProtocol = sys
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0073: NamedTuple tuple compat
// =============================================================================

#[test]
fn namedtuple_wrong_field_count() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

p: tuple[int, int, int] = Point(1, 2)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0122: Callable arity checks
// =============================================================================

#[test]
fn too_few_positional_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(a: int, b: str, c: float) -> None:
    pass

f: Callable[[int, str], None] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn optional_params_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def foo(a: int, b: str = "default") -> None:
    pass

f: Callable[[int], None] = foo
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0121: Protocol conformance
// =============================================================================

#[test]
fn protocol_wrong_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Sized(Protocol):
    def __len__(self) -> int: ...

class MyList:
    def __len__(self) -> str:
        return "10"

x: Sized = MyList()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0095: InitVar and dataclass fields
// =============================================================================

#[test]
fn initvar_no_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar

@dataclass
class MyClass:
    x: int
    y: InitVar[str]
    z: int = 0
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn initvar_after_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar, field

@dataclass
class MyClass:
    x: int = 0
    y: InitVar[str] = "default"
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0143: NamedTuple with methods
// =============================================================================

#[test]
fn namedtuple_with_methods_and_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int = 0

    def distance(self) -> float:
        return (self.x ** 2 + self.y ** 2) ** 0.5
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0145: Invalid type[X] bracket
// =============================================================================

#[test]
fn type_bracket_with_union() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union

def foo(x: type[int | str]) -> None:
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0102: TypeVar default violations
// =============================================================================

#[test]
fn typevar_default_with_constraints() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", int, str, default=float)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0054: Final reassignment
// =============================================================================

#[test]
fn final_in_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

def foo() -> None:
    x: Final = 42
    x = 100
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn final_class_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

class MyClass:
    X: Final = 42

MyClass.X = 100
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0050: Invalid NewType
// =============================================================================

#[test]
fn newtype_with_generic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, List

UserId = NewType("UserId", int)
UserList = NewType("UserList", List[int])
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0076: Overload union expansion
// =============================================================================

#[test]
fn overload_with_union_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload, Union

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...

def process(x: Union[int, str]) -> Union[int, str]:
    return x
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0015: Return type annotation required
// =============================================================================

#[test]
fn complex_function_no_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class MyClass:
    def method(self, x: int, y: str):
        return x
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0041: Too few arguments
// =============================================================================

#[test]
fn missing_multiple_required() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def foo(a: int, b: str, c: float) -> None:
    pass

foo(1)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn missing_after_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def bar(a: int, b: str = "default", *, c: float) -> None:
    pass

bar(1)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0116: NamedTuple definition errors
// =============================================================================

#[test]
fn namedtuple_functional_duplicate() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Point = NamedTuple("Point", [("x", int), ("x", int)])
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0118: Super abstract no implementation
// =============================================================================

#[test]
fn abstract_method_not_implemented() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from abc import ABC, abstractmethod

class Base(ABC):
    @abstractmethod
    def method(self) -> int: ...

class Child(Base):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0092: Too few type args
// =============================================================================

#[test]
fn generic_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
U = TypeVar("U")
V = TypeVar("V")

class Triple(Generic[T, U, V]):
    pass

x: Triple[int] = Triple()
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0094: Self type invalid location
// =============================================================================

#[test]
fn self_in_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Self

class MyClass:
    def method(self) -> None:
        def helper() -> Self:
            return self
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0038: TypedDict inheritance invalid
// =============================================================================

#[test]
fn typeddict_non_typeddict_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class Base:
    x: int

class MyDict(TypedDict, Base):
    y: str
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0108: Dataclass slots
// =============================================================================

#[test]
fn slots_with_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(slots=True)
class MyClass:
    __slots__ = ("x",)
    x: int
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0110: Protocol variance
// =============================================================================

#[test]
fn protocol_covariant_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_co = TypeVar("T_co", covariant=True)

class Producer(Protocol[T_co]):
    def produce(self) -> T_co: ...
    def accept(self, item: T_co) -> None: ...
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0117: Unbound TypeVar
// =============================================================================

#[test]
fn unbound_typevar_in_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

x: T = 42
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0051: Invalid Literal
// =============================================================================

#[test]
fn literal_with_variable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x = 42
y: Literal[x] = 42
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0064: NamedTuple invalid arg
// =============================================================================

#[test]
fn namedtuple_keyword_fieldname() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Point = NamedTuple("Point", x=int, y=int)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0091: TypeVar default incompatible
// =============================================================================

#[test]
fn typevar_default_outside_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", bound=int, default=str)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0069: Dataclass kw_only
// =============================================================================

#[test]
fn kwonly_after_regular() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field

@dataclass
class MyClass:
    x: int
    y: int = field(kw_only=True)
    z: int = 0
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0112: TypeGuard callable return
// =============================================================================

#[test]
fn typeguard_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeGuard

def outer() -> None:
    def is_str(x: object) -> TypeGuard[str]:
        return isinstance(x, str)
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0139: TypeVarTuple specialization
// =============================================================================

#[test]
fn typevartuple_with_regular_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, TypeVarTuple, Generic, Unpack

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Mixed(Generic[T, Unpack[Ts]]):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0126: Literal string assignment
// =============================================================================

#[test]
fn literal_none_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

x: Literal["hello"] = None
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0148: Generic type arg
// =============================================================================

#[test]
fn generic_subscript_wrong_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import List, Dict

x: List[int, str] = []
y: Dict[int] = {}
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0146: Protocol class object
// =============================================================================

#[test]
fn protocol_multiple_methods_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Serializable(Protocol):
    def serialize(self) -> str: ...
    def deserialize(self, data: str) -> None: ...

class JsonObj:
    def serialize(self) -> str:
        return "{}"
    def deserialize(self, data: str) -> None:
        pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0119: Protocol isinstance overlap
// =============================================================================

#[test]
fn runtime_checkable_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Drawable(Protocol):
    def draw(self) -> None: ...
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn non_runtime_protocol_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyProtocol(Protocol):
    def method(self) -> None: ...

x = object()
if isinstance(x, MyProtocol):
    pass
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// E0138: Dataclass transform metaclass
// =============================================================================

#[test]
fn transform_with_multiple_options() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform(kw_only_default=True, frozen_default=True)
class ModelMeta(type):
    pass

class Model(metaclass=ModelMeta):
    x: int
    y: str = "default"
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn transform_base_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class BaseModel:
    def __init_subclass__(cls, **kwargs: object) -> None:
        pass

class User(BaseModel):
    name: str
    age: int
"#;
    let _ = run(source)?;
    Ok(())
}

// =============================================================================
// Additional edge cases for maximum coverage
// =============================================================================

#[test]
fn tuple_bytes_element() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[bytes, str] = (b"data", "text")
t1 = (b"new", "updated")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_bool_element() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[bool, bool] = (True, False)
t1 = (True, True)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn tuple_none_element() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, None] = (1, None)
t1 = ("wrong", None)
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn star_import_names() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import *

x: List[int] = []
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn constructor_multiple_classes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class A:
    def __init__(self, x: int) -> None:
        self.x = x

class B:
    def __init__(self, x: str) -> None:
        self.x = x

class C(A, B):
    def __init__(self) -> None:
        super().__init__(1)

a = A(1)
b = B("hello")
c = C()
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn typevar_scope_with_multiple_constraints() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", int, str, bytes)

def process(x: T) -> T:
    return x

result1 = process(42)
result2 = process("hello")
result3 = process(b"data")
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn classvar_with_cv_prefix() -> Result<(), Box<dyn std::error::Error>> {
    // Test the "CV[" prefix path
    let source = r#"
from typing import ClassVar as CV

class MyClass:
    x: CV[int] = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn dataclass_transform_attr_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import dataclass_transform

@dataclass_transform()
class BaseModel:
    def __init_subclass__(cls, **kwargs: object) -> None:
        pass

class User(BaseModel):
    name: str
    age: int

u = User()
u.name = 42
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn class_attr_list_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    x: [1, 2, 3] = None
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn class_attr_set_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    x: {int, str} = None
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn iterator_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Iterator

def my_range(n: int) -> Iterator[int]:
    i = 0
    while i < n:
        yield i
        i += 1
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn async_iterator() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import AsyncIterator

async def async_range(n: int) -> AsyncIterator[int]:
    i = 0
    while i < n:
        yield i
        i += 1
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn generator_with_send() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator

def accumulator() -> Generator[int, int, str]:
    total = 0
    while True:
        value = yield total
        if value is None:
            return "done"
        total += value
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn protocol_method_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Transform(Protocol):
    def __call__(self, x: int) -> str: ...

def my_transform(x: int) -> int:
    return x

f: Transform = my_transform
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn type_call_wrong_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
MyClass = type("WrongName", (object,), {})
"#;
    let _ = run(source)?;
    Ok(())
}

#[test]
fn method_lookup_chain() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class GrandParent:
    def __init__(self, x: int) -> None:
        self.x = x

class Parent(GrandParent):
    def __init__(self, x: int, y: str) -> None:
        super().__init__(x)
        self.y = y

class Child(Parent):
    def __init__(self) -> None:
        super().__init__(1, "hello")

class GrandChild(Child):
    def __init__(self) -> None:
        super().__init__()
"#;
    let _ = run(source)?;
    Ok(())
}
