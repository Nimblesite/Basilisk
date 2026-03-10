//! Integration tests for BSK-E0091: `TypeVar` default incompatible.
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
fn e0091_typevar_default_incompat_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", bound=int, default=str)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0091_valid_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", bound=int, default=int)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0091"),
        "compatible default should not fire E0091"
    );
    Ok(())
}
