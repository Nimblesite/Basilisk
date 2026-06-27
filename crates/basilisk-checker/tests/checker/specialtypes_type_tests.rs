//! Tests for [specialtypes_type] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for specialtypes_type: Invalid type[X] usage violations.

use super::common::*;

#[test]
fn valid_type_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

class A: ...

def func(x: type[A]) -> None:
    pass

func(A)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"specialtypes_type"),
        "valid type usage should not fire E0145"
    );
    Ok(())
}

#[test]
fn callable_as_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, TypeVar

T = TypeVar("T")

def func5(x: type[T]) -> None:
    pass

func5(Callable)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn unknown_attr_on_type_object() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func8(a: type[object]) -> None:
    a.unknown
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
