//! Integration tests for BSK-E0106: Protocol as type.
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
fn e0106_protocol_as_type_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

def factory(cls: type[Drawable]) -> Drawable:
    return cls()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
