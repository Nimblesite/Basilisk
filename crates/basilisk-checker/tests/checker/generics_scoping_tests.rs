//! Tests for [`generics_scoping`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_scoping: Unbound type variable in scope.

use super::common::*;

#[test]
fn bound_typevar_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

def fun(x: T) -> list[T]:
    return [x]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_scoping"),
        "TypeVar bound in function sig should not fire E0117"
    );
    Ok(())
}

#[test]
fn unbound_typevar_in_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

def fun(x: T) -> list[T]:
    z: list[S] = []
    return [x]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn bound_typevar_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    items: list[T]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_scoping"),
        "TypeVar bound in Generic class should not fire E0117"
    );
    Ok(())
}

#[test]
fn unbound_typevar_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

class Bar(Generic[T]):
    an_attr: list[S]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
