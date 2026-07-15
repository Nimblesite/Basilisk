//! Tests for [BSK-0040] from [CHKARCH-DIAG-IMMUTABILITY] / [TYPEINF-FUNC-LAMBDA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for BSK-0040: Lambda missing type annotations.

use super::common::*;

#[test]
fn unannotated_lambda_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "f = lambda x: x + 1\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0040"),
        "unannotated lambda should fire BSK-0040, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn annotated_lambda_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import Callable\nf: Callable[[int], int] = lambda x: x + 1\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0040"),
        "annotated lambda should not fire BSK-0040"
    );
    Ok(())
}

#[test]
fn lambda_is_warning_not_error() -> Result<(), Box<dyn std::error::Error>> {
    let source = "f = lambda x: x + 1\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let w0040 = diags.iter().find(|d| d.code.code == "BSK-0040");
    assert!(w0040.is_some(), "should fire BSK-0040");
    let Some(diag) = w0040 else {
        return Err("BSK-0040 diagnostic missing after assertion".into());
    };
    assert_eq!(
        diag.severity,
        basilisk_checker::Severity::Warning,
        "BSK-0040 should be a warning"
    );
    Ok(())
}

#[test]
fn enum_body_lambda_exempt() -> Result<(), Box<dyn std::error::Error>> {
    // Enum bodies legitimately assign bare lambdas as non-member callables;
    // annotating them is discouraged, so BSK-0040 must not fire
    // (conformance enums_members.py).
    let source = "from enum import Enum\nclass Color(Enum):\n    RED = 1\n    converter = lambda x: str(x)\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0040"),
        "lambda in an enum body must not fire BSK-0040, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn non_enum_class_lambda_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The enum exemption must NOT leak to ordinary classes.
    let source = "class Plain:\n    converter = lambda x: str(x)\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0040"),
        "lambda in a non-enum class must still fire BSK-0040, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
