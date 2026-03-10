//! Integration tests for BSK-E0145: Invalid type[X] usage violations.
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
fn e0145_valid_type_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

class A: ...

def func(x: type[A]) -> None:
    pass

func(A)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0145"),
        "valid type usage should not fire E0145"
    );
    Ok(())
}

#[test]
fn e0145_callable_as_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, TypeVar

T = TypeVar("T")

def func5(x: type[T]) -> None:
    pass

func5(Callable)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0145_unknown_attr_on_type_object() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func8(a: type[object]) -> None:
    a.unknown
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
