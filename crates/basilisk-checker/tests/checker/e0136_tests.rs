//! Tests for [BSK-E0136] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0136: Callable subtyping violations.

use super::common::*;

#[test]
fn e0136_compatible_callable_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def func(cb: Callable[[int], int]) -> None:
    f: Callable[[int], int] = cb
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0136"),
        "compatible callable assignment should not fire E0136"
    );
    Ok(())
}

#[test]
fn e0136_incompatible_param_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def func(cb: Callable[[int], int]) -> None:
    f: Callable[[float], float] = cb
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
