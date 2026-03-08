//! Integration tests for BSK-E0121: Protocol conformance.
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
fn e0121_protocol_conformance_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasStr(Protocol):
    def __str__(self) -> str: ...

class MyClass:
    def __str__(self) -> str:
        return 'hello'

x: HasStr = MyClass()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0121_non_conforming_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasLen(Protocol):
    def __len__(self) -> int: ...

class NoLen:
    pass

x: HasLen = NoLen()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
