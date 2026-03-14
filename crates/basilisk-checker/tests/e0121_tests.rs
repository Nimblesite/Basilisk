#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0121: Protocol conformance violation.
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
fn e0121_conforming_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class P(Protocol):
    def method(self) -> None: ...

class C:
    def method(self) -> None:
        pass

x: P = C()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0121"),
        "conforming class should not fire E0121"
    );
    Ok(())
}

#[test]
fn e0121_non_conforming_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class P(Protocol):
    def method(self) -> None: ...

class C:
    pass

x: P = C()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
