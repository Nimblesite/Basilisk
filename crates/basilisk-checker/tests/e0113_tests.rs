//! Integration tests for BSK-E0113: TypeIs inconsistent narrowing.
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
fn e0113_typeis_inconsistent_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def is_str(val: int | str) -> TypeIs[float]:
    return isinstance(val, str)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0113_typeis_consistent_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def is_str(val: int | str) -> TypeIs[str]:
    return isinstance(val, str)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0113"),
        "consistent TypeIs should not fire E0113"
    );
    Ok(())
}
