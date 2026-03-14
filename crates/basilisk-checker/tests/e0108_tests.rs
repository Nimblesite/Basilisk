#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0108: Dataclass slots violations.
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
fn e0108_no_slots_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class DC:
    x: int
    y: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0108"),
        "dataclass without slots=True should not fire E0108"
    );
    Ok(())
}

#[test]
fn e0108_slots_valid_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(slots=True)
class DC:
    x: int
    y: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0108"),
        "valid dataclass with slots=True should not fire E0108"
    );
    Ok(())
}

#[test]
fn e0108_slots_invalid_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(slots=True)
class DC:
    x: int

    def __init__(self) -> None:
        self.y = 3
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0108_slots_access_on_non_slots() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class DC2:
    a: int

DC2.__slots__
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
