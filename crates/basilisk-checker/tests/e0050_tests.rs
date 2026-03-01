#![doc = "Tests for BSK-E0050: Invalid type argument count for generic type."]
//! Tests for BSK-E0050: Invalid type argument count for generic type.
//!
//! This rule detects when a generic type is subscripted with the wrong number
//! of type arguments. For example:
//! - `List[int, str]` (should be `List[int]`)
//! - `Dict[str]` (should be `Dict[str, int]`)

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run_e2e(src: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

#[test]
fn test_e0050_list_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: list[int, str]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(!e0050.is_empty(), "list with too many args should fire E0050");
    Ok(())
}

#[test]
fn test_e0050_dict_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: dict[str]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(!e0050.is_empty(), "dict with too few args should fire E0050");
    Ok(())
}

#[test]
fn test_e0050_dict_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: dict[str, int, float]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(!e0050.is_empty(), "dict with too many args should fire E0050");
    Ok(())
}

#[test]
fn test_e0050_tuple_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: tuple[int, str, float]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(e0050.is_empty(), "tuple with correct args should not fire E0050");
    Ok(())
}

#[test]
fn test_e0050_set_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: set[int]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(e0050.is_empty(), "set with correct args should not fire E0050");
    Ok(())
}

#[test]
fn test_e0050_set_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: set[int, str]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(!e0050.is_empty(), "set with too many args should fire E0050");
    Ok(())
}

#[test]
fn test_e0050_optional_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: optional[int]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(e0050.is_empty(), "optional with correct args should not fire E0050");
    Ok(())
}

#[test]
fn test_e0050_optional_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: optional[int, str]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(!e0050.is_empty(), "optional with too many args should fire E0050");
    Ok(())
}

#[test]
fn test_e0050_union_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: union[int, str, float]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(e0050.is_empty(), "union with correct args should not fire E0050");
    Ok(())
}

#[test]
fn test_e0050_callable_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: callable[[int, str], bool]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(e0050.is_empty(), "callable with correct args should not fire E0050");
    Ok(())
}

#[test]
fn test_e0050_callable_malformed_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
x: callable[int, str, bool]
"#;
    let diags = run_e2e(src)?;
    let e0050: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0050")
        .collect();
    assert!(!e0050.is_empty(), "callable with malformed args should fire E0050");
    Ok(())
}