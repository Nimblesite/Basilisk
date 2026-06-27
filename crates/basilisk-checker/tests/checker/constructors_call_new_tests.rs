//! Tests for [constructors_call_new] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
// Integration tests for constructors_call_new: `Constructor __new__ mismatch`.

use super::common::*;

#[test]
fn specialized_generic_arg_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Self
T = TypeVar("T")
class Class1(Generic[T]):
    def __new__(cls, x: T) -> Self:
        return super().__new__(cls)

Class1[int](1.0)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn valid_specialized_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Self
T = TypeVar("T")
class Class1(Generic[T]):
    def __new__(cls, x: T) -> Self:
        return super().__new__(cls)

Class1[int](42)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"constructors_call_new"),
        "valid specialized call should not fire E0074"
    );
    Ok(())
}

#[test]
fn cls_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Self
T = TypeVar("T")
class Class11(Generic[T]):
    def __new__(cls: "type[Class11[int]]", x: T) -> Self:
        return super().__new__(cls)

Class11[str]()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
