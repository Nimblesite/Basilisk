//! Integration tests for BSK-E0119: Protocol isinstance/issubclass violations.
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
fn e0119_isinstance_non_runtime_checkable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Proto1(Protocol):
    name: str

x = object()
isinstance(x, Proto1)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0119_issubclass_data_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Proto2(Protocol):
    name: str
    def method(self) -> int: ...

class X: ...
issubclass(X, Proto2)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0119_isinstance_runtime_checkable_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Proto3(Protocol):
    def method(self) -> int: ...

x = object()
isinstance(x, Proto3)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0119"),
        "runtime_checkable non-data protocol isinstance should not fire E0119"
    );
    Ok(())
}
