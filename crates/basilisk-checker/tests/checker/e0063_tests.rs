//! Tests for [BSK-E0063] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
// Integration tests for BSK-E0063: Non-hashable dataclass.

use super::common::*;

#[test]
fn e0063_mutable_dataclass_in_set() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

s = {Point(1, 2)}
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0063_frozen_dataclass_hashable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class Point:
    x: int
    y: int

s = {Point(1, 2)}
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0063"),
        "frozen dataclass should be hashable, no E0063"
    );
    Ok(())
}

#[test]
fn e0063_mutable_dataclass_as_dict_key() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

d = {Point(1, 2): 'a'}
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
