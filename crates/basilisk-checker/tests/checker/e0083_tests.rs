//! Tests for [BSK-E0083] from [CHKARCH-DIAG-UNUSED]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-UNUSED
// Integration tests for BSK-E0083: `TypeVarTuple` unpack required.

use super::common::*;

#[test]
fn e0083_unpack_required_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple
Ts = TypeVarTuple("Ts")

def func(*args: *Ts) -> tuple[*Ts]:
    return args
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
