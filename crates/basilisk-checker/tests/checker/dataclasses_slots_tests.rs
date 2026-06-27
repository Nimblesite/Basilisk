//! Tests for [dataclasses_slots] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for dataclasses_slots: Dataclass slots violations.

use super::common::*;

#[test]
fn no_slots_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class DC:
    x: int
    y: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_slots"),
        "dataclass without slots=True should not fire E0108"
    );
    Ok(())
}

#[test]
fn slots_valid_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(slots=True)
class DC:
    x: int
    y: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_slots"),
        "valid dataclass with slots=True should not fire E0108"
    );
    Ok(())
}

#[test]
fn slots_invalid_attr() -> Result<(), Box<dyn std::error::Error>> {
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
fn slots_access_on_non_slots() -> Result<(), Box<dyn std::error::Error>> {
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

#[test]
fn slots_true_with_manual_slots_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from dataclasses import dataclass\n@dataclass(slots=True)\nclass C:\n    x: int\n    __slots__ = ()\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"dataclasses_slots"),
        "@dataclass(slots=True) plus a manual __slots__ must fire E0108, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn slots_true_without_manual_slots_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "from dataclasses import dataclass\n@dataclass(slots=True)\nclass D:\n    x: int\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_slots"),
        "slots=True alone must not fire the already-defined check"
    );
    Ok(())
}
