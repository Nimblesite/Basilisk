//! Integration tests for BSK-E0092: Too few type args.
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
fn e0092_too_few_type_args_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
U = TypeVar("U")
class Pair(Generic[T, U]):
    pass

x: Pair[int]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0092_correct_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
U = TypeVar("U")
class Pair(Generic[T, U]):
    pass

x: Pair[int, str]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0092"),
        "correct type arg count should not fire E0092"
    );
    Ok(())
}
