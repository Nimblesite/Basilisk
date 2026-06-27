//! Tests for [`literals_semantics`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for literals_semantics: Literal augmented assignment.

use super::common::*;

#[test]
fn normal_augmented_assignment_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(x: int) -> None:
    x += 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"literals_semantics"),
        "normal augmented assignment should not fire E0100"
    );
    Ok(())
}
