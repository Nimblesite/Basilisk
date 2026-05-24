//! Tests for [BSK-E0066] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
// Integration tests for BSK-E0066: Enum value type mismatch.

use super::common::*;

#[test]
fn e0066_enum_value_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import IntEnum
class Color(IntEnum):
    RED = 'not_an_int'
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0066_valid_int_enum() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import IntEnum
class Color(IntEnum):
    RED = 1
    GREEN = 2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0066"),
        "valid IntEnum values should not fire E0066"
    );
    Ok(())
}
