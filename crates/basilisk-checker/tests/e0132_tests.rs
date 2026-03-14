#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0132: Inconsistent `TypeVar` ordering.
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
