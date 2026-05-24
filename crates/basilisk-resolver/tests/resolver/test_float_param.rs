//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_float_param`.

use super::common::resolve_src;

#[test]
fn float_param_int_attr_access_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def process(x: float) -> None:\n", "    x.numerator\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.float_param_int_attr_accesses.is_empty(),
        "accessing int-only attribute on float param must be detected"
    );
    Ok(())
}

#[test]
fn float_param_valid_attr_no_detection() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def process(x: float) -> None:\n", "    x.is_integer()\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.float_param_int_attr_accesses.is_empty());
    Ok(())
}

#[test]
fn float_param_int_attr_numerator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo(x: float) -> None:\n", "    x.numerator\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.float_param_int_attr_accesses.is_empty());
    Ok(())
}
