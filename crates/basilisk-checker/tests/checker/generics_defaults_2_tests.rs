//! Tests for [generics_defaults_2] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Integration tests for generics_defaults_2: `TypeVar` default incompatible.

use super::common::*;

#[test]
fn e0091_typevar_default_incompat_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", bound=int, default=str)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0091_valid_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", bound=int, default=int)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_defaults_2"),
        "compatible default should not fire E0091"
    );
    Ok(())
}
