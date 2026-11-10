//! Asynchronous typing: `Awaitable`, coroutines, `async for`, `async with`,
//! async generators and `contextlib.asynccontextmanager`.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `Awaitable`, `AsyncIterator`, `AsyncIterable`, `AsyncGenerator`,
//! `AsyncContextManager` and `Generator` sit outside the 55 typing symbols
//! `conformance/tests/` imports, so no rule can carry a hardcoded arm for them;
//! they appear bare. `Coroutine` and `Any` *are* in that vocabulary and are
//! quarantined — they occur only under an alias (`Coroutine as SuspendedCall`,
//! `Any as Unconstrained`) or an alternate import form (`typing.Coroutine`,
//! `collections.abc.Coroutine`), never bare. Identifiers are drawn from a
//! vocabulary disjoint from the suite's 913.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};
#[allow(
    clippy::wildcard_imports,
    unused_imports,
    reason = "shared golden fixtures: each sibling uses the subset it references"
)]
use super::r#async::*;

#[test]
fn async_generator_may_not_return_a_value() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an async generator function may not use `return <value>`",
        rejected: ASYNC_RETURN_REJECTED,
        accepted: ASYNC_RETURN_ACCEPTED,
        rejected_variants: &[aliased(ASYNC_RETURN_REJECTED_ALIASED)],
        accepted_variants: &[import_form(ASYNC_RETURN_ACCEPTED_IMPORT_FORM)],
    }
    .assert("async generator return value")
}

// ── Async generator yield type ───────────────────────────────────────────
// The first argument of `AsyncGenerator[Y, S]` is the yield type; every `yield`
// expression is checked against it.

const YIELD_REJECTED: &str = r#"
from typing import AsyncGenerator

async def recite_marks() -> AsyncGenerator[str, None]:
    yield b"portside"
"#;

const YIELD_ACCEPTED: &str = r#"
from typing import AsyncGenerator

async def recite_marks() -> AsyncGenerator[str, None]:
    yield "portside"
"#;

const YIELD_REJECTED_ALIASED: &str = r#"
from typing import AsyncGenerator as AsyncYieldStream

async def recite_marks() -> AsyncYieldStream[str, None]:
    yield b"portside"
"#;

const YIELD_REJECTED_REFORMATTED: &str = "
from typing import AsyncGenerator
async def recite_marks() -> AsyncGenerator[
        str,
        None,
]:
        # bytes where the annotation promises str
        yield b'portside'
";

#[test]
fn async_generator_yield_type_is_load_bearing() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`AsyncGenerator[str, None]` may not yield `bytes`",
        rejected: YIELD_REJECTED,
        accepted: YIELD_ACCEPTED,
        rejected_variants: &[
            aliased(YIELD_REJECTED_ALIASED),
            reformatted(YIELD_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("async generator yield type")
}

// ── Async generator declared return type ─────────────────────────────────
// A function containing `yield` never returns its yield type directly; its
// declared return type must be an async-iterable type.

const DECL_REJECTED: &str = r"
async def count_staves() -> int:
    yield 3
";

const DECL_ACCEPTED: &str = r"
from typing import AsyncIterator

async def count_staves() -> AsyncIterator[int]:
    yield 3
";

const DECL_ACCEPTED_ALIASED: &str = r"
from typing import AsyncIterator as AsyncStream

async def count_staves() -> AsyncStream[int]:
    yield 3
";

#[test]
fn async_generator_return_annotation_must_be_async_iterable(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an async generator function returns an async iterator, never its yield type",
        rejected: DECL_REJECTED,
        accepted: DECL_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[aliased(DECL_ACCEPTED_ALIASED)],
    }
    .assert("async generator return annotation")
}

// ── `async with` ─────────────────────────────────────────────────────────
// `async with` resolves `__aenter__`/`__aexit__`. The synchronous context
// manager protocol does not satisfy it.

const ASYNC_WITH_REJECTED: &str = r#"
class Postern:
    def __enter__(self) -> "Postern":
        return self

    def __exit__(self, mishap: object, trouble: object, trace: object) -> None:
        return None

async def breach() -> None:
    async with Postern() as gate:
        print(gate)
"#;

const ASYNC_WITH_ACCEPTED: &str = r#"
class Postern:
    async def __aenter__(self) -> "Postern":
        return self

    async def __aexit__(self, mishap: object, trouble: object, trace: object) -> None:
        return None

async def breach() -> None:
    async with Postern() as gate:
        print(gate)
"#;

const ASYNC_WITH_REJECTED_RENAMED: &str = r#"
class Barbican:
    def __enter__(self) -> "Barbican":
        return self

    def __exit__(self, upset: object, bother: object, unwind: object) -> None:
        return None

async def sally() -> None:
    async with Barbican() as hatch:
        print(hatch)
"#;

const ASYNC_WITH_ACCEPTED_REFORMATTED: &str = "
class Postern:  # a back gate, opened asynchronously

        async def __aenter__(self) -> 'Postern':
                return self
        async def __aexit__(
                self,
                mishap: object,
                trouble: object,
                trace: object,
        ) -> None:
                return None
async def breach() -> None:
        async with Postern() as gate:
                print(gate)
";

#[test]
fn async_with_requires_the_async_context_protocol() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`async with` needs `__aenter__`/`__aexit__`, not `__enter__`/`__exit__`",
        rejected: ASYNC_WITH_REJECTED,
        accepted: ASYNC_WITH_ACCEPTED,
        rejected_variants: &[renamed(ASYNC_WITH_REJECTED_RENAMED)],
        accepted_variants: &[reformatted(ASYNC_WITH_ACCEPTED_REFORMATTED)],
    }
    .assert("async with protocol")
}

// ── `contextlib.asynccontextmanager` ─────────────────────────────────────
// The decorator turns an async generator into an `AsyncContextManager[T]`, and
// the `as` target takes the type the single `yield` produces.

const ACM_REJECTED: &str = r#"
import contextlib
from typing import AsyncContextManager, AsyncIterator

@contextlib.asynccontextmanager
async def tap_hogshead() -> AsyncIterator[str]:
    yield "amber"

async def decant() -> None:
    cask: AsyncContextManager[str] = tap_hogshead()
    async with cask as pour:
        ullage: int = pour
        print(ullage)
"#;

const ACM_ACCEPTED: &str = r#"
import contextlib
from typing import AsyncContextManager, AsyncIterator

@contextlib.asynccontextmanager
async def tap_hogshead() -> AsyncIterator[str]:
    yield "amber"

async def decant() -> None:
    cask: AsyncContextManager[str] = tap_hogshead()
    async with cask as pour:
        ullage: str = pour
        print(ullage)
"#;

const ACM_REJECTED_IMPORT_FORM: &str = r#"
import collections.abc
import contextlib
from contextlib import asynccontextmanager

@asynccontextmanager
async def tap_hogshead() -> collections.abc.AsyncIterator[str]:
    yield "amber"

async def decant() -> None:
    cask: contextlib.AbstractAsyncContextManager[str] = tap_hogshead()
    async with cask as pour:
        ullage: int = pour
        print(ullage)
"#;

const ACM_ACCEPTED_ALIASED: &str = r#"
from contextlib import asynccontextmanager as async_scope
from typing import AsyncContextManager as ScopedAsync, AsyncIterator as AsyncStream

@async_scope
async def tap_hogshead() -> AsyncStream[str]:
    yield "amber"

async def decant() -> None:
    cask: ScopedAsync[str] = tap_hogshead()
    async with cask as pour:
        ullage: str = pour
        print(ullage)
"#;

#[test]
fn asynccontextmanager_binds_the_yielded_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an `asynccontextmanager` generator yields the type its `yield` produces",
        rejected: ACM_REJECTED,
        accepted: ACM_ACCEPTED,
        rejected_variants: &[import_form(ACM_REJECTED_IMPORT_FORM)],
        accepted_variants: &[aliased(ACM_ACCEPTED_ALIASED)],
    }
    .assert("asynccontextmanager yield type")
}

// ── `Awaitable[T]` ───────────────────────────────────────────────────────
// Awaiting an `Awaitable[bytes]` produces `bytes`: the awaited type is the
// parameter, not the awaitable itself.

const AWAITABLE_REJECTED: &str = r"
from typing import Awaitable

async def steep(brew: Awaitable[bytes]) -> None:
    liquor: str = await brew
    print(liquor)
";

const AWAITABLE_ACCEPTED: &str = r"
from typing import Awaitable

async def steep(brew: Awaitable[bytes]) -> None:
    liquor: bytes = await brew
    print(liquor)
";

const AWAITABLE_REJECTED_ALIASED: &str = r"
from typing import Awaitable as PendingValue

async def steep(brew: PendingValue[bytes]) -> None:
    liquor: str = await brew
    print(liquor)
";

const AWAITABLE_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc

async def steep(brew: collections.abc.Awaitable[bytes]) -> None:
    liquor: bytes = await brew
    print(liquor)
";

#[test]
fn awaiting_an_awaitable_yields_its_parameter() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`await` on `Awaitable[bytes]` produces `bytes`, not `str`",
        rejected: AWAITABLE_REJECTED,
        accepted: AWAITABLE_ACCEPTED,
        rejected_variants: &[aliased(AWAITABLE_REJECTED_ALIASED)],
        accepted_variants: &[import_form(AWAITABLE_ACCEPTED_IMPORT_FORM)],
    }
    .assert("Awaitable parameter")
}

// ── An `Awaitable[T]` parameter rejects a plain value ────────────────────
// A bare `int` is not an `Awaitable[int]`; the call of an `async def` is.

const PARAM_REJECTED: &str = r"
from typing import Awaitable

async def settle(pending: Awaitable[int]) -> int:
    return await pending

async def sluice_run() -> int:
    return await settle(7)
";

const PARAM_ACCEPTED: &str = r"
from typing import Awaitable

async def yield_toll() -> int:
    return 7

async def settle(pending: Awaitable[int]) -> int:
    return await pending

async def sluice_run() -> int:
    return await settle(yield_toll())
";

const PARAM_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc

async def yield_toll() -> int:
    return 7

async def settle(pending: collections.abc.Awaitable[int]) -> int:
    return await pending

async def sluice_run() -> int:
    return await settle(yield_toll())
";

#[test]
fn awaitable_parameter_rejects_a_plain_value() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`int` has no `__await__`, so it is not an `Awaitable[int]`",
        rejected: PARAM_REJECTED,
        accepted: PARAM_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[import_form(PARAM_ACCEPTED_IMPORT_FORM)],
    }
    .assert("Awaitable argument")
}
