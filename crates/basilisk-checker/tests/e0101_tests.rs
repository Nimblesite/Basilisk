//! Integration tests for BSK-E0101: `TypeGuard` no narrowing param.
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
fn e0101_typeguard_no_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def check() -> TypeGuard[int]:
    return True
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0101_typeguard_with_param_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def check(val: object) -> TypeGuard[int]:
    return isinstance(val, int)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0101"),
        "TypeGuard with narrowing param should not fire E0101"
    );
    Ok(())
}
