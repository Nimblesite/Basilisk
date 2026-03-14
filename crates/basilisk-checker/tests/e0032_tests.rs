#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0032: Invalid `TypedDict` keyword.
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
fn e0032_invalid_keyword_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Movie(TypedDict, metaclass=type):
    name: str
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0032"),
        "invalid keyword in TypedDict should fire E0032, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0032_total_keyword_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Movie(TypedDict, total=False):
    name: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0032"),
        "total keyword should not fire E0032"
    );
    Ok(())
}
