//! Integration tests for BSK-E0114: Protocol isinstance.
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
fn e0114_protocol_isinstance_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasLen(Protocol):
    def __len__(self) -> int: ...

x: object = []
isinstance(x, HasLen)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
