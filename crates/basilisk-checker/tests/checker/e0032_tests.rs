//! Tests for [BSK-E0032] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for BSK-E0032: Invalid `TypedDict` keyword.

use super::common::*;

#[test]
fn e0032_invalid_keyword_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Movie(TypedDict, metaclass=type):
    name: str
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0032"),
        "invalid keyword in TypedDict should fire E0032, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0032_total_keyword_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Movie(TypedDict, total=False):
    name: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0032"),
        "total keyword should not fire E0032"
    );
    Ok(())
}
