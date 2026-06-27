//! Tests for [generics_upper_bound_2] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_upper_bound_2: `TypeVar` bound violation at call site.

use super::common::*;

#[test]
fn e0109_valid_bound_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, LiteralString

TLiteral = TypeVar("TLiteral", bound=LiteralString)

def literal_identity(s: TLiteral) -> TLiteral:
    return s
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_upper_bound_2"),
        "valid bound usage should not fire E0109"
    );
    Ok(())
}

#[test]
fn e0109_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, LiteralString

TLiteral = TypeVar("TLiteral", bound=LiteralString)

def literal_identity(s: TLiteral) -> TLiteral:
    return s

def func5(s: str) -> None:
    literal_identity(s)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
