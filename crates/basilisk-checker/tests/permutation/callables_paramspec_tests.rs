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

const SPINDLE_REJECTED: &str = r#"
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

const SPINDLE_ACCEPTED: &str = r#"
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

const SPINDLE_REJECTED_ALIASED: &str = r#"
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

const SPINDLE_REJECTED_IMPORT_FORM: &str = r#"
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

const SPINDLE_REJECTED_RENAMED: &str = r#"
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

const KEYWORD_REJECTED: &str = r#"
import collections.abc
def bracing[**PSpindle, TQuarry](inner: collections.abc.Callable[PSpindle, TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
brattice(gauge=3, spoil="stope")
"#;

const KEYWORD_ACCEPTED: &str = r#"
import collections.abc
def bracing[**PSpindle, TQuarry](inner: collections.abc.Callable[PSpindle, TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
brattice(gauge=3, sigil="stope")
"#;

const KEYWORD_REJECTED_REFORMATTED: &str = "
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

const RETURN_REJECTED: &str = r#"
import collections.abc
def bracing[**PSpindle, TQuarry](inner: collections.abc.Callable[PSpindle, TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner(*args, **kwargs)
    return sheath
@bracing
def brattice(gauge: int, sigil: str) -> bytes: return sigil.encode()
tallage: str = brattice(3, "stope")
"#;

const RETURN_ACCEPTED: &str = r#"
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

const ARITY_REJECTED: &str = r#"
import collections.abc
def swaling[**PWithy, TMarl](outer: collections.abc.Callable[PWithy, TMarl]) -> collections.abc.Callable[PWithy, TMarl]:
    def liner(*args: PWithy.args, **kwargs: PWithy.kwargs) -> TMarl:
        return outer(*args, **kwargs)
    return liner
@swaling
def coppice(rung: int) -> str: return str(rung)
coppice(2, 5)
"#;

const ARITY_ACCEPTED: &str = r#"
import collections.abc
def swaling[**PWithy, TMarl](outer: collections.abc.Callable[PWithy, TMarl]) -> collections.abc.Callable[PWithy, TMarl]:
    def liner(*args: PWithy.args, **kwargs: PWithy.kwargs) -> TMarl:
        return outer(*args, **kwargs)
    return liner
@swaling
def coppice(rung: int) -> str: return str(rung)
coppice(2)
"#;

const ARITY_REJECTED_IMPORT_FORM: &str = r#"
import typing
def swaling[**PWithy, TMarl](outer: typing.Callable[PWithy, TMarl]) -> typing.Callable[PWithy, TMarl]:
    def liner(*args: PWithy.args, **kwargs: PWithy.kwargs) -> TMarl:
        return outer(*args, **kwargs)
    return liner
@swaling
def coppice(rung: int) -> str: return str(rung)
coppice(2, 5)
"#;

#[test]
fn paramspec_decorator_preserves_arity() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the decorated callable keeps the wrapped arity, so a second positional \
                      argument has no parameter to bind",
        rejected: ARITY_REJECTED,
        accepted: ARITY_ACCEPTED,
        rejected_variants: &[import_form(ARITY_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("ParamSpec decorator arity")
}

// ── `.args` and `.kwargs` are a pair ─────────────────────────────────────
// `PSpindle.args` is well-formed only on a `*args` parameter accompanied by a
// `**kwargs` parameter annotated `PSpindle.kwargs`. A lone `.args` is
// ill-formed however the surrounding code is spelled.

const PAIR_REJECTED: &str = r#"
import collections.abc
def girth[**PSpindle](inner: collections.abc.Callable[PSpindle, bytes]) -> collections.abc.Callable[PSpindle, bytes]:
    def sheath(*args: PSpindle.args) -> bytes:
        return inner(*args)
    return sheath
"#;

const PAIR_ACCEPTED: &str = r#"
import collections.abc
def girth[**PSpindle](inner: collections.abc.Callable[PSpindle, bytes]) -> collections.abc.Callable[PSpindle, bytes]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> bytes:
        return inner(*args, **kwargs)
    return sheath
"#;

const PAIR_ACCEPTED_ALIASED: &str = r#"
from typing import Callable as Applicable, ParamSpec as SignatureOf
PSpindle = SignatureOf("PSpindle")
def girth(inner: Applicable[PSpindle, bytes]) -> Applicable[PSpindle, bytes]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> bytes:
        return inner(*args, **kwargs)
    return sheath
"#;

#[test]
fn paramspec_args_requires_matching_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`PSpindle.args` is well-formed only alongside a `**kwargs` parameter \
                      annotated `PSpindle.kwargs`",
        rejected: PAIR_REJECTED,
        accepted: PAIR_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[aliased(PAIR_ACCEPTED_ALIASED)],
    }
    .assert("ParamSpec args/kwargs pairing")
}

// ── `.args` and `.kwargs` may not be exchanged ───────────────────────────
// `ParamSpecArgs` belongs on `*args` and `ParamSpecKwargs` on `**kwargs`;
// swapping them is ill-typed even though both members exist.

const SWAP_REJECTED: &str = r#"
import collections.abc
from typing import ParamSpecArgs, ParamSpecKwargs
def docket(banner: ParamSpecArgs | ParamSpecKwargs) -> str: return repr(banner)
def girth[**PSpindle](inner: collections.abc.Callable[PSpindle, bytes]) -> collections.abc.Callable[PSpindle, bytes]:
    def sheath(*args: PSpindle.kwargs, **kwargs: PSpindle.args) -> bytes:
        return inner(*args, **kwargs)
    return sheath
"#;

const SWAP_ACCEPTED: &str = r#"
import collections.abc
from typing import ParamSpecArgs, ParamSpecKwargs
def docket(banner: ParamSpecArgs | ParamSpecKwargs) -> str: return repr(banner)
def girth[**PSpindle](inner: collections.abc.Callable[PSpindle, bytes]) -> collections.abc.Callable[PSpindle, bytes]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> bytes:
        return inner(*args, **kwargs)
    return sheath
"#;

#[test]
fn paramspec_args_and_kwargs_may_not_be_exchanged() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`*args` must carry `.args` and `**kwargs` must carry `.kwargs`; the two \
                      marker types are not interchangeable",
        rejected: SWAP_REJECTED,
        accepted: SWAP_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("ParamSpecArgs/ParamSpecKwargs positions")
}

// ── Concatenate consumes the leading parameter ───────────────────────────
// `Callable[Concatenate[str, PSpindle], TQuarry]` is a callable whose first
// positional parameter is `str`, followed by `PSpindle`. A decorator that
// supplies that argument returns `Callable[PSpindle, TQuarry]`, so the
// decorated function is one positional parameter shorter.

const CONCAT_REJECTED: &str = r#"
import collections.abc
from typing import Concatenate as Prepend
def sluicing[**PSpindle, TQuarry](inner: collections.abc.Callable[Prepend[str, PSpindle], TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner("headrace", *args, **kwargs)
    return sheath
@sluicing
def culvert(sluiceway: str, gauge: int) -> int: return len(sluiceway) + gauge
culvert("headrace", 4)
"#;

const CONCAT_ACCEPTED: &str = r#"
import collections.abc
from typing import Concatenate as Prepend
def sluicing[**PSpindle, TQuarry](inner: collections.abc.Callable[Prepend[str, PSpindle], TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner("headrace", *args, **kwargs)
    return sheath
@sluicing
def culvert(sluiceway: str, gauge: int) -> int: return len(sluiceway) + gauge
culvert(4)
"#;

const CONCAT_REJECTED_IMPORT_FORM: &str = r#"
import collections.abc
import typing
def sluicing[**PSpindle, TQuarry](inner: collections.abc.Callable[typing.Concatenate[str, PSpindle], TQuarry]) -> collections.abc.Callable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner("headrace", *args, **kwargs)
    return sheath
@sluicing
def culvert(sluiceway: str, gauge: int) -> int: return len(sluiceway) + gauge
culvert("headrace", 4)
"#;

const CONCAT_ACCEPTED_ALIASED: &str = r#"
from typing import Callable as Applicable, Concatenate as Prepend, ParamSpec as SignatureOf, TypeVar as Quantified
PSpindle = SignatureOf("PSpindle")
TQuarry = Quantified("TQuarry")
def sluicing(inner: Applicable[Prepend[str, PSpindle], TQuarry]) -> Applicable[PSpindle, TQuarry]:
    def sheath(*args: PSpindle.args, **kwargs: PSpindle.kwargs) -> TQuarry:
        return inner("headrace", *args, **kwargs)
    return sheath
@sluicing
def culvert(sluiceway: str, gauge: int) -> int: return len(sluiceway) + gauge
culvert(4)
"#;

#[test]
fn concatenate_consumes_the_leading_parameter() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Concatenate[str, PSpindle]`'s leading argument is supplied by the \
                      decorator, so the decorated callable no longer accepts it",
        rejected: CONCAT_REJECTED,
        accepted: CONCAT_ACCEPTED,
        rejected_variants: &[import_form(CONCAT_REJECTED_IMPORT_FORM)],
        accepted_variants: &[aliased(CONCAT_ACCEPTED_ALIASED)],
    }
    .assert("Concatenate leading parameter")
}

// ── Callable parameters are contravariant ────────────────────────────────
// `Callable[[Wapentake], int]` accepts a callback whose parameter is wider
// than `Wapentake` and rejects one that is narrower: a callback demanding a
// `Bailiwick` cannot be handed an arbitrary `Wapentake`.

const CONTRA_REJECTED: &str = r#"
import collections.abc
class Wapentake: pass
class Bailiwick(Wapentake): pass
def levying(unit: Wapentake) -> int: return 1
def tithing(unit: Bailiwick) -> int: return 2
def keelhaul(step: collections.abc.Callable[[Wapentake], int]) -> int: return step(Wapentake())
def whittle(step: collections.abc.Callable[[Bailiwick], int]) -> int: return step(Bailiwick())
keelhaul(levying)
whittle(levying)
whittle(tithing)
keelhaul(tithing)
"#;

const CONTRA_ACCEPTED: &str = r#"
import collections.abc
class Wapentake: pass
class Bailiwick(Wapentake): pass
def levying(unit: Wapentake) -> int: return 1
def tithing(unit: Bailiwick) -> int: return 2
def keelhaul(step: collections.abc.Callable[[Wapentake], int]) -> int: return step(Wapentake())
def whittle(step: collections.abc.Callable[[Bailiwick], int]) -> int: return step(Bailiwick())
keelhaul(levying)
whittle(levying)
whittle(tithing)
"#;

const CONTRA_ACCEPTED_ALIASED: &str = r#"
from typing import Callable as Applicable
class Wapentake: pass
class Bailiwick(Wapentake): pass
def levying(unit: Wapentake) -> int: return 1
def tithing(unit: Bailiwick) -> int: return 2
def keelhaul(step: Applicable[[Wapentake], int]) -> int: return step(Wapentake())
def whittle(step: Applicable[[Bailiwick], int]) -> int: return step(Bailiwick())
keelhaul(levying)
whittle(levying)
whittle(tithing)
"#;

#[test]
fn callable_parameters_are_contravariant() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "callable parameters are contravariant, so a callback requiring `Bailiwick` \
                      is not assignable where `Callable[[Wapentake], int]` is expected",
        rejected: CONTRA_REJECTED,
        accepted: CONTRA_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[aliased(CONTRA_ACCEPTED_ALIASED)],
    }
    .assert("Callable parameter contravariance")
}

// ── Callable returns are covariant ───────────────────────────────────────

const COVAR_REJECTED: &str = r#"
import collections.abc
class Wapentake:
    def levy(self) -> int: return 1
class Bailiwick(Wapentake): pass
def casting(rung: int) -> Bailiwick: return Bailiwick()
def forging(rung: int) -> object: return object()
def stanchion(maker: collections.abc.Callable[[int], Wapentake]) -> int: return maker(0).levy()
stanchion(casting)
stanchion(forging)
"#;

const COVAR_ACCEPTED: &str = r#"
import collections.abc
class Wapentake:
    def levy(self) -> int: return 1
class Bailiwick(Wapentake): pass
def casting(rung: int) -> Bailiwick: return Bailiwick()
def forging(rung: int) -> object: return object()
def stanchion(maker: collections.abc.Callable[[int], Wapentake]) -> int: return maker(0).levy()
stanchion(casting)
"#;

#[test]
fn callable_returns_are_covariant() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "callable returns are covariant, so a callback returning `object` is not \
                      assignable where `Wapentake` is promised",
        rejected: COVAR_REJECTED,
        accepted: COVAR_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("Callable return covariance")
}

// ── A callback's arity must match the declared parameter list ────────────

const CALLBACK_REJECTED: &str = r#"
import collections.abc
def gimbal(step: collections.abc.Callable[[int], str]) -> str: return step(0)
def lathe(rung: int, spoil: str) -> str: return spoil * rung
gimbal(lathe)
"#;

const CALLBACK_ACCEPTED: &str = r#"
import collections.abc
def gimbal(step: collections.abc.Callable[[int], str]) -> str: return step(0)
def lathe(rung: int) -> str: return "spoil" * rung
gimbal(lathe)
"#;

const CALLBACK_REJECTED_RENAMED: &str = r#"
import collections.abc
def corbel(tread: collections.abc.Callable[[int], str]) -> str: return tread(0)
def muntin(sigil: int, banner: str) -> str: return banner * sigil
corbel(muntin)
"#;

const CALLBACK_ACCEPTED_ALIASED: &str = r#"
from typing import Callable as Applicable
def gimbal(step: Applicable[[int], str]) -> str: return step(0)
def lathe(rung: int) -> str: return "spoil" * rung
gimbal(lathe)
"#;

#[test]
fn callback_arity_must_match_the_declared_signature() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Callable[[int], str]` supplies exactly one argument, so a two-parameter \
                      callback cannot be called through it",
        rejected: CALLBACK_REJECTED,
        accepted: CALLBACK_ACCEPTED,
        rejected_variants: &[renamed(CALLBACK_REJECTED_RENAMED)],
        accepted_variants: &[aliased(CALLBACK_ACCEPTED_ALIASED)],
    }
    .assert("Callback arity")
}
