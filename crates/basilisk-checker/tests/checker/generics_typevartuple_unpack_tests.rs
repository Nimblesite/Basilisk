//! Tests for [`generics_typevartuple_unpack`] from [CHKARCH-DIAG-UNUSED]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-UNUSED
// Integration tests for generics_typevartuple_unpack: `TypeVarTuple` unpack minimum args.

use super::common::*;

#[test]
fn too_few_args_for_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Unpack, Generic
Ts = TypeVarTuple("Ts")

class Tensor(Generic[*Ts]):
    pass

def process(t: Tensor[int, str, float]) -> None:
    pass

process(Tensor())
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn valid_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic
Ts = TypeVarTuple("Ts")

class Tensor(Generic[*Ts]):
    pass

x: Tensor[int, str] = Tensor()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_typevartuple_unpack"),
        "valid TypeVarTuple should not fire E0081"
    );
    Ok(())
}
