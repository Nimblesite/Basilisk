//! Tests for [BSK-E0090] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Integration tests for BSK-E0090: Invalid tuple syntax.

use super::common::*;

#[test]
fn e0090_invalid_tuple_syntax_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: tuple[int, ..., str] = (1, 2, 'a')
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0090_valid_tuple_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: tuple[int, str] = (1, 'a')
y: tuple[int, ...] = (1, 2, 3)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0090"),
        "valid tuple syntax should not fire E0090"
    );
    Ok(())
}
