#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0123: Super call on abstract protocol method.
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
fn e0123_super_on_protocol_abstract() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0123_super_on_protocol_with_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class PColor(Protocol):
    def draw(self) -> str:
        return "default"

class GoodColor(PColor):
    def draw(self) -> str:
        return super().draw() + " extended"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0123"),
        "super() on protocol with default impl should not fire E0123"
    );
    Ok(())
}
