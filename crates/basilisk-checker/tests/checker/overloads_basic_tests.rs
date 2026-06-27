//! Tests for [`overloads_basic`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
// Integration tests for overloads_basic: No matching overload.

use super::common::*;

#[test]
fn no_matching_overload_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: int | str) -> int | str:
    return x

process(1.0)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn matching_overload_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: int | str) -> int | str:
    return x

process(1)
process('hello')
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"overloads_basic"),
        "matching overload should not fire E0072"
    );
    Ok(())
}
