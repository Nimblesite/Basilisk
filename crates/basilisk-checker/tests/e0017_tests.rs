//! Integration tests for BSK-E0017: Incompatible class attribute override.
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
fn e0017_incompatible_attr_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    count: int = 0

class Child(Base):
    count: str = "zero"
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0017"),
        "incompatible attr type should fire E0017, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0017_compatible_attr_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    count: int = 0

class Child(Base):
    count: int = 10
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0017"),
        "compatible attr type should not fire E0017"
    );
    Ok(())
}

#[test]
fn e0017_different_attr_name_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    count: int = 0

class Child(Base):
    name: str = "hello"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0017"),
        "different attr name should not fire E0017"
    );
    Ok(())
}
