#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0043: Non-`TypeVar` in Generic[...].
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
fn e0043_concrete_type_in_generic_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generic
class Bad(Generic[int]):
    pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0043"),
        "concrete type in Generic should fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0043_typevar_in_generic_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class Good(Generic[T]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0043"),
        "TypeVar in Generic should not fire E0043"
    );
    Ok(())
}

#[test]
fn e0043_concrete_type_in_protocol_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol
class Bad(Protocol[int]):
    pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0043"),
        "concrete type in Protocol should fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0043_undeclared_typevar_in_base_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Iterable
T = TypeVar("T")
S = TypeVar("S")
class Bad(Iterable[T], Generic[S]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0043"),
        "undeclared TypeVar in base should fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0043_multiple_typevars_in_generic_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
U = TypeVar("U")
class Good(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0043"),
        "Multiple TypeVars in Generic should not fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0043_typevar_in_protocol_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T = TypeVar("T")
class Good(Protocol[T]):
    def method(self, x: T) -> T: ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0043"),
        "TypeVar in Protocol should not fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
