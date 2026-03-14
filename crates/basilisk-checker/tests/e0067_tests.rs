#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0067: Enum non-member literal.
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
fn e0067_enum_non_member_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum, nonmember
class Color(Enum):
    RED = 1
    description = nonmember('A color')
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0067_valid_enum_members() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum
class Color(Enum):
    RED = 1
    GREEN = 2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0067"),
        "valid enum members should not fire E0067"
    );
    Ok(())
}
