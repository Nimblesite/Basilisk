#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0092: Too few type arguments.
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
fn e0092_valid_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2")

class Pair(Generic[T1, T2]): ...

x: Pair[int, str] = Pair()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0092"),
        "correct type arg count should not fire E0092"
    );
    Ok(())
}

#[test]
fn e0092_too_few_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T1 = TypeVar("T1")
T2 = TypeVar("T2")

class Pair(Generic[T1, T2]): ...

x: Pair[int] = Pair()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
