// Integration tests for BSK-E0065: Float param int attr access.

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
        !codes(&diags).contains(&"BSK-E0065"),
        "int has numerator attr, should not fire E0065"
    );
    Ok(())
}
