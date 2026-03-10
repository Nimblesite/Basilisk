//! Integration tests for BSK-E0029: Method defined in `TypedDict`.
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
fn e0029_method_in_typeddict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int

    def display(self) -> str:
        return self["name"]
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0029"),
        "method in TypedDict should fire E0029, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0029_typeddict_fields_only_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Movie(TypedDict):
    name: str
    year: int
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0029"),
        "TypedDict with only fields should not fire E0029"
    );
    Ok(())
}
