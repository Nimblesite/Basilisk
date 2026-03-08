//! Integration tests for BSK-E0031: Invalid cast() call.
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
fn e0031_cast_literal_first_arg_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import cast
x: int = 1
y = cast(1, x)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0031"),
        "cast with literal first arg should fire E0031, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0031_cast_too_few_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import cast
y = cast()
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0031"),
        "cast() with no args should fire E0031, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0031_cast_valid_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import cast
x: int = 1
y = cast(str, x)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0031"),
        "valid cast should not fire E0031"
    );
    Ok(())
}
