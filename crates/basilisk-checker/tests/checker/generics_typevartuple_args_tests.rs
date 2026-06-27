//! Tests for [generics_typevartuple_args] from [CHKARCH-DIAG-UNUSED]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-UNUSED
// Integration tests for generics_typevartuple_args: `TypeVarTuple` arg count.

use super::common::*;

#[test]
fn arg_count_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn shared_typevartuple_vararg_length_mismatch() -> Result<(), Box<dyn std::error::Error>> {
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
        has_code(&diags, "generics_typevartuple_args"),
        "differing tuple lengths must fire E0085"
    );
    Ok(())
}

#[test]
fn shared_typevartuple_vararg_equal_lengths_ok() -> Result<(), Box<dyn std::error::Error>> {
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
        !has_code(&diags, "generics_typevartuple_args"),
        "equal tuple lengths must not fire E0085"
    );
    Ok(())
}

#[test]
fn typevartuple_element_order_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    // Right arity, but the constructor reorders the declared dimensions.
    let source = r#"
from typing import Generic, NewType, TypeVarTuple
Shape = TypeVarTuple("Shape")

class Array(Generic[*Shape]):
    def __init__(self, shape: tuple[*Shape]):
        self._shape: tuple[*Shape] = shape

Height = NewType("Height", int)
Width = NewType("Width", int)

v: Array[Height, Width] = Array((Width(1), Height(2)))
"#;
    let diags = run(source)?;
    assert!(
        has_code(&diags, "generics_typevartuple_args"),
        "a permuted constructor must fire E0085, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn typevartuple_element_order_correct_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, NewType, TypeVarTuple
Shape = TypeVarTuple("Shape")

class Array(Generic[*Shape]):
    def __init__(self, shape: tuple[*Shape]):
        self._shape: tuple[*Shape] = shape

Height = NewType("Height", int)
Width = NewType("Width", int)

v: Array[Height, Width] = Array((Height(1), Width(2)))
"#;
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "generics_typevartuple_args"),
        "correct element order must not fire E0085, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
