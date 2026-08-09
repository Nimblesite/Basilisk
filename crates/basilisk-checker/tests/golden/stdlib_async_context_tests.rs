//! Awaitable, generator, and context-manager protocol obligations, plus the
//! obligation that a value-returning function actually returns one.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! Each of these is a protocol lookup on the *resolved type* of an operand:
//! `await x` needs `type(x).__await__`, `yield from x` needs `type(x).__iter__`,
//! `with x` needs `__enter__`/`__exit__`, `async with x` needs
//! `__aenter__`/`__aexit__`. None of them can be decided from how the operand is
//! spelled, so every case is also given aliased and attribute-access forms.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── `await` requires `__await__` ─────────────────────────────────────────

#[test]
fn await_requires_an_awaitable() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`await x` desugars to `type(x).__await__(x)`; `int` declares no `__await__`",
        rejected: r#"
async def cascade() -> None:
    await 1
"#,
        accepted: r#"
async def upstream() -> int:
    return 1


async def cascade() -> None:
    await upstream()
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole


async def cascade() -> None:
    sluice: Whole = 1
    await sluice
"#,
            ),
            import_form(
                r#"
import builtins


async def cascade() -> None:
    sluice: builtins.int = 1
    await sluice
"#,
            ),
            renamed(
                r#"
async def spillway() -> None:
    await 1
"#,
            ),
            reformatted(
                "
async def cascade() -> None:

        await (
            1   # <- nothing to suspend on
        )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole


async def upstream() -> Whole:
    return 1


async def cascade() -> None:
    await upstream()
"#,
            ),
            renamed(
                r#"
async def headwater() -> int:
    return 1


async def spillway() -> None:
    await headwater()
"#,
            ),
        ],
    }
    .assert("await requires an awaitable")
}

// ── `yield from` requires an iterable ────────────────────────────────────

#[test]
fn yield_from_requires_an_iterable() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`yield from x` iterates `x`; `int` declares neither `__iter__` nor \
                      `__getitem__`",
        rejected: r#"
import typing


def cadence() -> typing.Generator[None, None, None]:
    yield from 42
"#,
        accepted: r#"
import typing


def cadence() -> typing.Generator[None, None, None]:
    yield from (None, None)
"#,
        rejected_variants: &[
            aliased(
                r#"
from collections.abc import Generator as Stream


def cadence() -> Stream[None, None, None]:
    yield from 42
"#,
            ),
            import_form(
                r#"
import collections.abc


def cadence() -> collections.abc.Generator[None, None, None]:
    yield from 42
"#,
            ),
            renamed(
                r#"
import typing


def metre() -> typing.Generator[None, None, None]:
    yield from 42
"#,
            ),
            reformatted(
                "
import typing

def cadence() -> typing.Generator[ None , None , None ]:

        yield from (
            42   # <- a scalar has nothing to delegate
        )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from collections.abc import Generator as Stream


def cadence() -> Stream[None, None, None]:
    yield from (None, None)
"#,
            ),
            renamed(
                r#"
import typing


def metre() -> typing.Generator[None, None, None]:
    yield from (None, None)
"#,
            ),
        ],
    }
    .assert("yield from requires an iterable")
}

// ── `with` requires `__enter__` and `__exit__` ───────────────────────────

#[test]
fn with_statement_requires_the_context_manager_protocol() -> Result<(), Box<dyn std::error::Error>>
{
    SpecObligation {
        spec_reason: "`with x` calls `type(x).__enter__` and `type(x).__exit__`; a bare class \
                      declares neither",
        rejected: r#"
class Bulkhead:
    ...


with Bulkhead():
    pass
"#,
        accepted: r#"
import types


class Bulkhead:
    def __enter__(self) -> "Bulkhead":
        return self

    def __exit__(
        self,
        failure: type[BaseException] | None,
        detail: BaseException | None,
        trail: types.TracebackType | None,
    ) -> None:
        return None


with Bulkhead():
    pass
"#,
        rejected_variants: &[
            renamed(
                r#"
class Coffer:
    ...


with Coffer():
    pass
"#,
            ),
            reformatted(
                "
class Bulkhead:   # no enter, no exit

        ...

with (
    Bulkhead()
):
        pass
",
            ),
            import_form(
                r#"
import builtins


class Bulkhead:
    def seal(self) -> builtins.int:
        return 0


with Bulkhead():
    pass
"#,
            ),
            aliased(
                r#"
from builtins import object as Base


class Bulkhead(Base):
    ...


with Bulkhead():
    pass
"#,
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
import types


class Coffer:
    def __enter__(self) -> "Coffer":
        return self

    def __exit__(
        self,
        kind: type[BaseException] | None,
        value: BaseException | None,
        trace: types.TracebackType | None,
    ) -> None:
        return None


with Coffer():
    pass
"#,
            ),
            import_form(
                r#"
import types


class Bulkhead:
    def __enter__(self) -> "Bulkhead":
        return self

    def __exit__(
        self,
        failure: type[BaseException] | None,
        detail: BaseException | None,
        trail: types.TracebackType | None,
    ) -> None:
        return None


with Bulkhead():
    pass
"#,
            ),
        ],
    }
    .assert("with statement requires the context manager protocol")
}
