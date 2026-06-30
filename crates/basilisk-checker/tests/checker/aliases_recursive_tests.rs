//! Tests for [`aliases_recursive`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for aliases_recursive: Cyclical type alias.

use super::common::*;

#[test]
fn non_cyclical_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

IntList: TypeAlias = list[int]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"aliases_recursive"),
        "non-cyclical alias should not fire E0104"
    );
    Ok(())
}
