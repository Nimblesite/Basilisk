#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0066: Enum value type mismatch.
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
fn e0066_enum_value_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import IntEnum
class Color(IntEnum):
    RED = 'not_an_int'
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0066_valid_int_enum() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import IntEnum
class Color(IntEnum):
    RED = 1
    GREEN = 2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0066"),
        "valid IntEnum values should not fire E0066"
    );
    Ok(())
}
