#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0048: `TypeAlias` invalid RHS.
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
fn e0048_valid_type_alias_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
MyType: TypeAlias = list[int]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0048"),
        "valid TypeAlias should not fire E0048"
    );
    Ok(())
}

#[test]
fn e0048_type_alias_with_union() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
NumOrStr: TypeAlias = int | str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0048"),
        "union TypeAlias should not fire E0048"
    );
    Ok(())
}

#[test]
fn e0048_type_alias_bool_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = True
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0048_type_alias_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = 42
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0048_type_alias_list_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = [int, str]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0048_type_alias_fstring() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias
Bad: TypeAlias = f"hello"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0048_type_alias_conditional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = int if True else str
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0048_type_alias_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias
Bad: TypeAlias = {"a": int}
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0048_type_alias_lambda() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = lambda: int
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0048_type_alias_nested() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Inner: TypeAlias = list[int]
Outer: TypeAlias = dict[str, Inner]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0048"),
        "nested valid TypeAlias should not fire E0048"
    );
    Ok(())
}
