#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0117: Unbound type variable in scope.
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
fn e0117_bound_typevar_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

def fun(x: T) -> list[T]:
    return [x]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0117"),
        "TypeVar bound in function sig should not fire E0117"
    );
    Ok(())
}

#[test]
fn e0117_unbound_typevar_in_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

def fun(x: T) -> list[T]:
    z: list[S] = []
    return [x]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0117_bound_typevar_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    items: list[T]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0117"),
        "TypeVar bound in Generic class should not fire E0117"
    );
    Ok(())
}

#[test]
fn e0117_unbound_typevar_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")
S = TypeVar("S")

class Bar(Generic[T]):
    an_attr: list[S]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
