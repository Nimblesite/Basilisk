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

// ── `async with` requires `__aenter__` and `__aexit__` ───────────────────

#[test]
fn async_with_requires_the_async_context_manager_protocol()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`async with x` calls `__aenter__`/`__aexit__`; the synchronous pair does not \
                      satisfy it",
        rejected: r#"
class Airlock:
    ...


async def cycle() -> None:
    async with Airlock():
        pass
"#,
        accepted: r#"
import types


class Airlock:
    async def __aenter__(self) -> "Airlock":
        return self

    async def __aexit__(
        self,
        failure: type[BaseException] | None,
        detail: BaseException | None,
        trail: types.TracebackType | None,
    ) -> None:
        return None


async def cycle() -> None:
    async with Airlock():
        pass
"#,
        rejected_variants: &[
            renamed(
                r#"
class Vestibule:
    ...


async def rotate() -> None:
    async with Vestibule():
        pass
"#,
            ),
            reformatted(
                "
class Airlock:

        ...

async def cycle() -> None:

        async with (
            Airlock()   # <- synchronous at best
        ):
                pass
",
            ),
            // The *synchronous* protocol is present and still insufficient: this is
            // the case a rule keyed to the word `__enter__` gets wrong.
            import_form(
                r#"
import types


class Airlock:
    def __enter__(self) -> "Airlock":
        return self

    def __exit__(
        self,
        failure: type[BaseException] | None,
        detail: BaseException | None,
        trail: types.TracebackType | None,
    ) -> None:
        return None


async def cycle() -> None:
    async with Airlock():
        pass
"#,
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
import types


class Vestibule:
    async def __aenter__(self) -> "Vestibule":
        return self

    async def __aexit__(
        self,
        kind: type[BaseException] | None,
        value: BaseException | None,
        trace: types.TracebackType | None,
    ) -> None:
        return None


async def rotate() -> None:
    async with Vestibule():
        pass
"#,
            ),
            aliased(
                r#"
import types
from builtins import object as Base


class Airlock(Base):
    async def __aenter__(self) -> "Airlock":
        return self

    async def __aexit__(
        self,
        failure: type[BaseException] | None,
        detail: BaseException | None,
        trail: types.TracebackType | None,
    ) -> None:
        return None


async def cycle() -> None:
    async with Airlock():
        pass
"#,
            ),
        ],
    }
    .assert("async with requires the async context manager protocol")
}

// ── A declared return type obliges a returned value ──────────────────────

#[test]
fn declared_return_type_obliges_a_value_on_every_path()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a function annotated `-> int` must return an `int` on every path; falling off \
                      the end returns `None`",
        rejected: r#"
def tally() -> int:
    print("hello")
"#,
        accepted: r#"
def tally() -> int:
    print("hello")
    return 0
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def tally() -> Whole:
    print("hello")
"#,
            ),
            import_form(
                r#"
import builtins


def tally() -> builtins.int:
    builtins.print("hello")
"#,
            ),
            renamed(
                r#"
def reckon() -> int:
    print("hello")
"#,
            ),
            // An implicit fall-through past a conditional return is the same defect.
            reformatted(
                "
def tally() -> int:
        if print:
                return 0
        # <- falls off the end and yields None
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def tally() -> Whole:
    print("hello")
    return 0
"#,
            ),
            renamed(
                r#"
def reckon() -> int:
    print("hello")
    return 0
"#,
            ),
            reformatted(
                "
def tally() -> int:
        if print:
                return 0
        return 1
",
            ),
        ],
    }
    .assert("declared return type obliges a value on every path")
}
