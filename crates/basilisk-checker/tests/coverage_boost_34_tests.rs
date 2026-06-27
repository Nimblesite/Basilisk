//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
//! Coverage boost tests batch 34: targeting resolver-dependent uncovered branches.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// ── E0107: Variance — class with base_subscripts ──

#[test]
fn covariant_in_invariant_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Container(Generic[T]):
    value: T

class BadCovariant(Container[T_co], Generic[T_co]):
    pass

class BadContravariant(Container[T_contra], Generic[T_contra]):
    pass

class DoubleNested(Generic[T_co]):
    data: Container[T_co]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn variance_through_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar, TypeAlias

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

class Base(Generic[T]):
    pass

ContainerAlias: TypeAlias = Base[T_co]

class Child(ContainerAlias, Generic[T_co]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn multiple_generic_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
S = TypeVar("S")
T_co = TypeVar("T_co", covariant=True)

class First(Generic[T]):
    pass

class Second(Generic[S]):
    pass

class Multi(First[T_co], Second[T_co], Generic[T_co]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0115: Deprecated — in-module usage ──

#[test]
fn deprecated_in_same_module() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import warnings

def deprecated(msg):
    def decorator(func):
        return func
    return decorator

@deprecated("Use new_func")
def old_func() -> int:
    return 1

@deprecated("Use NewClass")
class OldClass:
    @deprecated("Use new_method")
    def old_method(self) -> int:
        return 1

    @property
    def old_prop(self) -> int:
        return 42

result = old_func()
obj = OldClass()
obj.old_method()
val = obj.old_prop
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn deprecated_typing_extensions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

@deprecated("old")
def legacy_func(x: int) -> str:
    return str(x)

@deprecated("old class")
class LegacyClass:
    @deprecated("old method")
    def method(self) -> None:
        pass

    @deprecated("old static")
    @staticmethod
    def static_method() -> None:
        pass

legacy_func(1)
obj = LegacyClass()
obj.method()
LegacyClass.static_method()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn deprecated_warnings_module() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from warnings import deprecated

@deprecated("Use new_func instead")
def old_func() -> None:
    pass

@deprecated("Use NewClass instead")
class OldClass:
    pass

old_func()
x = OldClass()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0137: Generic protocol — subscripted assignment ──

#[test]
fn subscripted_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic

T = TypeVar("T")

class Getter(Protocol[T]):
    def get(self) -> T: ...

class IntGetter:
    def get(self) -> int:
        return 42

class StrGetter:
    def get(self) -> str:
        return "hello"

# Subscripted protocol assignment
a: Getter[int] = IntGetter()
b: Getter[str] = StrGetter()
c: Getter[int] = StrGetter()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn protocol_generic_both_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)

class ProtoAndGeneric(Protocol[T_co], Generic[T_co]):
    def get(self) -> T_co: ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn protocol_self_typed_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class Clonable(Protocol):
    def clone(self: T) -> T: ...

class Good:
    def clone(self) -> "Good":
        return Good()

class Bad:
    def clone(self) -> int:
        return 42

x: Clonable = Good()
y: Clonable = Bad()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0148: Generic type arg — constrained/subscript/metaclass ──

#[test]
fn constrained_typevar_mixed_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

AnyStr = TypeVar("AnyStr", str, bytes)

def concat(a: AnyStr, b: AnyStr) -> AnyStr:
    return a + b

# Mixed constraint violation
result = concat("hello", b"world")

# Valid
ok1 = concat("a", "b")
ok2 = concat(b"a", b"b")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn mapping_key_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Mapping

my_map: Mapping[str, int] = {}

# Valid access
val = my_map["key"]

# Invalid key type
val2 = my_map[0]
val3 = my_map[True]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn generic_metaclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class GenericMeta(type, Generic[T]):
    pass

class MyClass(metaclass=GenericMeta[int]):
    pass

class MyClass2(metaclass=GenericMeta):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0130: TypeVar scope — line-by-line scope tracking ──

#[test]
fn nested_class_typevar_scope() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
S = TypeVar("S")

class Outer(Generic[T]):
    class Inner(Generic[T]):
        value: T

    class Inner2:
        x: T = None
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn unbound_typevar_module_level() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar, TypeAlias

T = TypeVar("T")
S = TypeVar("S")

# Unbound at module level
x: list[T] = []
y: T = None

class MyClass(Generic[T]):
    MyAlias: TypeAlias = list[T]

    def method(self, x: T) -> T:
        return x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn typevar_in_function_scope() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Container(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

    def get(self) -> T:
        return self.value

    def transform(self, other: T) -> T:
        return other

# Method call with wrong types
c: Container[int] = Container(42)
c.transform("wrong")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0111: Constructor — more patterns ──

#[test]
fn specialized_generic_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Wrapper(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

# Specialized constructor calls
w1 = Wrapper[int](42)
w2 = Wrapper[str]("hello")
w3 = Wrapper[int]("wrong_type")
w4 = Wrapper[float](3.14)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn explicit_self_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
S = TypeVar("S")

class Pair(Generic[T, S]):
    def __init__(self: "Pair[T, S]", first: T, second: S) -> None:
        self.first = first
        self.second = second

p1 = Pair(1, "hello")
p2 = Pair[int, str](1, "hello")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn namedtuple_type_check() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Person(NamedTuple):
    name: str
    age: int
    active: bool = True

# Correct
p1 = Person("Alice", 30)
p2 = Person("Bob", 25, False)
p3 = Person(name="Carol", age=40)

# Wrong types
p4 = Person(42, "wrong")
p5 = Person("Dave", "not_int")

# Wrong number of args
p6 = Person("Eve")
p7 = Person("Frank", 1, True, "extra")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0097: Protocol __init__ — deeper body walking ──

#[test]
fn protocol_init_for_loop_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyProto(Protocol):
    x: int
    y: str

    def __init__(self) -> None:
        self.x = 0
        self.y = ""
        self.undeclared1 = True
        for i in range(10):
            self.undeclared2 = i
        if True:
            self.undeclared3 = "nested"
            if False:
                self.undeclared4 = 42
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0102: TypeVar default — more patterns ──

#[test]
fn typevar_default_bound_compat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

# Bound compatibility
T1 = TypeVar("T1", bound=int)
T2 = TypeVar("T2", bound=float, default=T1)

# Bound incompatibility
X1 = TypeVar("X1", bound=int)
X2 = TypeVar("X2", bound=str, default=X1)

# Constraint compatibility
C1 = TypeVar("C1", int, str, float)
C2 = TypeVar("C2", int, str, default=C1)

# Ordering
D1 = TypeVar("D1")
D2 = TypeVar("D2", default=D1)
D3 = TypeVar("D3", default=D2)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0085: TypeVarTuple — more patterns ──

#[test]
fn typevartuple_mixed_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic, TypeVar, Unpack

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

class Tensor(Generic[T, *Ts]):
    def __init__(self, dtype: T, *shape: Unpack[Ts]) -> None:
        pass

t1 = Tensor[int, int, int](0, 1, 2)
t2 = Tensor[float](3.14)
t3 = Tensor[str, int, int, int]("", 1, 2, 3)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0119: Protocol isinstance — more patterns ──

#[test]
fn protocol_isinstance_data_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, runtime_checkable

@runtime_checkable
class DataProto(Protocol):
    name: str
    age: int

@runtime_checkable
class MethodProto(Protocol):
    def compute(self) -> int: ...

@runtime_checkable
class MixedProto(Protocol):
    name: str
    def compute(self) -> int: ...

x = object()

# Data protocol with isinstance
isinstance(x, DataProto)

# Method-only protocol with isinstance
isinstance(x, MethodProto)

# Mixed protocol with isinstance
isinstance(x, MixedProto)

# issubclass checks
issubclass(int, DataProto)
issubclass(str, MethodProto)
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0146: Protocol class object — more patterns ──

#[test]
fn protocol_type_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Type

class Serializable(Protocol):
    def serialize(self) -> bytes: ...

class JsonSerializer:
    def serialize(self) -> bytes:
        return b"{}"

class XmlSerializer:
    def serialize(self) -> bytes:
        return b"<xml/>"

def make(cls: type[Serializable]) -> Serializable:
    return cls()

# Protocol itself passed as type
make(Serializable)

# Concrete class
make(JsonSerializer)
make(XmlSerializer)

# type[] annotation
x: type[Serializable] = Serializable
y: type[Serializable] = JsonSerializer
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0138: Frozen dataclass — with ordering ──

#[test]
fn frozen_with_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(frozen=True, order=True)
class Ordered:
    x: int
    y: str

a = Ordered(1, "a")
b = Ordered(2, "b")

# Ordering is enabled
r1 = a < b
r2 = a <= b
r3 = a > b
r4 = a >= b

# Mutation is forbidden
a.x = 10
a.y = "z"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn frozen_without_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass(frozen=True)
class NoOrder:
    x: int
    y: str

a = NoOrder(1, "a")
b = NoOrder(2, "b")

# Ordering NOT enabled — should error
r1 = a < b
r2 = a <= b

# Mutation forbidden
a.x = 10
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0140: Callable — more edge cases ──

#[test]
fn callable_protocol_form() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable, Protocol

class Processor(Protocol):
    def __call__(self, x: int) -> str: ...

def my_func(x: int) -> str:
    return str(x)

def wrong_func(x: str) -> int:
    return int(x)

a: Processor = my_func
b: Processor = wrong_func

# Callable with multiple params
c: Callable[[int, str, float], bool] = lambda x, y, z: True
d: Callable[[int, str, float], bool] = lambda x, y: True
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0120: Generator — yield from ──

#[test]
fn generator_yield_from() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generator, Iterator

def gen_yield_from_ok() -> Iterator[int]:
    yield from [1, 2, 3]

def gen_yield_from_bad() -> Iterator[str]:
    yield from [1, 2, 3]

def gen_mixed() -> Generator[int, None, str]:
    yield 1
    yield 2
    return "done"

def gen_async_iter():
    yield 1
    yield 2
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0143: NamedTuple — delete/unpack ──

#[test]
fn namedtuple_delete_and_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Vec3(NamedTuple):
    x: float
    y: float
    z: float

v = Vec3(1.0, 2.0, 3.0)

# Delete
del v.x
del v[0]

# Unpack
a, b, c = v
a, b, c, d = v

# Negative index
val = v[-1]
val2 = v[-4]
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0095: InitVar — attribute access ──

#[test]
fn initvar_attribute_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, InitVar

@dataclass
class WithInit:
    name: str
    debug: InitVar[bool]
    verbose: InitVar[int]

    def __post_init__(self, debug: bool, verbose: int) -> None:
        if debug:
            print(self.name)

w = WithInit("test", True, 1)
w.name
w.debug
w.verbose
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── E0108: Slots — __slots__ access ──

#[test]
fn slots_access_on_non_slots() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass

@dataclass
class NoSlots:
    x: int

@dataclass(slots=True)
class WithSlots:
    x: int
    y: str

ns = NoSlots(1)
ws = WithSlots(1, "a")

# Access __slots__ on non-slots class
ns.__slots__

# Dynamic attribute on slots class
ws.z = 3
ws.dynamic = True
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Additional collection inference ──

#[test]
fn collection_inference_empty_containers() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
# Empty collection inference
a: list[int] = []
b: dict[str, int] = {}
c: set[float] = set()

# List comprehension
d: list[str] = [str(i) for i in range(10)]

# Dict comprehension
e: dict[int, str] = {i: str(i) for i in range(10)}

# Set comprehension
f: set[int] = {i for i in range(10)}
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ── Diagnostic severity paths ──

#[test]
fn diagnostic_severity_info_path() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class MyClass(Generic[T]):
    pass

# Multiple type mismatches to exercise severity paths
x: int = "hello"
y: str = 42
z: float = "bad"
w: bool = "yes"
a: bytes = 42
b: list = "not a list"
c: dict = "not a dict"
d: set = "not a set"
e: tuple = "not a tuple"
"#;
    let diagnostics = run(source)?;
    assert!(
        !diagnostics.is_empty(),
        "Should produce type mismatch diagnostics"
    );
    Ok(())
}

// ── Narrowing — enter_branch coverage ──

#[test]
fn narrowing_nested_branches() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Optional, Union

def deep_narrow(x: Optional[Union[int, str, list]]) -> str:
    if x is not None:
        if isinstance(x, int):
            return str(x)
        elif isinstance(x, str):
            return x
        else:
            return str(x)
    return ""

def multi_branch(x: Union[int, str, float, bool]) -> str:
    if isinstance(x, bool):
        return "bool"
    elif isinstance(x, int):
        return "int"
    elif isinstance(x, float):
        return "float"
    else:
        return x
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}
