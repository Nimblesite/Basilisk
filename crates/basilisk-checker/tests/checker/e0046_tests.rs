// Integration tests for BSK-E0046: Enum member annotated (covered also in `e0040_e0046`).

use super::common::*;

#[test]
fn e0046_annotated_enum_member_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED: int = 1
    GREEN: int = 2
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0046"),
        "annotated enum member should fire E0046, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0046_unannotated_enum_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0046"),
        "unannotated enum member should not fire E0046"
    );
    Ok(())
}
