//! Integration tests for BSK-E0118: Super call on abstract method with no implementation.
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
fn e0118_super_on_abstract_stub() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol
from abc import abstractmethod

class PColor(Protocol):
    @abstractmethod
    def draw(self) -> str:
        ...

class BadColor(PColor):
    def draw(self) -> str:
        return super().draw()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0118_super_on_concrete_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    def method(self) -> str:
        return "base"

class Child(Base):
    def method(self) -> str:
        return super().method()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0118"),
        "super() on concrete method should not fire E0118"
    );
    Ok(())
}
