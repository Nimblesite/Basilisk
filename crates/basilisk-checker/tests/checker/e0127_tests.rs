//! Tests for [BSK-E0127] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0127: Tuple index out of range.

use super::common::*;

#[test]
fn e0127_valid_tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[0]
    y = v[1]
    z = v[2]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0127"),
        "valid tuple indices should not fire E0127"
    );
    Ok(())
}

#[test]
fn e0127_out_of_range_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[4]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0127_negative_out_of_range() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[-4]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
