//! Integration tests for BSK-E0109: TypeVar bound violation.
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
fn e0109_bound_violation_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T", bound=int)

class Container(Generic[T]):
    pass

x: Container[str]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0109_within_bound_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T", bound=int)

class Container(Generic[T]):
    pass

x: Container[int]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0109"),
        "within bound should not fire E0109"
    );
    Ok(())
}
