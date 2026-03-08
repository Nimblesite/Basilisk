//! Integration tests for BSK-E0021: Overlapping @overload signatures.
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
fn e0021_identical_unannotated_overloads_fires() -> Result<(), Box<dyn std::error::Error>> {
    // Overlap detection requires at least one side to have unannotated params
    let source = r"
from typing import overload

@overload
def process(x) -> int: ...

@overload
def process(x) -> str: ...

def process(x: int) -> int:
    return x
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0021"),
        "identical unannotated overloads should fire E0021, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0021_distinct_overloads_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...

@overload
def process(x: str) -> str: ...

def process(x: int | str) -> int | str:
    return x
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0021"),
        "distinct overloads should not fire E0021"
    );
    Ok(())
}
