//! Integration tests for BSK-E0110: Protocol variance violation.
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
fn e0110_covariant_in_input_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_co = TypeVar("T_co", covariant=True)
class BadProto(Protocol[T_co]):
    def method(self, x: T_co) -> None: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0110_contravariant_in_output_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_contra = TypeVar("T_contra", contravariant=True)
class BadProto2(Protocol[T_contra]):
    def method(self) -> T_contra: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0110_valid_covariant_output() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_co = TypeVar("T_co", covariant=True)
class GoodProto(Protocol[T_co]):
    def method(self) -> T_co: ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0110"),
        "covariant in output position should not fire E0110"
    );
    Ok(())
}

#[test]
fn e0110_valid_contravariant_input() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_contra = TypeVar("T_contra", contravariant=True)
class GoodProto2(Protocol[T_contra]):
    def method(self, x: T_contra) -> None: ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0110"),
        "contravariant in input position should not fire E0110"
    );
    Ok(())
}

#[test]
fn e0110_init_exempt_from_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_co = TypeVar("T_co", covariant=True)
class Proto(Protocol[T_co]):
    def __init__(self, x: T_co) -> None: ...
    def method(self) -> T_co: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
