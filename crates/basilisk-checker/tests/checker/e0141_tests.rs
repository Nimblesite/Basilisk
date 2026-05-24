//! Tests for [BSK-E0141] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0141: Unpack kwargs violations.

use super::common::*;

#[test]
fn e0141_valid_unpack_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict, Unpack

class Config(TypedDict):
    name: str
    value: int

def func(**kwargs: Unpack[Config]) -> None:
    pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0141"),
        "valid Unpack kwargs should not fire E0141"
    );
    Ok(())
}

#[test]
fn e0141_overlap_with_positional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict, Unpack

class Config(TypedDict):
    name: str
    value: int

def func(name: str, **kwargs: Unpack[Config]) -> None:
    pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
