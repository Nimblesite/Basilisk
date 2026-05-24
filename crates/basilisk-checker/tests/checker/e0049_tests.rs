//! Tests for [BSK-E0049] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for BSK-E0049: Multiple unbounded tuple components.

use super::common::*;

#[test]
fn e0049_single_unbounded_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Unpack
Ts = TypeVarTuple("Ts")

def f(x: tuple[int, *tuple[str, ...], float]) -> None:
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0049"),
        "single unbounded component should not fire E0049"
    );
    Ok(())
}

#[test]
fn e0049_no_unbounded_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(x: tuple[int, str, float]) -> None:
    pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0049"),
        "no unbounded component should not fire E0049"
    );
    Ok(())
}
