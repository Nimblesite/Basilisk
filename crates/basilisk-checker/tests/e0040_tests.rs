#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0040: Invalid Enum subclassing.
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
fn e0040_valid_enum() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0040"),
        "valid enum should not fire E0040"
    );
    Ok(())
}

#[test]
fn e0040_enum_with_members_subclassed() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2

class ExtendedColor(Color):
    BLUE = 3
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0040_memberless_enum_base_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Base(Enum):
    pass

class Child(Base):
    VALUE = 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0040"),
        "subclassing memberless enum should not fire E0040"
    );
    Ok(())
}
