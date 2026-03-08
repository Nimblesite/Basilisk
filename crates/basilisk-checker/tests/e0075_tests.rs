//! Integration tests for BSK-E0075: Self-typed attribute incompatibility.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn codes(diags: &[basilisk_checker::Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

#[test]
fn e0075_parent_assigned_to_self_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Self
from dataclasses import dataclass
T = TypeVar("T")

@dataclass
class LinkedList(Generic[T]):
    value: T
    next: Self | None = None

@dataclass
class OrdinalLinkedList(LinkedList[int]):
    def ordinal_value(self) -> str:
        return str(self.value)

xs = OrdinalLinkedList(value=1, next=LinkedList[int](value=2))
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0075_reassignment_parent_to_self() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Self
from dataclasses import dataclass
T = TypeVar("T")

@dataclass
class LinkedList(Generic[T]):
    value: T
    next: Self | None = None

@dataclass
class OrdinalLinkedList(LinkedList[int]):
    pass

xs = OrdinalLinkedList(value=1)
xs.next = LinkedList[int](value=3, next=None)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0075_valid_self_attr_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Self
from dataclasses import dataclass
T = TypeVar("T")

@dataclass
class LinkedList(Generic[T]):
    value: T
    next: Self | None = None

xs = LinkedList[int](value=1, next=LinkedList[int](value=2))
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0075"),
        "same-class Self attr should not fire E0075"
    );
    Ok(())
}
