//! Integration tests for BSK-E0099: Protocol instantiation.
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
fn e0099_direct_protocol_instantiation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class MyProto(Protocol):
    def method(self) -> int: ...

obj = MyProto()
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0099"),
        "direct Protocol instantiation should fire E0099, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0099_non_protocol_class_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class MyClass:
    def method(self) -> int:
        return 42

obj = MyClass()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0099"),
        "non-Protocol instantiation should not fire E0099"
    );
    Ok(())
}
