//! Integration tests for BSK-E0019: Unbound variable on some code paths.
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
fn e0019_conditionally_assigned_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def maybe_assign(flag: bool) -> int:
    if flag:
        result = 42
    return result
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0019"),
        "conditionally assigned variable should fire E0019, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0019_unconditionally_assigned_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def always_assign() -> int:
    result = 42
    return result
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0019"),
        "unconditionally assigned variable should not fire E0019"
    );
    Ok(())
}

#[test]
fn e0019_parameter_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def identity(x: int) -> int:\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0019"),
        "parameter should not fire E0019"
    );
    Ok(())
}
