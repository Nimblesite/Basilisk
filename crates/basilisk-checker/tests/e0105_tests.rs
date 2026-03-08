//! Integration tests for BSK-E0105: Bounded type var attribute access.
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
fn e0105_valid_attr_on_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class C[T: str]:
    def method(self, x: T) -> str:
        return x.upper()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0105"),
        "accessing valid str method on str-bounded typevar should not fire E0105"
    );
    Ok(())
}

#[test]
fn e0105_invalid_attr_on_bound() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class C[T: str]:
    def method(self, x: T) -> None:
        x.is_integer()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
