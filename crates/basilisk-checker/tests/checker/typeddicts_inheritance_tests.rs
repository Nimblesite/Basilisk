//! Tests for [typeddicts_inheritance] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for typeddicts_inheritance: Invalid `TypedDict` inheritance.

use super::common::*;

#[test]
fn e0038_typeddict_conflicting_field_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Base(TypedDict):
    name: str

class Child(Base):
    name: int
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"typeddicts_inheritance"),
        "TypedDict field type conflict should fire E0038, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0038_typeddict_compatible_inheritance_no_diagnostic() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
from typing import TypedDict

class Base(TypedDict):
    name: str

class Child(Base):
    age: int
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"typeddicts_inheritance"),
        "compatible TypedDict inheritance should not fire E0038"
    );
    Ok(())
}
