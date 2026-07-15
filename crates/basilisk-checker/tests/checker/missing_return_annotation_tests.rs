//! Tests for [BSK-0002] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
// Integration tests for BSK-0002: Missing return type annotation.

use super::common::*;

#[test]
fn missing_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str):
    return name
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0002"),
        "function without return annotation should fire BSK-0002, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn with_return_annotation_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str) -> str:
    return name
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0002"),
        "function with return annotation should not fire BSK-0002"
    );
    Ok(())
}

#[test]
fn none_return_annotation_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def do_nothing() -> None:
    pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0002"),
        "function with -> None should not fire BSK-0002"
    );
    Ok(())
}
