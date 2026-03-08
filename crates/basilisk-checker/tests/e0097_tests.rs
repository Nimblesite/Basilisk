//! Integration tests for BSK-E0097: Protocol self attr.
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
fn e0097_protocol_self_attr_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, Self

class Builder(Protocol):
    def build(self) -> Self: ...
    name: str
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
