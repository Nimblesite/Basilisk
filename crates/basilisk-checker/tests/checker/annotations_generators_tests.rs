//! Tests for [`annotations_generators`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for annotations_generators: Generator return type violations.

use super::common::*;

#[test]
fn generator_with_int_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def bad() -> int:
    yield 1
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn generator_with_iterator_return_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Iterator
def good() -> Iterator[int]:
    yield 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_generators"),
        "Iterator return type should not fire E0120"
    );
    Ok(())
}

#[test]
fn generator_with_generator_return_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generator
def good() -> Generator[int, None, None]:
    yield 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_generators"),
        "Generator return type should not fire E0120"
    );
    Ok(())
}

#[test]
fn async_generator_bad_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
async def bad_async() -> int:
    yield 1
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn async_generator_valid_return() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import AsyncIterator
async def good_async() -> AsyncIterator[int]:
    yield 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_generators"),
        "AsyncIterator return type should not fire E0120"
    );
    Ok(())
}

#[test]
fn iterable_return_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Iterable
def gen() -> Iterable[str]:
    yield 'hello'
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_generators"),
        "Iterable return type should not fire E0120"
    );
    Ok(())
}

#[test]
fn asynccontextmanager_async_iterator_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"annotations_generators"),
        "@asynccontextmanager async generator with AsyncIterator[None] must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "annotations_generators")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn asynccontextmanager_async_generator_annotation_not_flagged(
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
        !codes(&diags).contains(&"annotations_generators"),
        "@asynccontextmanager async generator with AsyncGenerator[int, None] must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "annotations_generators")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn dotted_async_iterator_annotation_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"annotations_generators"),
        "typing.AsyncIterator[None] async generator must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "annotations_generators")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn collections_abc_dotted_annotation_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #36: `collections.abc.AsyncIterator[None]`, no decorator — the
    // false positive is independent of @asynccontextmanager.
    let source = r"
import collections.abc

async def agen() -> collections.abc.AsyncIterator[None]:
    yield
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_generators"),
        "collections.abc.AsyncIterator[None] must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "annotations_generators")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn string_annotation_async_iterator_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #36: a string (forward-reference) generator annotation is valid.
    let source = r#"
from collections.abc import AsyncIterator

async def agen() -> "AsyncIterator[None]":
    yield
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_generators"),
        "string annotation AsyncIterator[None] must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "annotations_generators")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn dotted_sync_iterator_annotation_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #36: same defect for sync generators with `typing.Iterator[int]`.
    let source = r"
import typing

def gen() -> typing.Iterator[int]:
    yield 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_generators"),
        "typing.Iterator[int] sync generator must not fire E0120, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "annotations_generators")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn yield_call_expressions_do_not_use_callee_name_as_type() -> Result<(), Box<dyn std::error::Error>>
{
    // GitHub #281: `yield a.b(c)` inferred the *method name* (`get`) as the
    // yielded type — "`get` is not assignable to `str`". The same name-as-type
    // shortcut also flagged `yield helper(x)` (type `helper`) and even
    // `yield str(x)` ("`str` is not assignable to `str`", Named vs builtin).
    let source = r#"
from collections.abc import Iterator

NAME_SYNONYMS: dict[str, str] = {"a": "b"}

def get_possible_names(name: str) -> Iterator[str]:
    yield NAME_SYNONYMS.get(name, name)

def helper(x: str) -> str:
    return x

def gen_local_call(name: str) -> Iterator[str]:
    yield helper(name)

def gen_builtin_call(name: str) -> Iterator[str]:
    yield str(name)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"annotations_generators"),
        "yielding call results compatible with the declared yield type must not \
         fire annotations_generators, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "annotations_generators")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn yield_local_call_with_incompatible_return_still_flagged(
) -> Result<(), Box<dyn std::error::Error>> {
    // The fix resolves a direct callee to its declared return type — so a
    // genuinely incompatible local call must still be caught.
    let source = r"
from collections.abc import Iterator

def make_number() -> int:
    return 42

def gen(name: str) -> Iterator[str]:
    yield make_number()
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"annotations_generators"),
        "yielding an int-returning call from an Iterator[str] generator must fire"
    );
    Ok(())
}
