#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0106: Protocol class used where type[Proto] expected.
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
fn e0106_concrete_class_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Proto(Protocol):
    def meth(self) -> int: ...

class Concrete:
    def meth(self) -> int:
        return 42

def fun(cls: type[Proto]) -> int:
    return cls().meth()

fun(Concrete)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0106"),
        "passing concrete class should not fire E0106"
    );
    Ok(())
}

#[test]
fn e0106_protocol_class_itself() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Proto(Protocol):
    def meth(self) -> int: ...

def fun(cls: type[Proto]) -> int:
    return cls().meth()

fun(Proto)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
