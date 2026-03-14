#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0024: Invalid type form (numeric literal as annotation).
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
fn e0024_numeric_literal_param_annotation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: 42) -> None:\n    pass\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0024"),
        "numeric literal param annotation should fire E0024, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0024_numeric_literal_return_annotation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: int) -> 0:\n    pass\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0024"),
        "numeric literal return annotation should fire E0024, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0024_normal_type_annotation_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: int) -> str:\n    return str(x)\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0024"),
        "normal type annotation should not fire E0024"
    );
    Ok(())
}
