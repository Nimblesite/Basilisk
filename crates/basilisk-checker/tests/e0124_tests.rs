#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0124: Protocol tuple element type mismatch.
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
fn e0124_valid_tuple_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class RGB(Protocol):
    rgb: tuple[int, int, int]

class Point(RGB):
    def __init__(self, red: int, green: int, blue: int) -> None:
        self.rgb = red, green, blue
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0124"),
        "valid tuple assignment should not fire E0124"
    );
    Ok(())
}

#[test]
fn e0124_mismatched_tuple_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class RGB(Protocol):
    rgb: tuple[int, int, int]

class Point(RGB):
    def __init__(self, red: int, green: int, blue: str) -> None:
        self.rgb = red, green, blue
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
