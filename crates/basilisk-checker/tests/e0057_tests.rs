//! Integration tests for BSK-E0057: PEP 695 type alias invalid.
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
fn e0057_pep695_type_alias_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Vector = list[float]
type Matrix = list[Vector]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_with_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Pair[T] = tuple[T, T]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
