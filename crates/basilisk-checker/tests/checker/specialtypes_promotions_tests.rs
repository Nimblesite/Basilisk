//! Tests for [specialtypes_promotions] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
// Integration tests for specialtypes_promotions: Float param int attr access.

use super::common::*;

#[test]
fn e0065_float_int_attr_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: float) -> int:
    return x.numerator
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0065_int_attr_valid() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: int) -> int:
    return x.numerator
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"specialtypes_promotions"),
        "int has numerator attr, should not fire E0065"
    );
    Ok(())
}
