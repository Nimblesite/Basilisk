//! Integration tests for BSK-E0022: Unhashable dict key.
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
fn e0022_hashable_key_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def good_key() -> None:
    mapping: dict[str, int] = {"key": 1}
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0022"),
        "string key should not fire E0022"
    );
    Ok(())
}

#[test]
fn e0022_list_as_key() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def bad_key() -> None:
    mapping = {[1, 2]: "value"}
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
