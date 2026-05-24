//! Tests for [BSK-E0019] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0019: Unbound variable on some code paths.

use super::common::*;

#[test]
fn e0019_conditionally_assigned_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def maybe_assign(flag: bool) -> int:
    if flag:
        result = 42
    return result
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0019"),
        "conditionally assigned variable should fire E0019, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0019_unconditionally_assigned_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def always_assign() -> int:
    result = 42
    return result
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0019"),
        "unconditionally assigned variable should not fire E0019"
    );
    Ok(())
}

#[test]
fn e0019_parameter_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def identity(x: int) -> int:\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0019"),
        "parameter should not fire E0019"
    );
    Ok(())
}
