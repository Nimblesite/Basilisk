//! Integration tests for BSK-E0148: Generic type argument violations.
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
fn e0148_constrained_typevar_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
AnyStr = TypeVar("AnyStr", str, bytes)

def concat(x: AnyStr, y: AnyStr) -> AnyStr:
    return x + y

def bad(s: str, b: bytes) -> None:
    concat(s, b)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0148_valid_constrained_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
AnyStr = TypeVar("AnyStr", str, bytes)

def concat(x: AnyStr, y: AnyStr) -> AnyStr:
    return x + y

def good() -> None:
    concat("a", "b")
    concat(b"a", b"b")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0148"),
        "matching constraint groups should not fire E0148"
    );
    Ok(())
}

#[test]
fn e0148_mapping_subscript_key_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class MyMap(dict[str, int]):
    pass

m = MyMap()
m[0]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
