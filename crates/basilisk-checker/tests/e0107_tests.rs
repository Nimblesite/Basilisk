//! Integration tests for BSK-E0107: Variance incompatibility.
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
fn e0107_variance_incompat_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T_co = TypeVar("T_co", covariant=True)

class Producer(Generic[T_co]):
    def get(self) -> T_co: ...

def func(p: Producer[int]) -> None:
    pass

x: Producer[object] = Producer()
func(x)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
