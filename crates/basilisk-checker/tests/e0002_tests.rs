//! Integration tests for BSK-E0002: Missing return type annotation.
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
fn e0002_missing_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def greet(name: str):
    return name
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0002"),
        "function without return annotation should fire E0002, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0002_with_return_annotation_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def greet(name: str) -> str:
    return name
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0002"),
        "function with return annotation should not fire E0002"
    );
    Ok(())
}

#[test]
fn e0002_none_return_annotation_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def do_nothing() -> None:
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0002"),
        "function with -> None should not fire E0002"
    );
    Ok(())
}
