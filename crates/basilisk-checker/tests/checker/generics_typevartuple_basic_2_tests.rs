//! Tests for [generics_typevartuple_basic_2] from [CHKARCH-DIAG-UNUSED]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-UNUSED
// Integration tests for generics_typevartuple_basic_2: `TypeVarTuple` unpack required.

use super::common::*;

#[test]
fn unpack_required_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
