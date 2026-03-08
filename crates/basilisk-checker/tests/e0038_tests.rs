//! Integration tests for BSK-E0038: Invalid `TypedDict` inheritance.
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
fn e0038_typeddict_conflicting_field_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Base(TypedDict):
    name: str

class Child(Base):
    name: int
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0038"),
        "TypedDict field type conflict should fire E0038, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0038_typeddict_compatible_inheritance_no_diagnostic() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
from typing import TypedDict

class Base(TypedDict):
    name: str

class Child(Base):
    age: int
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0038"),
        "compatible TypedDict inheritance should not fire E0038"
    );
    Ok(())
}
