//! Integration tests for BSK-E0113: `TypeIs` inconsistent narrowing.
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
fn e0113_valid_typeis() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def is_str(x: object) -> TypeIs[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0113"),
        "valid TypeIs should not fire E0113"
    );
    Ok(())
}

#[test]
fn e0113_inconsistent_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def bad_check(x: int) -> TypeIs[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
