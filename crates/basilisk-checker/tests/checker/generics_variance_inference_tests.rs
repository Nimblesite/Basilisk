//! Tests for [`generics_variance_inference`] from [CHKARCH-DIAG-CATEGORIES] and
//! [TYPEINF-GENERICS-VARIANCE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_variance_inference: `TypeVar` scoping violation.

use super::common::*;

#[test]
fn nested_class_reuses_outer_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class Outer(Generic[T]):
    class Inner(Generic[T]):
        pass
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn nested_class_in_generic_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
def func(x: T) -> T:
    class Inner(Generic[T]):
        pass
    return x
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn module_level_typevar_subscript() -> Result<(), Box<dyn std::error::Error>> {
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
fn method_call_typevar_substitution() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class MyClass(Generic[T]):
    def meth(self, x: T) -> T:
        return x

a: MyClass[int] = MyClass()
a.meth("str")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn pep695_variance_is_inferred_from_usage_positions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Readable[T]:
    def read(self) -> T: ...

class Consumer[T]:
    def consume(self, item: T) -> None: ...

class Stack[T]:
    def push(self, item: T) -> None: ...
    def pop(self) -> T: ...

readable_ok: Readable[float] = Readable[int]()
readable_bad: Readable[int] = Readable[float]()
consumer_ok: Consumer[int] = Consumer[float]()
consumer_bad: Consumer[float] = Consumer[int]()
stack_bad: Stack[float] = Stack[int]()
"#;
    let diags = run(source)?;
    let variance_diags = diags
        .iter()
        .filter(|diag| diag.code.code == "generics_variance_inference")
        .count();
    assert_eq!(
        variance_diags, 3,
        "covariant, contravariant, and invariant assignments must follow inferred variance: {diags:?}"
    );
    Ok(())
}
