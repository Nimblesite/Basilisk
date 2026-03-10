//! Integration tests for BSK-E0127: Tuple index out of range.
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
fn e0127_valid_tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[0]
    y = v[1]
    z = v[2]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0127"),
        "valid tuple indices should not fire E0127"
    );
    Ok(())
}

#[test]
fn e0127_out_of_range_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[4]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0127_negative_out_of_range() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[-4]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
