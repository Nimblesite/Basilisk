//! Tests for [tuples_type_form] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for tuples_type_form: Multiple unbounded tuple components.

use super::common::*;

#[test]
fn single_unbounded_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Unpack
Ts = TypeVarTuple("Ts")

def f(x: tuple[int, *tuple[str, ...], float]) -> None:
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_type_form"),
        "single unbounded component should not fire E0049"
    );
    Ok(())
}

#[test]
fn no_unbounded_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(x: tuple[int, str, float]) -> None:
    pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_type_form"),
        "no unbounded component should not fire E0049"
    );
    Ok(())
}
