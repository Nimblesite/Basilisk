#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0030: Non-default after default `TypeVar`.
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
fn e0030_non_default_after_default_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T", default=int)
U = TypeVar("U")
class Foo(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0030"),
        "non-default TypeVar after default should fire E0030, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0030_all_defaults_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T", default=int)
U = TypeVar("U", default=str)
class Foo(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0030"),
        "all default TypeVars should not fire E0030"
    );
    Ok(())
}
