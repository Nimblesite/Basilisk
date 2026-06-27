//! Tests for [`directives_reveal_type`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for directives_reveal_type: Invalid `reveal_type()` call.

use super::common::*;

#[test]
fn valid_reveal_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: int = 42
reveal_type(x)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_reveal_type"),
        "valid reveal_type call should not fire E0033"
    );
    Ok(())
}

#[test]
fn reveal_type_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
reveal_type()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn reveal_type_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 42
y: str = "hi"
reveal_type(x, y)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
