//! Tests for [BSK-E0088] from [CHKARCH-DIAG-UNUSED]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-UNUSED
// Integration tests for BSK-E0088: `TypedDict` isinstance.

use super::common::*;

#[test]
fn e0088_isinstance_typeddict_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class TD(TypedDict):
    name: str

x: object = {}
isinstance(x, TD)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
