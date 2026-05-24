//! Tests for [BSK-E0102] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0102: `TypeVar` default referential violation.

use super::common::*;

#[test]
fn e0102_valid_typevar_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", default=int)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0102"),
        "valid TypeVar default should not fire E0102"
    );
    Ok(())
}
