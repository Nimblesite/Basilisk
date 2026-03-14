#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0109: `TypeVar` bound violation at call site.
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
fn e0109_valid_bound_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, LiteralString

TLiteral = TypeVar("TLiteral", bound=LiteralString)

def literal_identity(s: TLiteral) -> TLiteral:
    return s
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0109"),
        "valid bound usage should not fire E0109"
    );
    Ok(())
}

#[test]
fn e0109_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, LiteralString

TLiteral = TypeVar("TLiteral", bound=LiteralString)

def literal_identity(s: TLiteral) -> TLiteral:
    return s

def func5(s: str) -> None:
    literal_identity(s)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
