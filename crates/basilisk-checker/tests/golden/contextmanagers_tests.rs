//! `with` statements and the `contextlib` surface. [PERMTEST-FAMILY-B] /
//! [PERMTEST-VOCABULARY].
//!
//! `contextlib` is absent from `conformance/tests/` in its entirety — no
//! `contextmanager`, `AbstractContextManager`, `ExitStack`, `closing`,
//! `suppress` or `nullcontext` appears anywhere in the suite — so no rule can
//! carry a hardcoded arm for any of it. `typing.ContextManager` is likewise
//! out of vocabulary and used bare. `Iterator`, which the suite *does* import,
//! is **quarantined**: it appears here only as `collections.abc.Iterator`
//! (A7 import form) or under an alias (A6), never bare from `typing`.
//!
//! Identifiers come from a cooperage/waterworks vocabulary disjoint from the
//! 913 names the suite defines.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── The context-manager protocol ─────────────────────────────────────────
// `with EXPR:` looks up `__enter__` and `__exit__` on `type(EXPR)`. An object
// supplying neither is not a context manager, however ordinary the rest of
// its interface looks.

pub(super) const NO_PROTOCOL_REJECTED: &str = r"
class Grommet:
    def seat(self) -> int:
        return 4

def fit_bushing() -> None:
    with Grommet():
        pass
";

pub(super) const NO_PROTOCOL_ACCEPTED: &str = r"
class Grommet:
    def seat(self) -> int:
        return 4
    def __enter__(self) -> None: return None
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def fit_bushing() -> None:
    with Grommet():
        pass
";

pub(super) const NO_PROTOCOL_REJECTED_RENAMED: &str = r"
class Spigot:
    def bore(self) -> int:
        return 4

def tap_cask() -> None:
    with Spigot():
        pass
";

pub(super) const NO_PROTOCOL_REJECTED_REFORMATTED: &str = r"
class Grommet:  # a rubber ring, not a context manager

        def seat(self) -> int:

                return 4

def fit_bushing() -> None:
        # neither __enter__ nor __exit__ is defined
        with Grommet():
                pass
";

#[test]
fn with_requires_the_context_manager_protocol() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`with` needs `__enter__` and `__exit__` on the object's type",
        rejected: NO_PROTOCOL_REJECTED,
        accepted: NO_PROTOCOL_ACCEPTED,
        rejected_variants: &[
            renamed(NO_PROTOCOL_REJECTED_RENAMED),
            reformatted(NO_PROTOCOL_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("with statement requires the context-manager protocol")
}

// Half the protocol is not the protocol: `__enter__` alone leaves the
// with-statement's exit path unimplementable.

pub(super) const HALF_PROTOCOL_REJECTED: &str = r"
class Tailrace:
    def __enter__(self) -> int:
        return 9

def run_off() -> None:
    with Tailrace() as depth:
        print(depth)
";

pub(super) const HALF_PROTOCOL_ACCEPTED: &str = r"
class Tailrace:
    def __enter__(self) -> int:
        return 9
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def run_off() -> None:
    with Tailrace() as depth:
        print(depth)
";

#[test]
fn enter_without_exit_is_not_a_context_manager() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`__enter__` without `__exit__` does not satisfy the protocol",
        rejected: HALF_PROTOCOL_REJECTED,
        accepted: HALF_PROTOCOL_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("half the context-manager protocol")
}

// ── The `as` target ──────────────────────────────────────────────────────
// `with EXPR as NAME:` binds NAME to whatever `__enter__` returns, not to the
// context manager itself. That distinction is the whole point of `as`.

pub(super) const AS_TARGET_REJECTED: &str = r"
class Sluice:
    def draw(self) -> int:
        return 11

class Penstock:
    def __enter__(self) -> Sluice:
        return Sluice()
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def draw_off() -> None:
    with Penstock() as gate:
        gate.spill()
";

pub(super) const AS_TARGET_ACCEPTED: &str = r"
class Sluice:
    def draw(self) -> int:
        return 11

class Penstock:
    def __enter__(self) -> Sluice:
        return Sluice()
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def draw_off() -> None:
    with Penstock() as gate:
        gate.draw()
";

pub(super) const AS_TARGET_REJECTED_RENAMED: &str = r"
class Weir:
    def siphon(self) -> int:
        return 11

class Headrace:
    def __enter__(self) -> Weir:
        return Weir()
    def __exit__(self, first: object, second: object, third: object) -> None: return None

def bleed_line() -> None:
    with Headrace() as port:
        port.vent()
";

#[test]
fn as_target_takes_the_enter_return_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the `as` target is typed by `__enter__`'s return, and `Sluice` has no `spill`",
        rejected: AS_TARGET_REJECTED,
        accepted: AS_TARGET_ACCEPTED,
        rejected_variants: &[renamed(AS_TARGET_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("as target takes the __enter__ return type")
}

// ── @contextlib.contextmanager ───────────────────────────────────────────
// The decorator maps `Callable[P, Iterator[T]]` to a factory of context
// managers over `T`, so a generator yielding `str` gives an `as` target of
// type `str` and nothing else.

pub(super) const YIELD_TYPE_REJECTED: &str = r#"
import collections.abc
import contextlib

@contextlib.contextmanager
def tapped_firkin() -> collections.abc.Iterator[str]:
    yield "oak"

def stave_count(hoops: int) -> int: return hoops
def brand(text: str) -> int: return len(text)

def broach() -> None:
    with tapped_firkin() as stencil:
        stave_count(stencil)
"#;

pub(super) const YIELD_TYPE_ACCEPTED: &str = r#"
import collections.abc
import contextlib

@contextlib.contextmanager
def tapped_firkin() -> collections.abc.Iterator[str]:
    yield "oak"

def stave_count(hoops: int) -> int: return hoops
def brand(text: str) -> int: return len(text)

def broach() -> None:
    with tapped_firkin() as stencil:
        brand(stencil)
"#;

pub(super) const YIELD_TYPE_REJECTED_ALIASED: &str = r#"
from collections.abc import Iterator as YieldsOf
from contextlib import contextmanager as scoped

@scoped
def tapped_firkin() -> YieldsOf[str]:
    yield "oak"

def stave_count(hoops: int) -> int: return hoops
def brand(text: str) -> int: return len(text)

def broach() -> None:
    with tapped_firkin() as stencil:
        stave_count(stencil)
"#;

#[test]
fn contextmanager_yield_type_reaches_the_as_target() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `@contextmanager` generator yielding `str` produces a context manager \
                      over `str`, not over `int`",
        rejected: YIELD_TYPE_REJECTED,
        accepted: YIELD_TYPE_ACCEPTED,
        rejected_variants: &[aliased(YIELD_TYPE_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("contextmanager yield type")
}

// `contextmanager` accepts an iterator-returning callable. A plain function
// returning `str` is not one, so the decoration itself is ill-typed.

pub(super) const NON_GENERATOR_REJECTED: &str = r#"
import contextlib

@contextlib.contextmanager
def shuttered_kiln() -> str:
    return "cool"
"#;

pub(super) const NON_GENERATOR_ACCEPTED: &str = r#"
import collections.abc
import contextlib

@contextlib.contextmanager
def shuttered_kiln() -> collections.abc.Iterator[str]:
    yield "cool"
"#;

pub(super) const NON_GENERATOR_REJECTED_ALIASED: &str = r#"
from contextlib import contextmanager as scoped

@scoped
def shuttered_kiln() -> str:
    return "cool"
"#;

#[test]
fn contextmanager_requires_an_iterator_returning_callable(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`contextmanager` takes `Callable[..., Iterator[T]]`, and a `-> str` \
                      function is not assignable to it",
        rejected: NON_GENERATOR_REJECTED,
        accepted: NON_GENERATOR_ACCEPTED,
        rejected_variants: &[aliased(NON_GENERATOR_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("contextmanager argument shape")
}

// ── `__exit__`'s return type ─────────────────────────────────────────────
// A truthy `__exit__` return swallows the exception, so a `with` body that
// always raises can still fall through and the narrowing that preceded it
// does not hold afterwards. A `None` return cannot suppress, so it does.

pub(super) const SUPPRESSING_EXIT_REJECTED: &str = r"
class Cofferdam:
    def __enter__(self) -> None: return None
    def __exit__(self, kind: object, value: object, trace: object) -> bool: return True

def gauge(datum: int | str) -> str:
    if isinstance(datum, int):
        with Cofferdam():
            raise ValueError
    return datum
";

pub(super) const SUPPRESSING_EXIT_ACCEPTED: &str = r"
class Cofferdam:
    def __enter__(self) -> None: return None
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def gauge(datum: int | str) -> str:
    if isinstance(datum, int):
        with Cofferdam():
            raise ValueError
    return datum
";

pub(super) const SUPPRESSING_EXIT_REJECTED_REFORMATTED: &str = r"
class Cofferdam:
        def __enter__(self) -> None:
                return None

        # a bool return may swallow the exception
        def __exit__(
                self,
                kind: object,
                value: object,
                trace: object,
        ) -> bool:
                return True

def gauge(datum: int | str) -> str:
        if isinstance(datum, int):
                with Cofferdam():
                        raise ValueError
        return datum
";
