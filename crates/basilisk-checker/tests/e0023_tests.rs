#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0023: Non-exhaustive match statement.
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
fn e0023_match_without_wildcard_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def check_val(x: int) -> str:
    match x:
        case 1:
            return "one"
        case 2:
            return "two"
    return ""
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0023"),
        "match without wildcard should fire E0023, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0023_match_with_wildcard_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def check_val(x: int) -> str:
    match x:
        case 1:
            return "one"
        case _:
            return "other"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0023"),
        "match with wildcard should not fire E0023"
    );
    Ok(())
}
