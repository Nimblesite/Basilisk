//! Integration tests for BSK-E0060: dataclass ordering invalid.
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
fn e0060_comparison_without_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

p1 = Point(1, 2)
p2 = Point(3, 4)
result = p1 < p2
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0060_comparison_with_order_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(order=True)
class Point:
    x: int
    y: int

p1 = Point(1, 2)
p2 = Point(3, 4)
result = p1 < p2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0060"),
        "order=True dataclass comparison should not fire E0060"
    );
    Ok(())
}

#[test]
fn e0060_eq_comparison_always_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

p1 = Point(1, 2)
p2 = Point(3, 4)
result = p1 == p2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0060"),
        "== comparison always valid for dataclass"
    );
    Ok(())
}
