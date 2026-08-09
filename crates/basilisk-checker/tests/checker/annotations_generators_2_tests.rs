//! PEP 484 tests for generator yield types and iterator element types.
//! <https://peps.python.org/pep-0484/#annotating-generator-functions-and-coroutines>

use super::common::*;

const RULE: &str = "annotations_generators_2";

fn assert_rejected(source: &str, mutation: &str) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(source)?;
    assert_eq!(
        diagnostics.len(),
        1,
        "{mutation}: one incompatible yielded type must produce one isolated diagnostic: {diagnostics:#?}"
    );
    assert_eq!(
        codes(&diagnostics),
        vec![RULE],
        "{mutation}: the PEP 484 generator rule itself must reject the yield"
    );
    assert_eq!(
        messages_for(&diagnostics, RULE).len(),
        1,
        "{mutation}: an unrelated diagnostic cannot satisfy the generator obligation"
    );
    Ok(())
}

fn assert_accepted(source: &str, mutation: &str) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(source)?;
    assert!(
        diagnostics.is_empty(),
        "{mutation}: compatible generator behavior must be accepted: {diagnostics:#?}"
    );
    assert_eq!(
        codes(&diagnostics),
        Vec::<&str>::new(),
        "{mutation}: symbol spelling must not invent a diagnostic"
    );
    assert_rule_count(
        &diagnostics,
        RULE,
        0,
        "PEP 484 accepted generator behavior",
    );
    Ok(())
}

#[test]
fn pep_484_generator_yield_must_match_first_type_argument(
) -> Result<(), Box<dyn std::error::Error>> {
    let rejected = [
        (
            "canonical Generator",
            r#"
from typing import Generator
class Quartz: ...
class Ember: ...
def excavate() -> Generator[Quartz, None, None]:
    yield Ember()
"#,
        ),
        (
            "aliased Generator and renamed classes",
            r#"
from typing import Generator as Stream
class Cipher: ...
class Lantern: ...
def decode() -> Stream[Cipher, None, None]:
    yield Lantern()
"#,
        ),
        (
            "qualified Generator",
            r#"
import typing as contracts
class Vessel: ...
class Anchor: ...
def navigate() -> contracts.Generator[Vessel, None, None]:
    yield Anchor()
"#,
        ),
        (
            "reformatted Generator",
            r#"
from typing import Generator as Stream
class Measurement: ...
class Noise: ...
def observe(
) -> Stream[
    Measurement,
    None,
    None,
]:
    yield (
        Noise()
    )
"#,
        ),
    ];

    for (mutation, source) in rejected {
        assert_rejected(source, mutation)?;
    }
    Ok(())
}

#[test]
fn pep_484_iterator_yield_must_match_element_type(
) -> Result<(), Box<dyn std::error::Error>> {
    let rejected = [
        (
            "canonical Iterator",
            r#"
from typing import Iterator
class Cedar: ...
class Flint: ...
def gather() -> Iterator[Cedar]:
    yield Flint()
"#,
        ),
        (
            "aliased Iterator",
            r#"
from typing import Iterator as SequenceSource
class Trestle: ...
class Torrent: ...
def gather() -> SequenceSource[Trestle]:
    yield Torrent()
"#,
        ),
        (
            "qualified typing Iterator",
            r#"
import typing as contracts
class Map: ...
class Compass: ...
def navigate() -> contracts.Iterator[Map]:
    yield Compass()
"#,
        ),
        (
            "qualified collections Iterator and whitespace",
            r#"
import collections.abc as interfaces
class Signal: ...
class Static: ...
def tune(
) -> interfaces.Iterator[
    Signal
]:
    yield (
        Static()
    )
"#,
        ),
    ];

    for (mutation, source) in rejected {
        assert_rejected(source, mutation)?;
    }
    Ok(())
}

#[test]
fn pep_484_matching_generator_yield_is_accepted(
) -> Result<(), Box<dyn std::error::Error>> {
    let accepted = [
        (
            "canonical Generator",
            r#"
from typing import Generator
class Quartz: ...
def excavate() -> Generator[Quartz, None, None]:
    yield Quartz()
"#,
        ),
        (
            "aliased Generator",
            r#"
from typing import Generator as Stream
class Cipher: ...
def decode() -> Stream[Cipher, None, None]:
    yield Cipher()
"#,
        ),
        (
            "qualified Generator",
            r#"
import typing as contracts
class Vessel: ...
def navigate() -> contracts.Generator[Vessel, None, None]:
    yield Vessel()
"#,
        ),
        (
            "reformatted Generator",
            r#"
from typing import Generator as Stream
class Measurement: ...
def observe(
) -> Stream[
    Measurement,
    None,
    None,
]:
    yield (
        Measurement()
    )
"#,
        ),
    ];

    for (mutation, source) in accepted {
        assert_accepted(source, mutation)?;
    }
    Ok(())
}

#[test]
fn pep_484_yield_from_element_must_match_outer_yield_type(
) -> Result<(), Box<dyn std::error::Error>> {
    let rejected = [
        (
            "canonical imports",
            r#"
from typing import Generator, Iterator
class Quartz: ...
class Ember: ...
def inner() -> Iterator[Ember]:
    yield Ember()
def outer() -> Generator[Quartz, None, None]:
    yield from inner()
"#,
        ),
        (
            "aliased imports",
            r#"
from typing import Generator as Stream, Iterator as Source
class Cipher: ...
class Lantern: ...
def inner() -> Source[Lantern]:
    yield Lantern()
def outer() -> Stream[Cipher, None, None]:
    yield from inner()
"#,
        ),
        (
            "qualified imports",
            r#"
import typing as contracts
class Vessel: ...
class Anchor: ...
def inner() -> contracts.Iterator[Anchor]:
    yield Anchor()
def outer() -> contracts.Generator[Vessel, None, None]:
    yield from inner()
"#,
        ),
        (
            "reformatted yield from",
            r#"
from typing import Generator as Stream, Iterator as Source
class Measurement: ...
class Noise: ...
def inner(
) -> Source[
    Noise
]:
    yield Noise()
def outer(
) -> Stream[
    Measurement,
    None,
    None,
]:
    yield from (
        inner()
    )
"#,
        ),
    ];

    for (mutation, source) in rejected {
        assert_rejected(source, mutation)?;
    }
    Ok(())
}

#[test]
fn pep_484_bare_yield_matches_iterator_none(
) -> Result<(), Box<dyn std::error::Error>> {
    // Regression for issue #108: a bare `yield` yields `None`; a following
    // expression statement must not be mistaken for the yielded expression.
    let accepted = [
        (
            "canonical collections Iterator",
            r#"
from collections.abc import Iterator
def purge() -> None: ...
def restore() -> Iterator[None]:
    purge()
    yield
    purge()
"#,
        ),
        (
            "aliased Iterator",
            r#"
from collections.abc import Iterator as Source
def sweep() -> None: ...
def rebuild() -> Source[None]:
    sweep()
    yield
    sweep()
"#,
        ),
        (
            "qualified Iterator",
            r#"
import collections.abc as interfaces
def rotate() -> None: ...
def reset() -> interfaces.Iterator[None]:
    rotate()
    yield
    rotate()
"#,
        ),
        (
            "reformatted bare yield",
            r#"
from collections.abc import Iterator as Source
def recalibrate(
) -> None: ...
def recover(
) -> Source[
    None
]:
    recalibrate()
    yield
    recalibrate()
"#,
        ),
    ];

    for (mutation, source) in accepted {
        assert_accepted(source, mutation)?;
    }
    Ok(())
}
