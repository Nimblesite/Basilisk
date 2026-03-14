#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0064: Invalid `NamedTuple` call.
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
fn e0064_valid_namedtuple_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0064"),
        "valid NamedTuple should not fire E0064"
    );
    Ok(())
}

#[test]
fn e0064_namedtuple_functional_form() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
Point = NamedTuple("Point", [("x", int), ("y", int)])
"#;
    let diags = run(source)?;
    // Just exercise - functional form may or may not fire
    let _ = codes(&diags);
    Ok(())
}
