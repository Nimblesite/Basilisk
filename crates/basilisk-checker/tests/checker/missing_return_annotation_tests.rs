//! Tests for [BSK-E0002] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
// Integration tests for BSK-E0002: Missing return type annotation.

use super::common::*;

#[test]
fn e0002_missing_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str):
    return name
";
    let diags = run_strict(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0002"),
        "function without return annotation should fire E0002, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0002_with_return_annotation_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str) -> str:
    return name
";
    let diags = run_strict(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0002"),
        "function with return annotation should not fire E0002"
    );
    Ok(())
}

#[test]
fn e0002_none_return_annotation_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def do_nothing() -> None:
    pass
";
    let diags = run_strict(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0002"),
        "function with -> None should not fire E0002"
    );
    Ok(())
}
