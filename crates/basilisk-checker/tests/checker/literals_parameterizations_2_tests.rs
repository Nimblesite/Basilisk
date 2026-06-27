//! Tests for [`literals_parameterizations_2`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
// Integration tests for literals_parameterizations_2: Literal string enum.

use super::common::*;

#[test]
fn literal_string_enum_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import StrEnum
class Status(StrEnum):
    ACTIVE = "active"
    INACTIVE = "inactive"

def func(s: Status) -> None:
    pass

func("active")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
