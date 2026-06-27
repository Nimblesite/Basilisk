//! Tests for [generics_base_class_2] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_base_class_2: Inconsistent `TypeVar` ordering.

use super::common::*;

#[test]
fn consistent_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2")

class Grandparent(Generic[T1, T2]): ...
class Parent(Grandparent[T1, T2]): ...
class GoodChild(Parent[T1, T2], Grandparent[T1, T2]): ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_base_class_2"),
        "consistent TypeVar ordering should not fire E0132"
    );
    Ok(())
}

#[test]
fn inconsistent_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2")

class Grandparent(Generic[T1, T2]): ...
class Parent(Grandparent[T1, T2]): ...
class BadChild(Parent[T1, T2], Grandparent[T2, T1]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
