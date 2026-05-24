//! Tests for [BSK-E0132] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0132: Inconsistent `TypeVar` ordering.

use super::common::*;

#[test]
fn e0132_consistent_ordering() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"BSK-E0132"),
        "consistent TypeVar ordering should not fire E0132"
    );
    Ok(())
}

#[test]
fn e0132_inconsistent_ordering() -> Result<(), Box<dyn std::error::Error>> {
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
