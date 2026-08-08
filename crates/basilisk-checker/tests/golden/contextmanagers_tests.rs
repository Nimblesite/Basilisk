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

const NO_PROTOCOL_REJECTED: &str = r"
class Grommet:
    def seat(self) -> int:
        return 4

def fit_bushing() -> None:
    with Grommet():
        pass
";

const NO_PROTOCOL_ACCEPTED: &str = r"
class Grommet:
    def seat(self) -> int:
        return 4
    def __enter__(self) -> None: return None
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def fit_bushing() -> None:
    with Grommet():
        pass
";

const NO_PROTOCOL_REJECTED_RENAMED: &str = r"
class Spigot:
    def bore(self) -> int:
        return 4

def tap_cask() -> None:
    with Spigot():
        pass
";

const NO_PROTOCOL_REJECTED_REFORMATTED: &str = r"
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

const HALF_PROTOCOL_REJECTED: &str = r"
class Tailrace:
    def __enter__(self) -> int:
        return 9

def run_off() -> None:
    with Tailrace() as depth:
        print(depth)
";

const HALF_PROTOCOL_ACCEPTED: &str = r"
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

const AS_TARGET_REJECTED: &str = r"
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

const AS_TARGET_ACCEPTED: &str = r"
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

const AS_TARGET_REJECTED_RENAMED: &str = r"
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

const YIELD_TYPE_REJECTED: &str = r#"
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

const YIELD_TYPE_ACCEPTED: &str = r#"
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

const YIELD_TYPE_REJECTED_ALIASED: &str = r#"
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

const NON_GENERATOR_REJECTED: &str = r#"
import contextlib

@contextlib.contextmanager
def shuttered_kiln() -> str:
    return "cool"
"#;

const NON_GENERATOR_ACCEPTED: &str = r#"
import collections.abc
import contextlib

@contextlib.contextmanager
def shuttered_kiln() -> collections.abc.Iterator[str]:
    yield "cool"
"#;

const NON_GENERATOR_REJECTED_ALIASED: &str = r#"
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

const SUPPRESSING_EXIT_REJECTED: &str = r"
class Cofferdam:
    def __enter__(self) -> None: return None
    def __exit__(self, kind: object, value: object, trace: object) -> bool: return True

def gauge(datum: int | str) -> str:
    if isinstance(datum, int):
        with Cofferdam():
            raise ValueError
    return datum
";

const SUPPRESSING_EXIT_ACCEPTED: &str = r"
class Cofferdam:
    def __enter__(self) -> None: return None
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def gauge(datum: int | str) -> str:
    if isinstance(datum, int):
        with Cofferdam():
            raise ValueError
    return datum
";

const SUPPRESSING_EXIT_REJECTED_REFORMATTED: &str = r"
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

#[test]
fn exit_returning_bool_defeats_the_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `bool`-returning `__exit__` may suppress, so `datum` reaches the return \
                      as `int | str`; a `None`-returning one cannot, so it reaches it as `str`",
        rejected: SUPPRESSING_EXIT_REJECTED,
        accepted: SUPPRESSING_EXIT_ACCEPTED,
        rejected_variants: &[reformatted(SUPPRESSING_EXIT_REJECTED_REFORMATTED)],
        accepted_variants: &[],
    }
    .assert("__exit__ return type and exception suppression")
}

// ── contextlib.ExitStack ─────────────────────────────────────────────────
// `enter_context` returns exactly what the entered manager's `__enter__`
// returns, so the entered type propagates out of the call.

const ENTER_CONTEXT_REJECTED: &str = r"
import contextlib

class Bellows:
    def __enter__(self) -> int:
        return 3
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def widen_stack(stack: contextlib.ExitStack) -> None:
    gust = stack.enter_context(Bellows())
    gust.upper()
";

const ENTER_CONTEXT_ACCEPTED: &str = r"
import contextlib

class Bellows:
    def __enter__(self) -> int:
        return 3
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def widen_stack(stack: contextlib.ExitStack) -> None:
    gust = stack.enter_context(Bellows())
    gust.bit_length()
";

const ENTER_CONTEXT_REJECTED_IMPORT_FORM: &str = r"
from contextlib import ExitStack

class Bellows:
    def __enter__(self) -> int:
        return 3
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def widen_stack(stack: ExitStack) -> None:
    gust = stack.enter_context(Bellows())
    gust.upper()
";

#[test]
fn enter_context_propagates_the_entered_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`ExitStack.enter_context` returns the entered manager's `__enter__` type, \
                      and `int` has no `upper`",
        rejected: ENTER_CONTEXT_REJECTED,
        accepted: ENTER_CONTEXT_ACCEPTED,
        rejected_variants: &[import_form(ENTER_CONTEXT_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("ExitStack.enter_context type propagation")
}

const ENTER_CONTEXT_ARG_REJECTED: &str = r"
import contextlib

class Tuyere:
    def blast(self) -> int:
        return 1

def charge(stack: contextlib.ExitStack) -> None:
    stack.enter_context(Tuyere())
";

const ENTER_CONTEXT_ARG_ACCEPTED: &str = r"
import contextlib

class Tuyere:
    def blast(self) -> int:
        return 1
    def __enter__(self) -> int:
        return self.blast()
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def charge(stack: contextlib.ExitStack) -> None:
    stack.enter_context(Tuyere())
";

#[test]
fn enter_context_requires_a_context_manager() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`enter_context` is parameterised over context managers, and `Tuyere` \
                      supplies neither dunder",
        rejected: ENTER_CONTEXT_ARG_REJECTED,
        accepted: ENTER_CONTEXT_ARG_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("ExitStack.enter_context argument")
}

// ── contextlib.suppress ──────────────────────────────────────────────────
// `suppress(*exceptions: type[BaseException])` is parameterised over
// exception **classes**. An instance is a value of such a class, never one.

const SUPPRESS_REJECTED: &str = r#"
import contextlib

def quell() -> None:
    with contextlib.suppress(ValueError("bad bung")):
        pass
"#;

const SUPPRESS_ACCEPTED: &str = r"
import contextlib

def quell() -> None:
    with contextlib.suppress(ValueError):
        pass
";

const SUPPRESS_REJECTED_ALIASED: &str = r#"
from contextlib import suppress as quelling

def quell() -> None:
    with quelling(ValueError("bad bung")):
        pass
"#;

#[test]
fn suppress_takes_exception_classes_not_instances() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`suppress` accepts `type[BaseException]`, and `ValueError(...)` is an \
                      instance rather than the class",
        rejected: SUPPRESS_REJECTED,
        accepted: SUPPRESS_ACCEPTED,
        rejected_variants: &[aliased(SUPPRESS_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("contextlib.suppress argument kind")
}

// ── contextlib.closing ───────────────────────────────────────────────────
// `closing` is bounded by objects supplying `close()`, and re-exposes the
// wrapped object as the `as` target.

const CLOSING_REJECTED: &str = r"
import contextlib

class Croze:
    def cut(self) -> int:
        return 2

def finish_stave() -> None:
    with contextlib.closing(Croze()) as tool:
        tool.cut()
";

const CLOSING_ACCEPTED: &str = r"
import contextlib

class Croze:
    def cut(self) -> int:
        return 2
    def close(self) -> None: return None

def finish_stave() -> None:
    with contextlib.closing(Croze()) as tool:
        tool.cut()
";

const CLOSING_ACCEPTED_RENAMED: &str = r"
import contextlib

class Chime:
    def shave(self) -> int:
        return 2
    def close(self) -> None: return None

def dress_stave() -> None:
    with contextlib.closing(Chime()) as edge:
        edge.shave()
";

#[test]
fn closing_requires_a_close_method() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`closing` is bounded by objects with `close()`, which `Croze` lacks",
        rejected: CLOSING_REJECTED,
        accepted: CLOSING_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[renamed(CLOSING_ACCEPTED_RENAMED)],
    }
    .assert("contextlib.closing bound")
}

// ── contextlib.nullcontext ───────────────────────────────────────────────
// `nullcontext(x)` enters to `x` itself, so the `as` target carries `x`'s
// type rather than the wrapper's.

const NULLCONTEXT_REJECTED: &str = r#"
import contextlib

def bare_run() -> None:
    with contextlib.nullcontext("mordant") as agent:
        agent.bit_length()
"#;

const NULLCONTEXT_ACCEPTED: &str = r#"
import contextlib

def bare_run() -> None:
    with contextlib.nullcontext("mordant") as agent:
        agent.upper()
"#;

const NULLCONTEXT_REJECTED_ALIASED: &str = r#"
from contextlib import nullcontext as passthrough

def bare_run() -> None:
    with passthrough("mordant") as agent:
        agent.bit_length()
"#;

#[test]
fn nullcontext_enters_to_its_argument() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`nullcontext(x)` binds the `as` target to `x`, and `str` has no `bit_length`",
        rejected: NULLCONTEXT_REJECTED,
        accepted: NULLCONTEXT_ACCEPTED,
        rejected_variants: &[aliased(NULLCONTEXT_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("contextlib.nullcontext entered type")
}

// ── AbstractContextManager as an annotation ──────────────────────────────
// `AbstractContextManager[T]` is satisfied structurally by an `__enter__`
// returning `T`. `typing.ContextManager` is the same object reached by a
// different binding path, so it must decide identically.

const ACM_PARAM_REJECTED: &str = r#"
import contextlib

class Tundish:
    def __enter__(self) -> bytes:
        return b"pour"
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def decant(wellhead: contextlib.AbstractContextManager[str]) -> None:
    with wellhead as sluicing:
        print(sluicing.upper())

decant(Tundish())
"#;

const ACM_PARAM_ACCEPTED: &str = r#"
import contextlib

class Tundish:
    def __enter__(self) -> str:
        return "pour"
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def decant(wellhead: contextlib.AbstractContextManager[str]) -> None:
    with wellhead as sluicing:
        print(sluicing.upper())

decant(Tundish())
"#;

const ACM_PARAM_REJECTED_IMPORT_FORM: &str = r#"
from typing import ContextManager

class Tundish:
    def __enter__(self) -> bytes:
        return b"pour"
    def __exit__(self, kind: object, value: object, trace: object) -> None: return None

def decant(wellhead: ContextManager[str]) -> None:
    with wellhead as sluicing:
        print(sluicing.upper())

decant(Tundish())
"#;

#[test]
fn abstract_context_manager_parameter_checks_the_entered_type(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`AbstractContextManager[str]` needs `__enter__` returning `str`, and a \
                      `bytes` return does not satisfy it",
        rejected: ACM_PARAM_REJECTED,
        accepted: ACM_PARAM_ACCEPTED,
        rejected_variants: &[import_form(ACM_PARAM_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("AbstractContextManager entered type")
}
