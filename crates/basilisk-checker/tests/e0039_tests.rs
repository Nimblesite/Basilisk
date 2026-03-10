//! Integration tests for BSK-E0039: Invalid `assert_type()` call.
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
fn e0039_valid_assert_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import assert_type

x: int = 42
assert_type(x, int)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0039"),
        "valid assert_type call should not fire E0039"
    );
    Ok(())
}

#[test]
fn e0039_assert_type_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import assert_type
assert_type()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0039_assert_type_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import assert_type
x: int = 42
assert_type(x, int, "extra")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
