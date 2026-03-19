// Integration tests for BSK-E0067: Enum non-member literal.

use super::common::*;

#[test]
fn e0067_enum_non_member_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0067_valid_enum_members() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum
class Color(Enum):
    RED = 1
    GREEN = 2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0067"),
        "valid enum members should not fire E0067"
    );
    Ok(())
}
