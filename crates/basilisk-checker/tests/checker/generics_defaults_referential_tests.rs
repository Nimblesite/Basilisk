//! Tests for [generics_defaults_referential] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_defaults_referential: `TypeVar` default referential violation.

use super::common::*;

#[test]
fn valid_typevar_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T", default=int)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_defaults_referential"),
        "valid TypeVar default should not fire E0102"
    );
    Ok(())
}
