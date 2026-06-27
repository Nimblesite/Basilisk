//! Tests for [`overloads_consistency`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for overloads_consistency: Overlapping @overload signatures.

use super::common::*;

#[test]
fn identical_unannotated_overloads_fires() -> Result<(), Box<dyn std::error::Error>> {
    // Overlap detection requires at least one side to have unannotated params
    let source = r"
from typing import overload

@overload
def process(x) -> int: ...

@overload
def process(x) -> str: ...

def process(x: int) -> int:
    return x
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"overloads_consistency"),
        "identical unannotated overloads should fire E0021, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn distinct_overloads_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...

@overload
def process(x: str) -> str: ...

def process(x: int | str) -> int | str:
    return x
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"overloads_consistency"),
        "distinct overloads should not fire E0021"
    );
    Ok(())
}
