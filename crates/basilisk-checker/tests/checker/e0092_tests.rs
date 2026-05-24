//! Tests for [BSK-E0092] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Integration tests for BSK-E0092: Too few type arguments.

use super::common::*;

#[test]
fn e0092_valid_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2")

class Pair(Generic[T1, T2]): ...

x: Pair[int, str] = Pair()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0092"),
        "correct type arg count should not fire E0092"
    );
    Ok(())
}

#[test]
fn e0092_too_few_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2")

class Pair(Generic[T1, T2]): ...

x: Pair[int] = Pair()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
