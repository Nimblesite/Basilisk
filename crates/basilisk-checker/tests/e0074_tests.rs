#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0074: `Constructor __new__ mismatch`.
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
fn e0074_specialized_generic_arg_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Self
T = TypeVar("T")
class Class1(Generic[T]):
    def __new__(cls, x: T) -> Self:
        return super().__new__(cls)

Class1[int](1.0)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0074_valid_specialized_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Self
T = TypeVar("T")
class Class1(Generic[T]):
    def __new__(cls, x: T) -> Self:
        return super().__new__(cls)

Class1[int](42)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0074"),
        "valid specialized call should not fire E0074"
    );
    Ok(())
}

#[test]
fn e0074_cls_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Self
T = TypeVar("T")
class Class11(Generic[T]):
    def __new__(cls: "type[Class11[int]]", x: T) -> Self:
        return super().__new__(cls)

Class11[str]()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
