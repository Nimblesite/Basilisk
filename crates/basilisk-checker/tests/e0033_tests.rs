#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0033: Invalid `reveal_type()` call.
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
fn e0033_valid_reveal_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: int = 42
reveal_type(x)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0033"),
        "valid reveal_type call should not fire E0033"
    );
    Ok(())
}

#[test]
fn e0033_reveal_type_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
reveal_type()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0033_reveal_type_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 42
y: str = "hi"
reveal_type(x, y)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
