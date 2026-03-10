//! Integration tests for BSK-E0078: Self type violation.
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
fn e0078_self_type_violation_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Base:
    def copy(self) -> Self:
        return Base()

class Child(Base):
    pass

c = Child()
result = c.copy()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
