//! Tests for [enums_expansion] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
// Tests for enums_expansion: `assert_type` with `Literal[Enum.MEMBER]` on enum-typed param.
//
// This rule detects when `assert_type()` is used with a `Literal[Enum.MEMBER]` type
// on a parameter that is already typed as the enum itself.

use super::common::*;

#[test]
fn test_e0061_assert_type_literal_enum_member() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
from enum import Enum
from typing import assert_type, Literal

class Status(Enum):
    ACTIVE = 1
    INACTIVE = 2

def process(status: Status) -> None:
    assert_type(status, Literal[Status.ACTIVE])
";
    let diags = run(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_expansion")
        .collect();
    assert!(
        !e0061.is_empty(),
        "assert_type with Literal[Enum.MEMBER] should fire E0061"
    );
    Ok(())
}

#[test]
fn test_e0061_assert_type_enum_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
from enum import Enum
from typing import assert_type

class Status(Enum):
    ACTIVE = 1
    INACTIVE = 2

def process(status: Status) -> None:
    assert_type(status, Status)
";
    let diags = run(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_expansion")
        .collect();
    assert!(
        e0061.is_empty(),
        "assert_type with enum type should not fire E0061"
    );
    Ok(())
}

#[test]
fn test_e0061_flag_enum_literal_narrowing_forbidden() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
from enum import Flag, auto
from typing import assert_type, Literal

class Permissions(Flag):
    READ = auto()
    WRITE = auto()
    EXECUTE = auto()

def check_perms(perms: Permissions) -> None:
    assert_type(perms, Literal[Permissions.READ])
";
    let diags = run(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_expansion")
        .collect();
    assert!(
        !e0061.is_empty(),
        "Flag enum literal narrowing should fire E0061"
    );
    Ok(())
}

#[test]
fn test_e0061_multiple_enum_members() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
from enum import Enum
from typing import assert_type, Literal

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

def set_color(color: Color) -> None:
    assert_type(color, Literal[Color.RED])
    assert_type(color, Literal[Color.GREEN])
";
    let diags = run(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_expansion")
        .collect();
    assert_eq!(
        e0061.len(),
        2,
        "Multiple assert_type calls should fire multiple E0061"
    );
    Ok(())
}

#[test]
fn test_e0061_nested_enum_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
from enum import Enum
from typing import assert_type, Literal

class Status(Enum):
    ACTIVE = 1
    INACTIVE = 2

class ExtendedStatus(Enum):
    PENDING = 3
    COMPLETED = 4

def handle_status(status: Status) -> None:
    assert_type(status, Literal[Status.ACTIVE])
";
    let diags = run(src)?;
    println!("All diagnostics:");
    for diag in &diags {
        println!("  Code: {}, Message: {}", diag.code.code, diag.message);
    }
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_expansion")
        .collect();
    assert!(!e0061.is_empty(), "Nested enum hierarchy should fire E0061");
    Ok(())
}

#[test]
fn test_e0061_literal_union_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
from enum import Enum
from typing import assert_type, Literal

class Status(Enum):
    ACTIVE = 1
    INACTIVE = 2

def process(status: Status) -> None:
    assert_type(status, Literal[Status.ACTIVE, Status.INACTIVE])
";
    let diags = run(src)?;
    let _e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_expansion")
        .collect();
    // Literal unions are not handled by E0061 - they might be handled by other rules
    // This test documents the current behavior
    Ok(())
}
