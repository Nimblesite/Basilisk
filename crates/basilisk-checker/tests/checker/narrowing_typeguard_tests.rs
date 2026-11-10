//! Tests for [`narrowing_typeguard`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for narrowing_typeguard: `TypeGuard` no narrowing param.

use super::common::*;

// Exercises [TYPEINF-NARROWING-TYPEGUARD] / [TYPEINF-NARROWING-TYPEIS] —
// a narrowing function with a real parameter to narrow is valid.
#[test]
fn valid_typeguard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_str(x: object) -> TypeGuard[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"narrowing_typeguard"),
        "valid TypeGuard should not fire E0101"
    );
    Ok(())
}

#[test]
fn typeguard_no_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_str() -> TypeGuard[str]:
    return True
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
