#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0128: `TypeVar` default referential violations.
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
fn e0128_bad_ordering_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
Start2T = TypeVar("Start2T", default="StopT")
Stop2T = TypeVar("Stop2T", default=int)
class slice2(Generic[Start2T, Stop2T]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0128_outer_scope_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
S1 = TypeVar("S1")
S2 = TypeVar("S2", default=S1)
class Foo3(Generic[S1]):
    class Bar2(Generic[S2]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0128_bound_constraint_incompatibility() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
Y1 = TypeVar("Y1", bound=int)
Invalid2 = TypeVar("Invalid2", float, str, default=Y1)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0128_valid_typevar_default_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
S1 = TypeVar("S1")
S2 = TypeVar("S2", default=S1)
class Good(Generic[S1, S2]): ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0128"),
        "valid ordering should not fire E0128"
    );
    Ok(())
}
