//! Tests for [BSK-E0082] from [CHKARCH-DIAG-UNUSED]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-UNUSED
// Integration tests for BSK-E0082: `TypeVarTuple` callable/tuple mismatch.

use super::common::*;

#[test]
fn e0082_tuple_arg_order_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Callable
Ts = TypeVarTuple("Ts")

class Process:
    def __init__(self, target: Callable[[*Ts], None], args: tuple[*Ts]) -> None: ...

def func1(arg1: int, arg2: str) -> None: ...

Process(target=func1, args=("", 0))
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0082_valid_tuple_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Callable
Ts = TypeVarTuple("Ts")

class Process:
    def __init__(self, target: Callable[[*Ts], None], args: tuple[*Ts]) -> None: ...

def func1(arg1: int, arg2: str) -> None: ...

Process(target=func1, args=(0, ""))
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0082"),
        "correct tuple arg order should not fire E0082"
    );
    Ok(())
}
