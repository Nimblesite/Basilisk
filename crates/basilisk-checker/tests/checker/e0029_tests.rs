//! Tests for [BSK-E0029] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0029: Method defined in `TypedDict`.

use super::common::*;

#[test]
fn e0029_method_in_typeddict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

    def display(self) -> str:
        return self["name"]
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0029"),
        "method in TypedDict should fire E0029, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0029_typeddict_fields_only_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0029"),
        "TypedDict with only fields should not fire E0029"
    );
    Ok(())
}
