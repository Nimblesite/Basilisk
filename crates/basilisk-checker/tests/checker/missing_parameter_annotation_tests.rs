//! Tests for [BSK-E0001] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
// Integration tests for BSK-E0001: Missing parameter type annotation.

use super::common::*;

#[test]
fn missing_param_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name):
    return name
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-E0001"),
        "unannotated parameter should fire E0001, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn annotated_param_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str) -> str:
    return name
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0001"),
        "annotated parameter should not fire E0001"
    );
    Ok(())
}

#[test]
fn self_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Foo:
    def method(self) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0001"),
        "self parameter should not fire E0001"
    );
    Ok(())
}

#[test]
fn cls_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Foo:
    @classmethod
    def method(cls) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0001"),
        "cls parameter should not fire E0001"
    );
    Ok(())
}

#[test]
fn multiple_unannotated_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def add(a, b):
    return a + b
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0001_count = codes(&diags).iter().filter(|c| **c == "BSK-E0001").count();
    assert!(
        e0001_count >= 2,
        "two unannotated params should fire E0001 at least twice, got {e0001_count}"
    );
    Ok(())
}
