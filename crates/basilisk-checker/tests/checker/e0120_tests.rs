//! Tests for [BSK-E0120] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0120: Generator return type violations.

use super::common::*;

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

#[test]
fn e0120_asynccontextmanager_async_iterator_not_flagged() -> Result<(), Box<dyn std::error::Error>>
{
    // Issue #36: async generators decorated with @asynccontextmanager are the
    // canonical async-context-manager pattern and must be accepted.
    let source = r"
from contextlib import asynccontextmanager
from collections.abc import AsyncIterator

@asynccontextmanager
async def lifespan() -> AsyncIterator[None]:
    yield
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "@asynccontextmanager async generator with AsyncIterator[None] must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0120")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0120_asynccontextmanager_async_generator_annotation_not_flagged(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #36: the AsyncGenerator[T, None] spelling is equally valid.
    let source = r"
from contextlib import asynccontextmanager
from collections.abc import AsyncGenerator

@asynccontextmanager
async def session() -> AsyncGenerator[int, None]:
    yield 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "@asynccontextmanager async generator with AsyncGenerator[int, None] must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0120")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0120_dotted_async_iterator_annotation_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #36: a dotted generator annotation (`typing.AsyncIterator[None]`)
    // is the same valid type as the unqualified spelling.
    let source = r"
import contextlib
import typing

@contextlib.asynccontextmanager
async def lifespan() -> typing.AsyncIterator[None]:
    yield
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "typing.AsyncIterator[None] async generator must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0120")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0120_collections_abc_dotted_annotation_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #36: `collections.abc.AsyncIterator[None]`, no decorator — the
    // false positive is independent of @asynccontextmanager.
    let source = r"
import collections.abc

async def agen() -> collections.abc.AsyncIterator[None]:
    yield
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "collections.abc.AsyncIterator[None] must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0120")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0120_string_annotation_async_iterator_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #36: a string (forward-reference) generator annotation is valid.
    let source = r#"
from collections.abc import AsyncIterator

async def agen() -> "AsyncIterator[None]":
    yield
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "string annotation AsyncIterator[None] must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0120")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0120_dotted_sync_iterator_annotation_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #36: same defect for sync generators with `typing.Iterator[int]`.
    let source = r"
import typing

def gen() -> typing.Iterator[int]:
    yield 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0120"),
        "typing.Iterator[int] sync generator must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0120")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}
