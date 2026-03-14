#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0090: Invalid tuple syntax.
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
fn e0090_invalid_tuple_syntax_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: tuple[int, ..., str] = (1, 2, 'a')
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0090_valid_tuple_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: tuple[int, str] = (1, 'a')
y: tuple[int, ...] = (1, 2, 3)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0090"),
        "valid tuple syntax should not fire E0090"
    );
    Ok(())
}
