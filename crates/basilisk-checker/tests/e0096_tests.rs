//! Integration tests for BSK-E0096: Dataclass field default factory mismatch.
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
fn e0096_valid_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from dataclasses import dataclass, field

@dataclass
class DC:
    items: list[int] = field(default_factory=list)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0096"),
        "valid default_factory should not fire E0096"
    );
    Ok(())
}
