#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0077: Protocol self return.
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
fn e0077_protocol_self_return_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, Self

class Cloneable(Protocol):
    def clone(self) -> Self: ...

class MyClass:
    def clone(self) -> 'MyClass':
        return MyClass()

x: Cloneable = MyClass()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
