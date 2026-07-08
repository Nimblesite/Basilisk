//! Tests for [`generics_defaults`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for generics_defaults: Non-default after default `TypeVar`.

use super::common::*;

#[test]
fn non_default_after_default_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T", default=int)
U = TypeVar("U")
class Foo(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_defaults"),
        "non-default TypeVar after default should fire E0030, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn all_defaults_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T", default=int)
U = TypeVar("U", default=str)
class Foo(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_defaults"),
        "all default TypeVars should not fire E0030"
    );
    Ok(())
}
