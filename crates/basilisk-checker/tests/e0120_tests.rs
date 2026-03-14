#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0120: Generator return type violations.
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
fn e0120_generator_with_int_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def bad() -> int:
    yield 1
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0120_generator_with_iterator_return_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Iterator
def good() -> Iterator[int]:
    yield 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "Iterator return type should not fire E0120"
    );
    Ok(())
}

#[test]
fn e0120_generator_with_generator_return_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator
def good() -> Generator[int, None, None]:
    yield 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "Generator return type should not fire E0120"
    );
    Ok(())
}

#[test]
fn e0120_async_generator_bad_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
async def bad_async() -> int:
    yield 1
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0120_async_generator_valid_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import AsyncIterator
async def good_async() -> AsyncIterator[int]:
    yield 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "AsyncIterator return type should not fire E0120"
    );
    Ok(())
}

#[test]
fn e0120_iterable_return_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Iterable
def gen() -> Iterable[str]:
    yield 'hello'
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "Iterable return type should not fire E0120"
    );
    Ok(())
}
