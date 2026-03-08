//! Integration tests for BSK-E0118: super() abstract no impl.
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
fn e0118_super_abstract_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from abc import ABC, abstractmethod

class Base(ABC):
    @abstractmethod
    def method(self) -> None: ...

class Child(Base):
    def method(self) -> None:
        super().method()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
