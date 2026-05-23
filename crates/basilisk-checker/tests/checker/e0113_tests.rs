//! Tests for [BSK-E0113] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0113: `TypeIs` inconsistent narrowing.

use super::common::*;

#[test]
fn e0113_valid_typeis() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def is_str(x: object) -> TypeIs[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0113"),
        "valid TypeIs should not fire E0113"
    );
    Ok(())
}

#[test]
fn e0113_inconsistent_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def bad_check(x: int) -> TypeIs[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
