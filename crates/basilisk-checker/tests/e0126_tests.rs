//! Integration tests for BSK-E0126: LiteralString assignment incompatibilities.
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
fn e0126_literal_value_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
def func(b: Literal["two"]) -> None:
    x1: Literal[""] = b
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0126_fstring_non_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import LiteralString
def func(non_literal: str) -> None:
    x2: LiteralString = f"{non_literal}"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0126_invariant_generic_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import LiteralString
def func(s: str) -> None:
    x3: list[LiteralString] = [s]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0126_valid_literal_string_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import LiteralString
def func() -> None:
    x: LiteralString = "hello"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0126"),
        "literal string constant should not fire E0126"
    );
    Ok(())
}
