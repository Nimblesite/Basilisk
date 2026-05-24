//! Tests for [BSK-E0030] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for BSK-E0030: Non-default after default `TypeVar`.

use super::common::*;

#[test]
fn e0030_non_default_after_default_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T", default=int)
U = TypeVar("U")
class Foo(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0030"),
        "non-default TypeVar after default should fire E0030, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0030_all_defaults_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T", default=int)
U = TypeVar("U", default=str)
class Foo(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0030"),
        "all default TypeVars should not fire E0030"
    );
    Ok(())
}
