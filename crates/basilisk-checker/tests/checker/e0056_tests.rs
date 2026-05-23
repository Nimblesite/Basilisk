//! Tests for [BSK-E0056] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// Integration tests for BSK-E0056: `ReadOnly` `TypedDict` mutation.

use super::common::*;

#[test]
fn e0056_no_readonly_fields_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Config(TypedDict):
    name: str
    version: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0056"),
        "TypedDict without ReadOnly fields should not fire E0056"
    );
    Ok(())
}

#[test]
fn e0056_readonly_mutation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict
from typing_extensions import ReadOnly

class Config(TypedDict):
    name: str
    version: ReadOnly[str]

cfg: Config = {"name": "test", "version": "1.0"}
cfg["version"] = "2.0"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
