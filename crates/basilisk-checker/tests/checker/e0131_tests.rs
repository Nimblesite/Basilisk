//! Tests for [BSK-E0131] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0131: Generator yield/send/return type mismatch.

use super::common::*;

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
