//! Tests for [BSK-E0085] from [CHKARCH-DIAG-UNUSED]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-UNUSED
// Integration tests for BSK-E0085: `TypeVarTuple` arg count.

use super::common::*;

#[test]
fn e0085_arg_count_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic
Ts = TypeVarTuple("Ts")

class Tensor(Generic[*Ts]):
    pass

def takes_3d(t: Tensor[int, int, int]) -> None:
    pass

x: Tensor[int, int] = Tensor()
takes_3d(x)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0085_shared_typevartuple_vararg_length_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // `*args: tuple[*Ts]` binds one TypeVarTuple across every argument, so all
    // tuple-literal arguments must share a length.
    let source = r#"
from typing import TypeVarTuple
Ts = TypeVarTuple("Ts")

def func4(*args: tuple[*Ts]):
    ...

func4((0,), (1, 2))
"#;
    let diags = run(source)?;
    assert!(
        has_code(&diags, "BSK-E0085"),
        "differing tuple lengths must fire E0085"
    );
    Ok(())
}

#[test]
fn e0085_shared_typevartuple_vararg_equal_lengths_ok() -> Result<(), Box<dyn std::error::Error>> {
    // Equal lengths conform — element types are joined, never conflicting.
    let source = r#"
from typing import TypeVarTuple
Ts = TypeVarTuple("Ts")

def func4(*args: tuple[*Ts]):
    ...

func4((0,), (1,))
func4((0,), ("1",))
"#;
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "BSK-E0085"),
        "equal tuple lengths must not fire E0085"
    );
    Ok(())
}
