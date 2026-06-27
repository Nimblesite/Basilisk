//! Tests for [`callables_kwargs`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for callables_kwargs: Unpack kwargs violations.

use super::common::*;

#[test]
fn valid_unpack_kwargs() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"callables_kwargs"),
        "valid Unpack kwargs should not fire E0141"
    );
    Ok(())
}

#[test]
fn overlap_with_positional() -> Result<(), Box<dyn std::error::Error>> {
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
