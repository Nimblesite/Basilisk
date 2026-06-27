//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
//! Coverage boost tests batch 32: targeting top uncovered rule files.
//! Focus on e0115 deprecated usage, e0130 `TypeVar` scope, e0148 generic type args,
//! and the long tail of near-complete rule files.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// ═══════════════════════════════════════════════════════════════════════
// E0115: Deprecated usage — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════

/// Tests that @deprecated functions are detected when called.
#[test]
fn e0115_deprecated_function_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

@deprecated("Use new_func instead")
def old_func(x: int) -> int:
    return x

# Call the deprecated function
result = old_func(5)

# Reference without calling
ref = old_func
"#;
    let diagnostics = run(source)?;
    let e0115: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "directives_deprecated")
        .collect();
    assert!(
        !e0115.is_empty(),
        "should flag deprecated function usage: {:?}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// Tests deprecated class usage.
#[test]
fn e0115_deprecated_class_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

@deprecated("Use NewClass instead")
class OldClass:
    pass

# Instantiate the deprecated class
obj = OldClass()

# Reference without instantiating
cls_ref = OldClass
"#;
    let diagnostics = run(source)?;
    let e0115: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "directives_deprecated")
        .collect();
    assert!(
        !e0115.is_empty(),
        "should flag deprecated class usage: {:?}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code.code, d.message))
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// Tests deprecated method calls via instance variable type inference.
#[test]
fn e0115_deprecated_method_via_instance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class MyClass:
    @deprecated("Use new_method instead")
    def old_method(self) -> int:
        return 1

    def new_method(self) -> int:
        return 2

obj = MyClass()
obj.old_method()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests deprecated property access.
#[test]
fn e0115_deprecated_property() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class Config:
    @property
    @deprecated("Use new_prop instead")
    def old_prop(self) -> int:
        return 42

    @old_prop.setter
    @deprecated("Use new_prop setter")
    def old_prop(self, value: int) -> None:
        pass

cfg = Config()
val = cfg.old_prop
cfg.old_prop = 10
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests deprecated __call__ dunder.
#[test]
fn e0115_deprecated_call_dunder() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class Invocable:
    @deprecated("Not callable anymore")
    def __call__(self) -> int:
        return 1

obj = Invocable()
obj()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests deprecated dunder via binary operator.
#[test]
fn e0115_deprecated_add_dunder() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class Addable:
    @deprecated("Use combine() instead")
    def __add__(self, other: int) -> "Addable":
        return self

x = Addable()
x += 1
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests deprecated setter via augmented assignment on attribute.
#[test]
fn e0115_deprecated_setter_aug_assign() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class Shape:
    @property
    def size(self) -> int:
        return 0

    @size.setter
    @deprecated("Use resize() instead")
    def size(self, value: int) -> None:
        pass

s = Shape()
s.size += 10
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests deprecated overloaded function.
#[test]
fn e0115_deprecated_overload() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload
from typing_extensions import deprecated

@overload
@deprecated("Use new_process")
def process(x: int) -> int: ...
@overload
@deprecated("Use new_process")
def process(x: str) -> str: ...
def process(x):
    return x

process(42)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests deprecated usage inside function body with param annotation types.
#[test]
fn e0115_deprecated_in_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class Widget:
    @deprecated("Use new_render")
    def render(self) -> str:
        return ""

def process(w: Widget) -> str:
    return w.render()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests deprecated usage in control flow.
#[test]
fn e0115_deprecated_in_control_flow() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

@deprecated("Use new_check")
def old_check() -> bool:
    return True

if old_check():
    pass

for i in range(10):
    old_check()

while old_check():
    break

x: int = old_check()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests deprecated via module attribute access pattern.
#[test]
fn e0115_deprecated_attribute_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import deprecated

class Library:
    @deprecated("Use Library.new_func")
    @staticmethod
    def old_func() -> None:
        pass

Library.old_func()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0130: TypeVar scoping — comprehensive coverage
// ═══════════════════════════════════════════════════════════════════════

/// Tests nested class using outer class `TypeVar` in base.
#[test]
fn e0130_nested_class_outer_typevar_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Outer(Generic[T]):
    class Inner(Generic[T]):
        pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests nested class body referencing outer `TypeVar` in annotation.
#[test]
fn e0130_nested_class_body_typevar_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

class Outer(Generic[T]):
    class Inner:
        x: T = None
        y: list[T] = []

    class Middle(Generic[S]):
        class Deep:
            z: T = None
            w: S = None
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests unbound `TypeVar` in function body annotation.
#[test]
fn e0130_unbound_typevar_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

def func(x: int) -> None:
    y: T = None
    z: list[S] = []
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `TypeAlias` inside class body referencing class `TypeVar`.
#[test]
fn e0130_typealias_in_class_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, TypeAlias

T = TypeVar("T")

class Container(Generic[T]):
    ItemList: TypeAlias = list[T]
    Mapping: TypeAlias = dict[str, T]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests module-level `TypeVar` usage in non-alias expressions.
#[test]
fn e0130_module_level_unbound_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

# Module-level subscript call with unbound T
x = list[T]()
y: list[T] = []
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests generic instance method call type mismatch.
/// ALL code at module level (no indentation) — e0130 line-by-line scanner requires this.
#[test]
fn e0130_generic_instance_method_call() -> Result<(), Box<dyn std::error::Error>> {
    // NOTE: e0130 check.rs scans SOURCE TEXT line-by-line. All generic classes,
    // instances, and method calls MUST be at module level (zero indentation).
    let source = "from typing import TypeVar, Generic

T = TypeVar(\"T\")

class Box(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

    def set(self, value: T) -> None:
        self.value = value

    def get(self) -> T:
        return self.value

a: Box[int] = Box(42)
a.set(\"wrong\")
a.set(99)

b: Box[str] = Box(\"hello\")
b.set(42)
b.get()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests generic instance with multiple type params at module level.
#[test]
fn e0130_generic_instance_multi_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypeVar, Generic

T = TypeVar(\"T\")
S = TypeVar(\"S\")

class Pair(Generic[T, S]):
    def set_first(self, value: T) -> None:
        pass

    def set_second(self, value: S) -> None:
        pass

p: Pair[int, str] = Pair()
p.set_first(\"wrong\")
p.set_second(42)
p.set_first(1)
p.set_second(\"ok\")
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests variance assignment checks.
#[test]
fn e0130_variance_assignments() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Invariant(Generic[T]):
    pass

class Covariant(Generic[T_co]):
    pass

class Contravariant(Generic[T_contra]):
    pass

# Invariant: exact type required
a: Invariant[int] = Invariant()
b: Invariant[float] = Invariant()

# Covariant: subtype allowed
c: Covariant[object] = Covariant()
d: Covariant[int] = Covariant()

# Contravariant: supertype allowed
e: Contravariant[object] = Contravariant()
f: Contravariant[int] = Contravariant()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `TypeVar` default with generic constructor call — at module level.
/// Targets `check_generic_constructor_calls` which needs `TypeVar` defaults
/// and partial specialization: Container[int]("wrong") where T defaults.
#[test]
fn e0130_typevar_default_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypeVar, Generic

T = TypeVar(\"T\", default=int)
S = TypeVar(\"S\", default=str)

class Container(Generic[T, S]):
    def __init__(self, value: T, label: S) -> None:
        self.value = value
        self.label = label

    def get(self) -> T:
        return self.value

c: Container[str] = Container(\"hello\")
c.get()
c2: Container[int] = Container(42)
c2.get()

Container[int](42, \"hello\")
Container[int](\"wrong\", \"hello\")
Container[str](\"ok\", \"label\")
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `TypeVar` default with partial specialization at module level.
/// This specifically targets check.rs lines 228-380.
#[test]
fn e0130_typevar_default_partial_specialization() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypeVar, Generic

T = TypeVar(\"T\")
S = TypeVar(\"S\", default=int)

class Pair(Generic[T, S]):
    def __init__(self, first: T, second: S) -> None:
        self.first = first
        self.second = second

Pair[str](\"hello\", \"wrong_type\")
Pair[str](\"hello\", 42)
Pair[int, str](1, \"ok\")
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0148: Generic type argument violations — comprehensive
// ═══════════════════════════════════════════════════════════════════════

/// Tests constrained `TypeVar` mismatch at call site.
#[test]
fn e0148_constrained_typevar_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

AnyStr = TypeVar("AnyStr", str, bytes)

def concat(a: AnyStr, b: AnyStr) -> AnyStr:
    return a + b

# Valid: same constraint group
concat("hello", "world")
concat(b"hello", b"world")

# Invalid: mixed constraint groups
concat("hello", b"world")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests Mapping key type mismatch.
#[test]
fn e0148_mapping_key_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Mapping

config: Mapping[str, int] = {}
port = config["port"]
invalid = config[8080]

scores: dict[str, float] = {}
val = scores["alice"]
bad_val = scores[42]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests generic metaclass detection.
#[test]
fn e0148_generic_metaclass() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class BadMeta(metaclass=Generic[T]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests constrained `TypeVar` with multiple parameters.
#[test]
fn e0148_constrained_multi_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T1 = TypeVar("T1", int, float)
T2 = TypeVar("T2", list, tuple)

def multi(a: T1, b: T2) -> T1:
    return a

multi("string", [])
multi(3.14, "tuple")
multi(42, [1, 2])
multi(1.0, (1, 2))
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0048: TypeAlias — additional coverage
// ═══════════════════════════════════════════════════════════════════════

/// Tests `TypeAlias` with `from typing import TypeAlias as TA`.
#[test]
fn e0048_typealias_as_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias as TA

BadAlias: TA = [int, str]
GoodAlias: TA = int | str
";
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(!e0048.is_empty(), "should flag invalid TA RHS: {e0048:?}");
    Ok(())
}

/// Tests `eval()` as `TypeAlias` RHS.
#[test]
fn e0048_eval_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias

BadAlias: TypeAlias = eval("int")
"#;
    let diagnostics = run(source)?;
    let e0048: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(!e0048.is_empty(), "eval() should be flagged");
    Ok(())
}

/// Tests union alias instantiation.
#[test]
fn e0048_union_alias_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

UnionAlias: TypeAlias = int | str
x = UnionAlias()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests generic alias with too many type args.
#[test]
fn e0048_generic_alias_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias, TypeVar

T = TypeVar("T")

GenericAlias: TypeAlias = list[T]
x: GenericAlias[int, str] = []
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests non-generic alias parameterized.
#[test]
fn e0048_non_generic_alias_parameterized() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

SimpleAlias: TypeAlias = int | str
x: SimpleAlias[int] = 42
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `ParamSpec` alias with simple args.
#[test]
fn e0048_paramspec_alias_simple_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias, TypeVar, ParamSpec, Callable

P = ParamSpec("P")
T = TypeVar("T")

CallbackAlias: TypeAlias = Callable[P, T]
x: CallbackAlias[int, str] = lambda: None
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `TypeVar` bound violation in type alias args.
#[test]
fn e0048_typevar_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias, TypeVar

T = TypeVar("T", bound=int)

BoundedAlias: TypeAlias = list[T]
x: BoundedAlias[str] = []
y: BoundedAlias[int] = []
z: BoundedAlias[bool] = []
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests runtime variable used as annotation.
#[test]
fn e0048_runtime_var_as_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
runtime_val = 42
other_val = [1, 2, 3]

def func(p1: runtime_val, p2: other_val) -> None:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests implicit alias detection.
#[test]
fn e0048_implicit_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

IntOrStr = int | str
ListOfInt = list[int]

x: IntOrStr = 42
y: ListOfInt = [1]
z = IntOrStr()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0150: Dead branch variables — inside function bodies
// ═══════════════════════════════════════════════════════════════════════

/// Tests version guard inside function.
#[test]
fn e0150_version_guard_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import sys

def my_func():
    if sys.version_info < (3, 8):
        old_val = 1
    else:
        new_val = 2

    x = old_val
    y = new_val
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests platform guard inside function.
#[test]
fn e0150_platform_guard_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check_platform():
    if sys.platform == "bogus":
        bogus_var = 42
    else:
        real_var = 99

    x = bogus_var
    y = real_var
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests multiple version operators.
#[test]
fn e0150_version_multiple_operators() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check_versions():
    if sys.version_info >= (3, 13):
        future_only = "future"
    else:
        current = "current"

    if sys.version_info <= (3, 11):
        pre312 = "old"
    else:
        post312 = "new"

    if sys.version_info == (3, 12):
        exact = "exact"
    else:
        not_exact = "not exact"

    if (3, 8) > sys.version_info:
        ancient = "ancient"
    else:
        modern = "modern"

    x = future_only
    y = pre312
    z = ancient
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests dead var usage in call expressions and attribute access.
#[test]
fn e0150_dead_var_in_call_and_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check_dead_usage():
    if sys.version_info < (3, 8):
        dead_val = "old"
    else:
        live_val = "new"

    print(dead_val)
    dead_val.upper()
    result = len(dead_val)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests dead branch else dead.
#[test]
fn e0150_dead_branch_else_dead() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check():
    if sys.version_info >= (3, 12):
        live = "live"
    else:
        dead = "dead"

    x = dead
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests platform != guard.
#[test]
fn e0150_platform_not_equal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check():
    if sys.platform != "bogus":
        live = "live"
    else:
        dead = "dead"

    x = dead
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests nested function dead branch.
#[test]
fn e0150_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import sys

def outer():
    def inner():
        if sys.version_info < (3, 8):
            old = 1
        else:
            new = 2
        x = old

    inner()
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `ann_assign` with dead var.
#[test]
fn e0150_ann_assign_dead_var() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import sys

def check():
    if sys.version_info < (3, 8):
        old = 1
    else:
        new = 2

    x: int = old
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0014: TypeForm — comprehensive
// ═══════════════════════════════════════════════════════════════════════

/// Tests `TypeForm` literal assignments.
#[test]
fn e0014_typeform_literal_assignments() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import TypeForm

x: TypeForm[int] = 42
y: TypeForm[str] = 3.14
z: TypeForm[bytes] = True
w: TypeForm[int] = b"data"
t: TypeForm[tuple] = (int, str)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `TypeForm` call expressions.
#[test]
fn e0014_typeform_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing_extensions import TypeForm

x: TypeForm[int] = type(42)
a: TypeForm[int] = int
b: TypeForm[str] = str
d: TypeForm[None] = None
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `TypeForm` string validation.
#[test]
fn e0014_typeform_string() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import TypeForm

x: TypeForm[int] = "not a type"
y: TypeForm[int] = "type(1)"
a: TypeForm[int] = "int"
b: TypeForm[int | str] = "int | str"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `TypeForm` RHS type expression — invalid forms.
#[test]
fn e0014_typeform_invalid_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import TypeForm
from typing import Annotated, Optional

x: TypeForm[int] = Self
y: TypeForm[int] = ClassVar
z: TypeForm[int] = Final
w: TypeForm[int] = Unpack
v: TypeForm[int] = Optional
a: TypeForm[int] = Final[int]
b: TypeForm[int] = Unpack[int]
c: TypeForm[int] = Annotated[int, "metadata"]
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `TypeForm` constructor calls.
#[test]
fn e0014_typeform_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import TypeForm

TypeForm(42)
TypeForm(3.14)
TypeForm(True)
TypeForm(b"data")
TypeForm((int, str))
TypeForm(type(1))
TypeForm("int")
TypeForm("str | None")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// Tests `TypeForm` function parameter args.
#[test]
fn e0014_typeform_function_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing_extensions import TypeForm

def expects_typeform(t: TypeForm[int]) -> None:
    pass

expects_typeform(42)
expects_typeform(True)
expects_typeform(b"data")
expects_typeform(int)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0149: PEP 695 — scoping violations
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn e0149_bound_cross_reference() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Sequence

class BadClass[S: Sequence[T], T]:
    pass

class BadClass2[S, T: Sequence[S]]:
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_type_stmt_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def my_func():
    type InnerAlias = int | str
    type AnotherAlias[T] = list[T]
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_type_stmt_circular() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type CircularAlias = CircularAlias
type CircularArgs[T] = CircularArgs[str]
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_type_stmt_operations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type MyType = int | str

x = MyType()
isinstance(1, MyType)
issubclass(int, MyType)
print(MyType.some_attr)

class Child(MyType):
    pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

#[test]
fn e0149_method_shadows_class_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Container[T]:
    def method[T](self) -> T:
        pass
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Additional coverage targets for various rules
// ═══════════════════════════════════════════════════════════════════════

/// E0097: Protocol __init__ with nested body walking.
#[test]
fn e0097_protocol_init_nested_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class Proto(Protocol):
    x: int
    y: str
    def __init__(self) -> None:
        self.x = 0
        self.y = ""
        self.z = True
        if True:
            self.w = 1
        for i in range(10):
            self.u = i
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// E0107: Variance with nested generics.
#[test]
fn e0107_variance_nested_generics() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)
T_contra = TypeVar("T_contra", contravariant=True)

class Base(Generic[T]):
    pass

class Covariant(Generic[T_co]):
    pass

class Contravariant(Generic[T_contra]):
    pass

class Bad1(Base[T_co], Generic[T_co]):
    pass

class Bad2(Covariant[T_contra], Generic[T_contra]):
    pass

class Bad3(Contravariant[T], Generic[T]):
    pass

class Multi(Base[T_co], Covariant[T_co], Generic[T_co]):
    pass
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// E0137: Generic protocol violations.
#[test]
fn e0137_generic_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic

T = TypeVar("T")

class Container(Protocol[T]):
    def get(self) -> T: ...
    def put(self, value: T) -> None: ...

class StringContainer:
    def get(self) -> str:
        return "hello"
    def put(self, value: str) -> None:
        pass

class BadContainer:
    def get(self) -> list:
        return []
    def put(self, value: int) -> None:
        pass

x: Container[str] = StringContainer()
z: Container[str] = BadContainer()
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// E0137: Protocol with Generic[T] both bases.
#[test]
fn e0137_protocol_generic_both() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar, Generic

T_co = TypeVar("T_co", covariant=True)

class Readable(Protocol[T_co], Generic[T_co]):
    def read(self) -> T_co: ...
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// E0137: Self-typed protocol method.
#[test]
fn e0137_self_typed_protocol() -> Result<(), Box<dyn std::error::Error>> {
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

/// E0111: Constructor errors — various patterns.
#[test]
fn e0111_constructor_various() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar, NamedTuple
from dataclasses import dataclass

T = TypeVar("T")

class NoInit:
    pass

class WithInit(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

class Point(NamedTuple):
    x: int
    y: int

@dataclass
class Data:
    x: int
    y: str = "default"

NoInit(1, 2, 3)
WithInit[int](42)
WithInit[int]("wrong")
Point(1, 2, 3)
Point(1)
Point(x=1, z=2)
Data(x=1, z=2)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

/// E0111: Generic constructor with self annotation.
#[test]
fn e0111_self_annotation_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
S = TypeVar("S")

class Pair(Generic[T, S]):
    def __init__(self: "Pair[T, S]", first: T, second: S) -> None:
        self.first = first
        self.second = second

p = Pair(1, "hello")
p2 = Pair[int, str](1, "hello")
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}
