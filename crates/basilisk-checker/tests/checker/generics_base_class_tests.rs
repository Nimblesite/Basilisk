//! Tests for [generics_base_class] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for generics_base_class: Duplicate `TypeVar` in Generic[...].

use super::common::*;

#[test]
fn duplicate_typevar_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class Foo(Generic[T, T]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_base_class"),
        "duplicate TypeVar in Generic should fire E0027, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn unique_typevars_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
U = TypeVar("U")
class Foo(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_base_class"),
        "unique TypeVars should not fire E0027"
    );
    Ok(())
}
