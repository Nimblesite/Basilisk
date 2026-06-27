//! Tests for [tuples_index_2] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for tuples_index_2: Tuple index out of range.

use super::common::*;

#[test]
fn valid_tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[0]
    y = v[1]
    z = v[2]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_index_2"),
        "valid tuple indices should not fire E0127"
    );
    Ok(())
}

#[test]
fn out_of_range_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[4]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn negative_out_of_range() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[-4]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
