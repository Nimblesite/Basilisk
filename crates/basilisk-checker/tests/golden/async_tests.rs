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

// ── `await` operand ──────────────────────────────────────────────────────
// The operand of `await` must be an awaitable: an object whose type implements
// `__await__` returning an iterator. Having methods is not having that one.

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

const AWAIT_REJECTED_REFORMATTED: &str = "
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
// Calling an `async def` declared `-> int` produces a coroutine object, not an
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

const CALL_REJECTED_REFORMATTED: &str = "
async def tally_firkins() -> int:

        return 12
async def audit() -> None:
        # no await, so this binds a coroutine object to an int
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
// third argument is the awaited result, so `str` there is wrong. `Coroutine`
// and `Any` are quarantined: aliased in the canonical pair, import-form below.

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

// ── `async for` and `AsyncIterable` ──────────────────────────────────────
// `async for` resolves `__aiter__`/`__anext__`, and `AsyncIterable[int]` is
// satisfied by `__aiter__` alone. The synchronous iteration protocol satisfies
// neither, however complete it is.

const ASYNC_FOR_REJECTED: &str = r"
from typing import AsyncIterable

class Coppice:
    def __iter__(self):
        return iter([1, 2])

async def gather(grove: AsyncIterable[int]) -> None:
    async for sapling in grove:
        print(sapling)

async def tend() -> None:
    await gather(Coppice())
";

const ASYNC_FOR_ACCEPTED: &str = r"
from typing import AsyncIterable, AsyncIterator

class Coppice:
    def __aiter__(self) -> AsyncIterator[int]:
        return self

    async def __anext__(self) -> int:
        raise StopAsyncIteration

async def gather(grove: AsyncIterable[int]) -> None:
    async for sapling in grove:
        print(sapling)

async def tend() -> None:
    await gather(Coppice())
";

const ASYNC_FOR_REJECTED_RENAMED: &str = r"
from typing import AsyncIterable

class Bosket:
    def __iter__(self):
        return iter([1, 2])

async def quench(thicket: AsyncIterable[int]) -> None:
    async for withy in thicket:
        print(withy)

async def storm() -> None:
    await quench(Bosket())
";

const ASYNC_FOR_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc

class Coppice:
    def __aiter__(self) -> collections.abc.AsyncIterator[int]:
        return self

    async def __anext__(self) -> int:
        raise StopAsyncIteration

async def gather(grove: collections.abc.AsyncIterable[int]) -> None:
    async for sapling in grove:
        print(sapling)

async def tend() -> None:
    await gather(Coppice())
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
// A function containing `yield` is a generator; an *async* generator has no
// channel to deliver a return value on, so `return <value>` is ill-formed.

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
