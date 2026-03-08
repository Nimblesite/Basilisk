//! Integration tests for BSK-E0012: Argument type mismatch at call site.
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
fn e0012_str_literal_for_int_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def add(x: int, y: int) -> int:
    return x + y

result: int = add("hello", "world")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0012"),
        "str literal for int param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0012_correct_arg_types_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def add(x: int, y: int) -> int:
    return x + y

result: int = add(1, 2)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0012"),
        "correct arg types should not fire E0012"
    );
    Ok(())
}

#[test]
fn e0012_int_literal_for_str_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str) -> str:
    return name

result: str = greet(42)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0012"),
        "int literal for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
