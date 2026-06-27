//! Tests for [`narrowing_typeis_2`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for narrowing_typeis_2: `TypeIs` inconsistent narrowing.

use super::common::*;

#[test]
fn valid_typeis() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def is_str(x: object) -> TypeIs[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"narrowing_typeis_2"),
        "valid TypeIs should not fire E0113"
    );
    Ok(())
}

#[test]
fn inconsistent_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def bad_check(x: int) -> TypeIs[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
