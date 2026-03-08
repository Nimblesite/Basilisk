//! Integration tests for BSK-E0100: Literal augmented assignment.
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
fn e0100_normal_augmented_assignment_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f(x: int) -> None:
    x += 1
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0100"),
        "normal augmented assignment should not fire E0100"
    );
    Ok(())
}
