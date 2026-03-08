//! Integration tests for BSK-E0020: Missing @overload implementation.
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
fn e0020_all_overloads_no_impl_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def process(x: int) -> int: ...

@overload
def process(x: str) -> str: ...
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0020"),
        "overloads without implementation should fire E0020, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0020_overloads_with_impl_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def process(x: int) -> int: ...

@overload
def process(x: str) -> str: ...

def process(x: int | str) -> int | str:
    return x
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0020"),
        "overloads with implementation should not fire E0020"
    );
    Ok(())
}

#[test]
fn e0020_single_function_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def process(x: int) -> int:\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0020"),
        "single function should not fire E0020"
    );
    Ok(())
}
