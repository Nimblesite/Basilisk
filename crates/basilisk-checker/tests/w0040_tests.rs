//! Integration tests for BSK-W0040: Lambda missing type annotations.
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
fn w0040_unannotated_lambda_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "f = lambda x: x + 1\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-W0040"),
        "unannotated lambda should fire W0040, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn w0040_annotated_lambda_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import Callable\nf: Callable[[int], int] = lambda x: x + 1\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-W0040"),
        "annotated lambda should not fire W0040"
    );
    Ok(())
}

#[test]
fn w0040_lambda_is_warning_not_error() -> Result<(), Box<dyn std::error::Error>> {
    let source = "f = lambda x: x + 1\n";
    let diags = run(source)?;
    let w0040 = diags.iter().find(|d| d.code.code == "BSK-W0040");
    assert!(w0040.is_some(), "should fire W0040");
    assert_eq!(
        w0040.expect("asserted").severity,
        basilisk_checker::Severity::Warning,
        "W0040 should be a warning"
    );
    Ok(())
}
