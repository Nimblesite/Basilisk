#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0103: Tuple index out of bounds.
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
fn e0103_valid_tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[int, str] = (1, "a")
x = t[0]
y = t[1]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0103"),
        "valid tuple index should not fire E0103"
    );
    Ok(())
}
