//! Tests for [enums_members] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for enums_members: Enum member annotated (covered also in `e0040_e0046`).

use super::common::*;

#[test]
fn annotated_enum_member_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED: int = 1
    GREEN: int = 2
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"enums_members"),
        "annotated enum member should fire E0046, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn unannotated_enum_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"enums_members"),
        "unannotated enum member should not fire E0046"
    );
    Ok(())
}
