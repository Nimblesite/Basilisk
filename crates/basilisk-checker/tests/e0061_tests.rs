#![doc = "Tests for BSK-E0061: Implicit bool-to-int coercion."]
//! Tests for BSK-E0061: Implicit bool-to-int coercion.
//!
//! This rule detects when a boolean value is implicitly coerced to an integer
//! without an explicit conversion.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run_e2e(src: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

#[test]
fn test_e0061_bool_to_int_coercion() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: int = True
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(!e0061.is_empty(), "bool-to-int coercion should fire E0061");
    Ok(())
}

#[test]
fn test_e0061_false_to_int_coercion() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: int = False
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(!e0061.is_empty(), "false-to-int coercion should fire E0061");
    Ok(())
}

#[test]
fn test_e0061_explicit_conversion_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: int = int(True)
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(e0061.is_empty(), "explicit conversion should not fire E0061");
    Ok(())
}

#[test]
fn test_e0061_bool_to_float_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: float = True
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(e0061.is_empty(), "bool-to-float should not fire E0061");
    Ok(())
}

#[test]
fn test_e0061_bool_to_str_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: str = True
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(e0061.is_empty(), "bool-to-str should not fire E0061");
    Ok(())
}

#[test]
fn test_e0061_unannotated_bool_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x = True
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(e0061.is_empty(), "unannotated bool should not fire E0061");
    Ok(())
}
