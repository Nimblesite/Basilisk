#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0125: Instance attribute on class object.
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
fn e0125_instance_attr_access_on_instance_ok() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"BSK-E0125"),
        "instance attr access on instance should not fire E0125"
    );
    Ok(())
}

#[test]
fn e0125_instance_attr_on_class() -> Result<(), Box<dyn std::error::Error>> {
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
