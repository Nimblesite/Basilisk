//! Tests for [`generics_type_erasure`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_type_erasure: Instance attribute on class object.

use super::common::*;

#[test]
fn instance_attr_access_on_instance_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Node(Generic[T]):
    label: T

n1: Node[int] = Node()
x = n1.label
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_type_erasure"),
        "instance attr access on instance should not fire E0125"
    );
    Ok(())
}

#[test]
fn instance_attr_on_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Node(Generic[T]):
    label: T

Node.label = 1
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
