//! Tests for [enums_behaviors] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for enums_behaviors: Invalid Enum subclassing.

use super::common::*;

#[test]
fn valid_enum() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"enums_behaviors"),
        "valid enum should not fire E0040"
    );
    Ok(())
}

#[test]
fn enum_with_members_subclassed() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2

class ExtendedColor(Color):
    BLUE = 3
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn memberless_enum_base_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Base(Enum):
    pass

class Child(Base):
    VALUE = 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"enums_behaviors"),
        "subclassing memberless enum should not fire E0040"
    );
    Ok(())
}
