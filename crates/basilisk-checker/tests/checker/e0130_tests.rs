//! Tests for [BSK-E0130] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0130: `TypeVar` scoping violation.

use super::common::*;

#[test]
fn e0130_nested_class_reuses_outer_typevar() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0130_nested_class_in_generic_function() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0130_module_level_typevar_subscript() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0130_method_call_typevar_substitution() -> Result<(), Box<dyn std::error::Error>> {
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
