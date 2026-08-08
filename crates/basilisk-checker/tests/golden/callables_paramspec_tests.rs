//! Callable types, `ParamSpec` and `Concatenate`, authored outside the
//! conformance suite's vocabulary. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `Callable`, `ParamSpec`, `Concatenate` and `TypeVar` are quarantined
//! symbols, so they are reached only through `collections.abc.Callable`, a
//! `typing.X` attribute path, an `as` alias, or a PEP 695 `[**PSpindle]`
//! declaration — never bare. `ParamSpecArgs` and `ParamSpecKwargs` lie outside
//! the 55 symbols `conformance/tests/` imports and are used bare on purpose:
//! no hardcoded arm can exist for a symbol the suite never mentions. Every
//! identifier below was checked against the suite's 913 defined names.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── ParamSpec preserves parameter types ──────────────────────────────────
// The spec requires `Callable[PSpindle, TQuarry]` to reproduce the wrapped
// signature, so a call ill-typed against the undecorated function stays
// ill-typed after decoration. A `ParamSpec` decorator is transparent, not a
// `(*args, **kwargs)` escape hatch.

pub(super) const SPINDLE_REJECTED: &str = r#"
import collections.abc
from typing import ParamSpec as SignatureOf, TypeVar as Quantified
PSpindle = SignatureOf("PSpindle")
TQuarry = Quantified("TQuarry")
def bracing(inner: collections.abc.Callable[PSpindle, TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
brattice("shallow", "stope")
"#;

pub(super) const SPINDLE_ACCEPTED: &str = r#"
import collections.abc
from typing import ParamSpec as SignatureOf, TypeVar as Quantified
PSpindle = SignatureOf("PSpindle")
TQuarry = Quantified("TQuarry")
def bracing(inner: collections.abc.Callable[PSpindle, TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
brattice(3, "stope")
"#;

pub(super) const SPINDLE_REJECTED_ALIASED: &str = r#"
from typing import Callable as Applicable, ParamSpec as SignatureOf, TypeVar as Quantified
PSpindle = SignatureOf("PSpindle")
TQuarry = Quantified("TQuarry")
def bracing(inner: Applicable[PSpindle, TQuarry]) -> Applicable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
brattice("shallow", "stope")
"#;

pub(super) const SPINDLE_REJECTED_IMPORT_FORM: &str = r#"
import typing
PSpindle = typing.ParamSpec("PSpindle")
TQuarry = typing.TypeVar("TQuarry")
def bracing(inner: typing.Callable[PSpindle, TQuarry]) -> typing.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
brattice("shallow", "stope")
"#;

pub(super) const SPINDLE_REJECTED_RENAMED: &str = r#"
import collections.abc
from typing import ParamSpec as SignatureOf, TypeVar as Quantified
PWithy = SignatureOf("PWithy")
TMarl = Quantified("TMarl")
def swaling(outer: collections.abc.Callable[PWithy, TMarl]) -> collections.abc.Callable[PWithy, TMarl]:
    def liner(*positional: PWithy.args, **keyworded: PWithy.kwargs) -> TMarl:
        return outer(*positional, **keyworded)
    return liner
@swaling
def coppice(rung: int, spoil: str) -> bytes: return spoil.encode()
coppice("shallow", "stope")
"#;

#[test]
fn paramspec_decorator_preserves_parameter_types() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a ParamSpec decorator reproduces the wrapped signature, so `gauge: int` \
                      still rejects a `str`",
        rejected: SPINDLE_REJECTED,
        accepted: SPINDLE_ACCEPTED,
        rejected_variants: &[
            aliased(SPINDLE_REJECTED_ALIASED),
            import_form(SPINDLE_REJECTED_IMPORT_FORM),
            renamed(SPINDLE_REJECTED_RENAMED),
        ],
        accepted_variants: &[],
    }
    .assert("ParamSpec decorator parameter types")
}

// ── ParamSpec preserves keyword names ────────────────────────────────────
// `PSpindle` captures parameter names as well as types, so a keyword the
// wrapped function does not declare is an error at the decorated call site.

pub(super) const KEYWORD_REJECTED: &str = r#"
import collections.abc
def bracing[**PSpindle, TQuarry](inner: collections.abc.Callable[PSpindle, TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
brattice(gauge=3, spoil="stope")
"#;

pub(super) const KEYWORD_ACCEPTED: &str = r#"
import collections.abc
def bracing[**PSpindle, TQuarry](inner: collections.abc.Callable[PSpindle, TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
brattice(gauge=3, sigil="stope")
"#;

pub(super) const KEYWORD_REJECTED_REFORMATTED: &str = "
import collections.abc

# a decorator obliged to stay transparent
def bracing[**PSpindle, TQuarry](
        inner: collections.abc.Callable[PSpindle, TQuarry],
) -> collections.abc.Callable[PSpindle, TQuarry]:
        def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
                return inner(*args, **kwargs)

        return sheath

@bracing
def brattice(gauge: int, sigil: str) -> bytes:
        return (sigil).encode()

# the defect is one line down
brattice(gauge=3, spoil='stope')
";

#[test]
fn paramspec_decorator_preserves_keyword_names() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "ParamSpec carries parameter names, so `spoil=` names no parameter of the \
                      decorated function",
        rejected: KEYWORD_REJECTED,
        accepted: KEYWORD_ACCEPTED,
        rejected_variants: &[reformatted(KEYWORD_REJECTED_REFORMATTED)],
        accepted_variants: &[],
    }
    .assert("ParamSpec decorator keyword names")
}

// ── ParamSpec preserves the return type ──────────────────────────────────
// `TQuarry` solves to `bytes` at the decoration site, so the decorated call
// yields `bytes` — not `Any`, and not the decorator's own type.

pub(super) const RETURN_REJECTED: &str = r#"
import collections.abc
def bracing[**PSpindle, TQuarry](inner: collections.abc.Callable[PSpindle, TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
tallage: str = brattice(3, "stope")
"#;

pub(super) const RETURN_ACCEPTED: &str = r#"
import collections.abc
def bracing[**PSpindle, TQuarry](inner: collections.abc.Callable[PSpindle, TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
tallage: bytes = brattice(3, "stope")
"#;

#[test]
fn paramspec_decorator_preserves_return_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the decorator's return type variable solves to `bytes`, which is not \
                      assignable to `str`",
        rejected: RETURN_REJECTED,
        accepted: RETURN_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("ParamSpec decorator return type")
}

// ── ParamSpec preserves arity ────────────────────────────────────────────

pub(super) const ARITY_REJECTED: &str = r#"
import typing
def swaling[**PWithy, TMarl](outer: typing.Callable[PWithy, TMarl]) -> typing.Callable[PWithy, TMarl]:
    def liner(*args: PWithy.args, **kwargs: PWithy.kwargs) -> TMarl:
        return outer(*args, **kwargs)
    return liner
@swaling
def coppice(rung: int) -> str: return str(rung)
coppice(2, 5)
"#;

pub(super) const ARITY_ACCEPTED: &str = r#"
import typing
def swaling[**PWithy, TMarl](outer: typing.Callable[PWithy, TMarl]) -> typing.Callable[PWithy, TMarl]:
    def liner(*args: PWithy.args, **kwargs: PWithy.kwargs) -> TMarl:
        return outer(*args, **kwargs)
    return liner
@swaling
def coppice(rung: int) -> str: return str(rung)
coppice(2)
"#;

#[test]
fn paramspec_decorator_preserves_arity() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the decorated callable keeps the wrapped arity, so a second positional \
                      argument has no parameter to bind",
        rejected: ARITY_REJECTED,
        accepted: ARITY_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("ParamSpec decorator arity")
}

// ── `.args` and `.kwargs` are a pair ─────────────────────────────────────
// `PSpindle.args` is well-formed only on a `*args` parameter accompanied by a
// `**kwargs` parameter annotated `PSpindle.kwargs`. A lone `.args` is
// ill-formed however the surrounding code is spelled.

pub(super) const PAIR_REJECTED: &str = r#"
import collections.abc
def girth[**PSpindle](inner: collections.abc.Callable[PSpindle, bytes]) -> collections.abc.Callable[PSpindle, bytes]:
    def sheath(*args: PSpindle.args) -> bytes:
        return inner(*args)
    return sheath
"#;

pub(super) const PAIR_ACCEPTED: &str = r#"
import collections.abc
def girth[**PSpindle](inner: collections.abc.Callable[PSpindle, bytes]) -> collections.abc.Callable[PSpindle, bytes]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> bytes:
        return inner(*args, **kwargs)
    return sheath
"#;

pub(super) const PAIR_ACCEPTED_ALIASED: &str = r#"
from typing import Callable as Applicable, ParamSpec as SignatureOf
PSpindle = SignatureOf("PSpindle")
def girth(inner: Applicable[PSpindle, bytes]) -> Applicable[PSpindle, bytes]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> bytes:
        return inner(*args, **kwargs)
    return sheath
"#;
