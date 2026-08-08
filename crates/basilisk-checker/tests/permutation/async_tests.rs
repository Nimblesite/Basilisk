//! Asynchronous typing: `Awaitable`, coroutines, `async for`, `async with`,
//! async generators, and `contextlib.asynccontextmanager`.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `Awaitable`, `AsyncIterator`, `AsyncIterable`, `AsyncGenerator`,
//! `AsyncContextManager` and `Generator` are outside the 55 typing symbols
//! `conformance/tests/` imports, so no rule can carry a hardcoded arm for them;
//! they appear bare and freely. `Coroutine` and `Any` *are* in that vocabulary
//! and are therefore quarantined — they occur only under an alias
//! (`Coroutine as SuspendedCall`, `Any as Unconstrained`) or an alternate import
//! form (`typing.Coroutine`, `collections.abc.Coroutine`), never bare.
//! Identifiers are drawn from a vocabulary disjoint from the suite's 913.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── `await` operand ──────────────────────────────────────────────────────
// The spec makes the operand of `await` an *awaitable*: an object whose type
// implements `__await__` returning an iterator. A class that merely has methods
// is not awaitable, and no nominal relationship rescues it.

const AWAIT_REJECTED: &str = r"
class Windlass:
    def haul(self) -> int:
        return 3


async def crank() -> int:
    return await Windlass()
";

const AWAIT_ACCEPTED: &str = r"
from typing import Generator


class Windlass:
    def __await__(self) -> Generator[None, None, int]:
        yield None
        return 3


async def crank() -> int:
    return await Windlass()
";

const AWAIT_REJECTED_RENAMED: &str = r"
class Capstan:
    def drag(self) -> int:
        return 3


async def wind() -> int:
    return await Capstan()
";

const AWAIT_REJECTED_REFORMATTED: &str = r"
class Windlass:  # a hauling drum, and nothing more

        def haul(self) -> int:

                return 3

async def crank() -> int:
        # the defect is on the next line
        return await (Windlass())
";

const AWAIT_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc


class Windlass:
    def __await__(self) -> collections.abc.Generator[None, None, int]:
        yield None
        return 3


async def crank() -> int:
    return await Windlass()
";

#[test]
fn await_requires_dunder_await() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the operand of `await` must implement `__await__`",
        rejected: AWAIT_REJECTED,
        accepted: AWAIT_ACCEPTED,
        rejected_variants: &[
            renamed(AWAIT_REJECTED_RENAMED),
            reformatted(AWAIT_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[import_form(AWAIT_ACCEPTED_IMPORT_FORM)],
    }
    .assert("await operand must be awaitable")
}

// ── Calling an `async def` ───────────────────────────────────────────────
// Calling an `async def` declared `-> int` produces a *coroutine object*, not an
// `int`. The `int` arrives only after `await`.

const CALL_REJECTED: &str = r"
async def tally_firkins() -> int:
    return 12


async def audit() -> None:
    volume: int = tally_firkins()
    print(volume)
";

const CALL_ACCEPTED: &str = r"
async def tally_firkins() -> int:
    return 12


async def audit() -> None:
    volume: int = await tally_firkins()
    print(volume)
";

const CALL_REJECTED_RENAMED: &str = r"
async def count_staves() -> int:
    return 12


async def reckoning() -> None:
    toll: int = count_staves()
    print(toll)
";

const CALL_REJECTED_REFORMATTED: &str = r"
async def tally_firkins() -> int:

        return 12
async def audit() -> None:
        # no await: this binds a coroutine object to an int
        volume: int = (tally_firkins())

        print(volume)
";

#[test]
fn unawaited_call_is_not_its_return_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "calling an `async def` yields a coroutine object, not its return type",
        rejected: CALL_REJECTED,
        accepted: CALL_ACCEPTED,
        rejected_variants: &[
            renamed(CALL_REJECTED_RENAMED),
            reformatted(CALL_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("unawaited coroutine call")
}

// ── The coroutine type itself ────────────────────────────────────────────
// An `async def` returning `int` has call type `Coroutine[Any, Any, int]`; the
// third argument is the awaited result, so `Coroutine[Any, Any, str]` is wrong.
// `Coroutine` and `Any` are quarantined: aliased here, import-form below.

const COROUTINE_REJECTED: &str = r"
from typing import Any as Unconstrained, Coroutine as SuspendedCall


async def tally_firkins() -> int:
    return 12


async def audit() -> None:
    pending: SuspendedCall[Unconstrained, Unconstrained, str] = tally_firkins()
    print(await pending)
";

const COROUTINE_ACCEPTED: &str = r"
from typing import Any as Unconstrained, Coroutine as SuspendedCall


async def tally_firkins() -> int:
    return 12


async def audit() -> None:
    pending: SuspendedCall[Unconstrained, Unconstrained, int] = tally_firkins()
    print(await pending)
";

const COROUTINE_REJECTED_IMPORT_FORM: &str = r"
import typing


async def tally_firkins() -> int:
    return 12


async def audit() -> None:
    pending: typing.Coroutine[typing.Any, typing.Any, str] = tally_firkins()
    print(await pending)
";

const COROUTINE_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc
import typing


async def tally_firkins() -> int:
    return 12


async def audit() -> None:
    pending: collections.abc.Coroutine[typing.Any, typing.Any, int] = tally_firkins()
    print(await pending)
";

#[test]
fn coroutine_carries_the_awaited_result_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an `async def` returning `int` has type `Coroutine[Any, Any, int]`",
        rejected: COROUTINE_REJECTED,
        accepted: COROUTINE_ACCEPTED,
        rejected_variants: &[import_form(COROUTINE_REJECTED_IMPORT_FORM)],
        accepted_variants: &[import_form(COROUTINE_ACCEPTED_IMPORT_FORM)],
    }
    .assert("coroutine result parameter")
}

// ── `async for` ──────────────────────────────────────────────────────────
// `async for` resolves `__aiter__`/`__anext__`. The synchronous iteration
// protocol does not satisfy it, however complete it is.

const ASYNC_FOR_REJECTED: &str = r"
class Coppice:
    def __iter__(self):
        return iter([1, 2])


async def gather() -> None:
    async for sapling in Coppice():
        print(sapling)
";

const ASYNC_FOR_ACCEPTED: &str = r"
from typing import AsyncIterator


class Coppice:
    def __aiter__(self) -> AsyncIterator[int]:
        return self

    async def __anext__(self) -> int:
        raise StopAsyncIteration


async def gather() -> None:
    async for sapling in Coppice():
        print(sapling)
";

const ASYNC_FOR_REJECTED_RENAMED: &str = r"
class Bosket:
    def __iter__(self):
        return iter([1, 2])


async def quench() -> None:
    async for withy in Bosket():
        print(withy)
";

const ASYNC_FOR_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc


class Coppice:
    def __aiter__(self) -> collections.abc.AsyncIterator[int]:
        return self

    async def __anext__(self) -> int:
        raise StopAsyncIteration


async def gather() -> None:
    async for sapling in Coppice():
        print(sapling)
";

#[test]
fn async_for_requires_the_async_iteration_protocol() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`async for` needs `__aiter__`/`__anext__`; `__iter__` does not stand in",
        rejected: ASYNC_FOR_REJECTED,
        accepted: ASYNC_FOR_ACCEPTED,
        rejected_variants: &[renamed(ASYNC_FOR_REJECTED_RENAMED)],
        accepted_variants: &[import_form(ASYNC_FOR_ACCEPTED_IMPORT_FORM)],
    }
    .assert("async for protocol")
}

// ── Async generator return statement ─────────────────────────────────────
// A function containing `yield` is a generator; an *async* generator may not
// return a value, because there is no channel to deliver it on.

const ASYNC_RETURN_REJECTED: &str = r"
from typing import AsyncGenerator


async def bail_bilge() -> AsyncGenerator[int, None]:
    yield 4
    return 9
";

const ASYNC_RETURN_ACCEPTED: &str = r"
from typing import AsyncGenerator


async def bail_bilge() -> AsyncGenerator[int, None]:
    yield 4
    return
";

const ASYNC_RETURN_REJECTED_ALIASED: &str = r"
from typing import AsyncGenerator as AsyncYieldStream


async def bail_bilge() -> AsyncYieldStream[int, None]:
    yield 4
    return 9
";

const ASYNC_RETURN_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc


async def bail_bilge() -> collections.abc.AsyncGenerator[int, None]:
    yield 4
    return
";

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

const YIELD_REJECTED_IMPORT_FORM: &str = r#"
import collections.abc


async def recite_marks() -> collections.abc.AsyncGenerator[str, None]:
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
            import_form(YIELD_REJECTED_IMPORT_FORM),
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


async def storm() -> None:
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
// The decorator turns an async generator into an async context manager whose
// `as` target has the type the single `yield` produces.

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
// Awaiting an `Awaitable[bytes]` produces `bytes`; the awaited type is the
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

// ── `AsyncIterable[T]` as a parameter type ───────────────────────────────
// A synchronous iterable is not an `AsyncIterable`; the call of an async
// generator function is.

const ITERABLE_REJECTED: &str = r"
from typing import AsyncIterable


async def drain(spigot: AsyncIterable[int]) -> int:
    reckoning: int = 0
    async for parcel in spigot:
        reckoning += parcel
    return reckoning


async def kick_off() -> int:
    return await drain([1, 2, 3])
";

const ITERABLE_ACCEPTED: &str = r"
from typing import AsyncGenerator, AsyncIterable


async def runlet() -> AsyncGenerator[int, None]:
    yield 1
    yield 2


async def drain(spigot: AsyncIterable[int]) -> int:
    reckoning: int = 0
    async for parcel in spigot:
        reckoning += parcel
    return reckoning


async def kick_off() -> int:
    return await drain(runlet())
";

const ITERABLE_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc


async def runlet() -> collections.abc.AsyncGenerator[int, None]:
    yield 1
    yield 2


async def drain(spigot: collections.abc.AsyncIterable[int]) -> int:
    reckoning: int = 0
    async for parcel in spigot:
        reckoning += parcel
    return reckoning


async def kick_off() -> int:
    return await drain(runlet())
";

#[test]
fn async_iterable_parameter_rejects_a_sync_list() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`list[int]` has no `__aiter__`, so it is not an `AsyncIterable[int]`",
        rejected: ITERABLE_REJECTED,
        accepted: ITERABLE_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[import_form(ITERABLE_ACCEPTED_IMPORT_FORM)],
    }
    .assert("AsyncIterable parameter")
}
