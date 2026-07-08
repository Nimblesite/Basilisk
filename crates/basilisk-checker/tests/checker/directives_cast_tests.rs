//! Tests for [`directives_cast`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for directives_cast: Invalid `cast()` call.

use super::common::*;

#[test]
fn cast_literal_first_arg_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast
x: int = 1
y = cast(1, x)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_cast"),
        "cast with literal first arg should fire E0031, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn cast_too_few_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast
y = cast()
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_cast"),
        "cast() with no args should fire E0031, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn cast_valid_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast
x: int = 1
y = cast(str, x)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_cast"),
        "valid cast should not fire E0031"
    );
    Ok(())
}
