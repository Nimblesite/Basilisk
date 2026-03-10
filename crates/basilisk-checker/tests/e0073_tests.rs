//! Integration tests for BSK-E0073: `NamedTuple` tuple compatibility.
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
fn e0073_namedtuple_tuple_compat_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

t: tuple[str, str] = Point(1, 2)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0073_valid_tuple_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

t: tuple[int, int] = Point(1, 2)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0073"),
        "compatible tuple assignment should not fire E0073"
    );
    Ok(())
}
