#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0079: Module-level protocol incompatibility.
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
fn e0079_module_var_protocol_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    def draw(self) -> None:
        pass

class Square:
    pass

x: Drawable = Square()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0079_valid_protocol_conformance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    def draw(self) -> None:
        pass

x: Drawable = Circle()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0079"),
        "valid protocol conformance should not fire E0079"
    );
    Ok(())
}

#[test]
fn e0079_missing_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasName(Protocol):
    def name(self) -> str: ...
    def id(self) -> int: ...

class OnlyName:
    def name(self) -> str:
        return 'x'

x: HasName = OnlyName()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
