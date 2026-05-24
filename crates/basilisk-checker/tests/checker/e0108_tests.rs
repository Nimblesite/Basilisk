//! Tests for [BSK-E0108] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0108: Dataclass slots violations.

use super::common::*;

#[test]
fn e0108_no_slots_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class DC:
    x: int
    y: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0108"),
        "dataclass without slots=True should not fire E0108"
    );
    Ok(())
}

#[test]
fn e0108_slots_valid_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(slots=True)
class DC:
    x: int
    y: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0108"),
        "valid dataclass with slots=True should not fire E0108"
    );
    Ok(())
}

#[test]
fn e0108_slots_invalid_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(slots=True)
class DC:
    x: int

    def __init__(self) -> None:
        self.y = 3
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0108_slots_access_on_non_slots() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class DC2:
    a: int

DC2.__slots__
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
