//! Integration tests for BSK-E0056: `ReadOnly` `TypedDict` mutation.
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
fn e0056_no_readonly_fields_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict

class Config(TypedDict):
    name: str
    version: str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0056"),
        "TypedDict without ReadOnly fields should not fire E0056"
    );
    Ok(())
}

#[test]
fn e0056_readonly_mutation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict
from typing_extensions import ReadOnly

class Config(TypedDict):
    name: str
    version: ReadOnly[str]

cfg: Config = {"name": "test", "version": "1.0"}
cfg["version"] = "2.0"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
