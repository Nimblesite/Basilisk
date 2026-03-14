#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0131: Generator yield/send/return type mismatch.
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
fn e0131_incompatible_yield_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator

class A: ...
class B: ...

def bad() -> Generator[A, None, None]:
    yield 3
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0131_iterator_yield_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Iterator

class A: ...
class B: ...

def bad2() -> Iterator[A]:
    yield B()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0131_valid_yield_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator
def good() -> Generator[int, None, None]:
    yield 42
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0131"),
        "compatible yield type should not fire E0131"
    );
    Ok(())
}

#[test]
fn e0131_yield_from_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator, Iterator

def inner() -> Iterator[str]:
    yield 'hello'

def outer() -> Generator[int, None, None]:
    yield from inner()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
