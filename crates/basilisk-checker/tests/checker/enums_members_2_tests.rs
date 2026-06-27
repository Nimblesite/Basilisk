//! Tests for [enums_members_2] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
// Integration tests for enums_members_2: Enum non-member literal.

use super::common::*;

#[test]
fn enum_non_member_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum, nonmember
class Color(Enum):
    RED = 1
    description = nonmember('A color')
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn valid_enum_members() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum
class Color(Enum):
    RED = 1
    GREEN = 2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"enums_members_2"),
        "valid enum members should not fire E0067"
    );
    Ok(())
}
