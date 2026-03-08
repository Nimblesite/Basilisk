//! Integration tests for BSK-E0049: Multiple unbounded tuple components.
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
fn e0049_single_unbounded_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Unpack
Ts = TypeVarTuple("Ts")

def f(x: tuple[int, *tuple[str, ...], float]) -> None:
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0049"),
        "single unbounded component should not fire E0049"
    );
    Ok(())
}

#[test]
fn e0049_no_unbounded_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(x: tuple[int, str, float]) -> None:
    pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0049"),
        "no unbounded component should not fire E0049"
    );
    Ok(())
}
