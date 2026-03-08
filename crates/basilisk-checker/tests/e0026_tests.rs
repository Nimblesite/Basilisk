//! Integration tests for BSK-E0026: TypeVar with single constraint.
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
fn e0026_single_constraint_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", int)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0026"),
        "TypeVar with single constraint should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0026_two_constraints_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", int, str)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0026"),
        "TypeVar with two constraints should not fire E0026"
    );
    Ok(())
}

#[test]
fn e0026_unconstrained_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0026"),
        "unconstrained TypeVar should not fire E0026"
    );
    Ok(())
}

#[test]
fn e0026_name_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
MyT = TypeVar("T")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0026"),
        "TypeVar name mismatch should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
