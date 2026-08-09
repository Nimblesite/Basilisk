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

#[allow(
    clippy::wildcard_imports,
    unused_imports,
    reason = "shared golden fixtures: each sibling uses the subset it references"
)]
use super::contextmanagers::*;
use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

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
