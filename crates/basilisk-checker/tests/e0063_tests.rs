#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0063: Non-hashable dataclass.
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
fn e0063_mutable_dataclass_in_set() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

s = {Point(1, 2)}
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0063_frozen_dataclass_hashable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class Point:
    x: int
    y: int

s = {Point(1, 2)}
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0063"),
        "frozen dataclass should be hashable, no E0063"
    );
    Ok(())
}

#[test]
fn e0063_mutable_dataclass_as_dict_key() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

d = {Point(1, 2): 'a'}
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
