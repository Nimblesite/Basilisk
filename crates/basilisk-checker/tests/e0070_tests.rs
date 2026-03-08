//! Integration tests for BSK-E0070: Never type compatibility.
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
fn e0070_never_type_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never

def never_returns() -> Never:
    raise RuntimeError('never')

x: int = never_returns()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0070_never_as_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never

def impossible(x: Never) -> None:
    pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
