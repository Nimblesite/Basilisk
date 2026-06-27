//! Tests for [directives_assert_type] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for directives_assert_type: Invalid `assert_type()` call.

use super::common::*;

#[test]
fn valid_assert_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import assert_type

x: int = 42
assert_type(x, int)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_assert_type"),
        "valid assert_type call should not fire E0039"
    );
    Ok(())
}

#[test]
fn assert_type_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import assert_type
assert_type()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn assert_type_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import assert_type
x: int = 42
assert_type(x, int, "extra")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
