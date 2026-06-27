//! Tests for [annotations_typeexpr] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for annotations_typeexpr: Invalid type form (numeric literal as annotation).

use super::common::*;

#[test]
fn numeric_literal_param_annotation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: 42) -> None:\n    pass\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"annotations_typeexpr"),
        "numeric literal param annotation should fire E0024, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn numeric_literal_return_annotation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: int) -> 0:\n    pass\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"annotations_typeexpr"),
        "numeric literal return annotation should fire E0024, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn normal_type_annotation_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: int) -> str:\n    return str(x)\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_typeexpr"),
        "normal type annotation should not fire E0024"
    );
    Ok(())
}
