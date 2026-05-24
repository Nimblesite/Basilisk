//! Tests for [BSK-E0018] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0018: Undefined variable in return.

use super::common::*;

#[test]
fn e0018_undefined_name_in_return_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute() -> int:\n    return undefined_name\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0018"),
        "undefined name in return should fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0018_defined_param_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute(x: int) -> int:\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "returning a parameter should not fire E0018"
    );
    Ok(())
}

#[test]
fn e0018_locally_assigned_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute() -> int:\n    result = 42\n    return result\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "returning a locally assigned variable should not fire E0018"
    );
    Ok(())
}

#[test]
fn e0018_module_level_variable_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
_EMPTY_TEXT_MSG = \"text is required\"

def validate(text: str) -> str:
    if not text:
        return _EMPTY_TEXT_MSG
    return text
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "returning a module-level variable should not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0018_diagnostic_has_help() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute() -> int:\n    return missing\n";
    let diags = run(source)?;
    let e0018 = diags.iter().find(|d| d.code.code == "BSK-E0018");
    assert!(e0018.is_some(), "should fire E0018");
    let Some(diag) = e0018 else {
        return Err("E0018 diagnostic missing after assertion".into());
    };
    assert!(diag.help.is_some(), "E0018 should have help text");
    Ok(())
}
