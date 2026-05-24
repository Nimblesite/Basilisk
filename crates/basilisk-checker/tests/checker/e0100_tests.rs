//! Tests for [BSK-E0100] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0100: Literal augmented assignment.

use super::common::*;

#[test]
fn e0100_normal_augmented_assignment_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(x: int) -> None:
    x += 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0100"),
        "normal augmented assignment should not fire E0100"
    );
    Ok(())
}
