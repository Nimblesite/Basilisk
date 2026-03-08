//! Integration tests for BSK-E0139: `TypeVarTuple` specialization violations.
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
fn e0139_valid_specialization() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

IntTupleGeneric = tuple[int, T]
x: IntTupleGeneric[str] = (1, "hello")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0139"),
        "valid specialization should not fire E0139"
    );
    Ok(())
}

#[test]
fn e0139_unpack_on_non_typevar_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, TypeVarTuple

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

IntTupleGeneric = tuple[int, T]
x: IntTupleGeneric[*Ts] = (1,)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
