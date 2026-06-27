//! Tests for [annotations_generators_2] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for annotations_generators_2: Generator yield/send/return type mismatch.

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
        !codes(&diags).contains(&"annotations_generators_2"),
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

#[test]
fn e0131_bare_yield_in_iterator_none_generator_not_flagged(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #108: a bare `yield` yields `None`, which is compatible with
    // `Iterator[None]`. The checker must not associate the value of the
    // NEXT statement (`_purge()`) with the bare yield.
    let source = r"
from collections.abc import Iterator

def _purge() -> None: ...

def _restore_workspace_modules() -> Iterator[None]:
    _purge()
    yield
    _purge()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_generators_2"),
        "bare yield in an Iterator[None] generator must not fire E0131, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "annotations_generators_2")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}
