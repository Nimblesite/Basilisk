//! Integration tests for BSK-E0047: Invalid type expression.
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
fn e0047_invalid_type_expr_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: 1 + 2
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_valid_type_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: int = 42
y: list[str] = []
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0047"),
        "valid type annotations should not fire E0047"
    );
    Ok(())
}

#[test]
fn e0047_string_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: "int" = 42
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_union_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: int | str = 42
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
