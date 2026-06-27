//! Tests for [generics_syntax_declarations_2] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_syntax_declarations_2: Bounded type var attribute access.

use super::common::*;

#[test]
fn e0105_valid_attr_on_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class C[T: str]:
    def method(self, x: T) -> str:
        return x.upper()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_syntax_declarations_2"),
        "accessing valid str method on str-bounded typevar should not fire E0105"
    );
    Ok(())
}

#[test]
fn e0105_invalid_attr_on_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class C[T: str]:
    def method(self, x: T) -> None:
        x.is_integer()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
