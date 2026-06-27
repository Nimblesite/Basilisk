//! Tests for [generics_typevartuple_basic_3] from [CHKARCH-DIAG-UNUSED]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-UNUSED
// Integration tests for generics_typevartuple_basic_3: `TypeVarTuple` invalid params.

use super::common::*;

#[test]
fn invalid_params_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic
Ts = TypeVarTuple("Ts")

class Tensor(Generic[*Ts]):
    def method(self) -> None:
        pass
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
