//! Integration tests for BSK-E0116: NamedTuple definition.
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
fn e0116_namedtuple_definition_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int
    z: str = 'zero'

p = Point(1, 2)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0116_functional_namedtuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

Point = NamedTuple("Point", [("x", int), ("y", int)])
p = Point(1, 2)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
