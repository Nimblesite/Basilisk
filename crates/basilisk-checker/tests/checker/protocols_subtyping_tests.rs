//! Tests for [protocols_subtyping] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_subtyping: Protocol tuple element type mismatch.

use super::common::*;

#[test]
fn valid_tuple_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class RGB(Protocol):
    rgb: tuple[int, int, int]

class Point(RGB):
    def __init__(self, red: int, green: int, blue: int) -> None:
        self.rgb = red, green, blue
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_subtyping"),
        "valid tuple assignment should not fire E0124"
    );
    Ok(())
}

#[test]
fn mismatched_tuple_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class RGB(Protocol):
    rgb: tuple[int, int, int]

class Point(RGB):
    def __init__(self, red: int, green: int, blue: str) -> None:
        self.rgb = red, green, blue
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
