//! Generic bounds, constraints, arity and variance.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! Type parameters are declared with PEP 695 syntax (`def f[TGrist: ...]`,
//! `class C[TRadix]`), which reaches the generics machinery without importing
//! `TypeVar` at all — the conformance suite barely contains this spelling, so a
//! rule fitted to `TypeVar(...)` call sites cannot see these programs. The one
//! legacy-spelled case appears solely as an A6 variant, under the alias
//! `QuarriedVar`, since `TypeVar` is quarantined rather than exempt.
//!
//! Every library symbol used here — `SupportsAbs`, `AbstractSet`,
//! `MutableSequence`, `Reversible` — is absent from `conformance/tests/`
//! entirely, so no hardcoded arm can exist for it. Identifiers are drawn from a
//! vocabulary disjoint from the suite's 913 names.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── Bound: the argument type must implement it ───────────────────────────
// Spec (generics, upper bounds): a type argument solved for a bounded type
// parameter must be assignable to the bound. `SupportsAbs[int]` is satisfied by
// `__abs__(self) -> int` and by nothing else; `__round__` is a different member.

pub(super) const ABS_BOUND_REJECTED: &str = r"
from typing import SupportsAbs

class Quernstone:
    def __round__(self, ndigits: int = 0) -> int:
        return 4

def slake[TGrist: SupportsAbs[int]](load: TGrist) -> TGrist:
    return load

slake(Quernstone())
";

pub(super) const ABS_BOUND_ACCEPTED: &str = r"
from typing import SupportsAbs

class Quernstone:
    def __abs__(self) -> int:
        return 4

def slake[TGrist: SupportsAbs[int]](load: TGrist) -> TGrist:
    return load

slake(Quernstone())
";

/// A6. The legacy declaration form binds the same bound to the same parameter,
/// so the verdict must not move. `TypeVar` is quarantined, hence the alias.
pub(super) const ABS_BOUND_REJECTED_LEGACY: &str = r"
from typing import SupportsAbs, TypeVar as QuarriedVar

TGrist = QuarriedVar('TGrist', bound=SupportsAbs[int])

class Quernstone:
    def __round__(self, ndigits: int = 0) -> int:
        return 4

def slake(load: TGrist) -> TGrist:
    return load

slake(Quernstone())
";

pub(super) const ABS_BOUND_REJECTED_IMPORT_FORM: &str = r"
import typing

class Quernstone:
    def __round__(self, ndigits: int = 0) -> int:
        return 4

def slake[TGrist: typing.SupportsAbs[int]](load: TGrist) -> TGrist:
    return load

slake(Quernstone())
";

pub(super) const ABS_BOUND_REJECTED_REFORMATTED: &str = r"
from typing import SupportsAbs
class Quernstone:  # carries a rounding member, not an absolute one

        def __round__(self, ndigits: int = 0) -> int:

                return 4
def slake[TGrist: SupportsAbs[int]](
        load: TGrist,
) -> TGrist:
        return (load)
# the violation, one line down
slake(Quernstone())
";

pub(super) const ABS_BOUND_ACCEPTED_ALIASED: &str = r"
from typing import SupportsAbs as HasMagnitude

class Quernstone:
    def __abs__(self) -> int:
        return 4

def slake[TGrist: HasMagnitude[int]](load: TGrist) -> TGrist:
    return load

slake(Quernstone())
";

#[test]
fn upper_bound_rejects_a_type_missing_the_bound_member(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a type argument for a bounded parameter must be assignable to the bound, \
                      and `__round__` does not satisfy `SupportsAbs[int]`",
        rejected: ABS_BOUND_REJECTED,
        accepted: ABS_BOUND_ACCEPTED,
        rejected_variants: &[
            aliased(ABS_BOUND_REJECTED_LEGACY),
            import_form(ABS_BOUND_REJECTED_IMPORT_FORM),
            reformatted(ABS_BOUND_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[aliased(ABS_BOUND_ACCEPTED_ALIASED)],
    }
    .assert("SupportsAbs upper bound")
}

// ── Bound: a satisfying argument must be accepted ────────────────────────
// The false-positive side of the same clause. `frozenset[str]` is an
// `AbstractSet[str]`; `list[str]` is not, whatever else it can do.

pub(super) const SET_BOUND_REJECTED: &str = r#"
from typing import AbstractSet

def winnow[TQuarry: AbstractSet[str]](husks: TQuarry) -> TQuarry:
    return husks

winnow(["barley"])
"#;

pub(super) const SET_BOUND_ACCEPTED: &str = r#"
from typing import AbstractSet

def winnow[TQuarry: AbstractSet[str]](husks: TQuarry) -> TQuarry:
    return husks

winnow(frozenset({"barley"}))
"#;

pub(super) const SET_BOUND_ACCEPTED_ALIASED: &str = r#"
from typing import AbstractSet as UnorderedOf

def winnow[TQuarry: UnorderedOf[str]](husks: TQuarry) -> TQuarry:
    return husks

winnow(frozenset({"barley"}))
"#;

pub(super) const SET_BOUND_ACCEPTED_IMPORT_FORM: &str = r#"
import typing

def winnow[TQuarry: typing.AbstractSet[str]](husks: TQuarry) -> TQuarry:
    return husks

winnow(frozenset({"barley"}))
"#;

#[test]
fn upper_bound_accepts_a_conforming_argument() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`frozenset[str]` is an `AbstractSet[str]` and `list[str]` is not",
        rejected: SET_BOUND_REJECTED,
        accepted: SET_BOUND_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[
            aliased(SET_BOUND_ACCEPTED_ALIASED),
            import_form(SET_BOUND_ACCEPTED_IMPORT_FORM),
        ],
    }
    .assert("AbstractSet upper bound")
}

// ── Bound on a class type parameter, checked at explicit specialisation ──
// Spec: the bound constrains every explicit type argument too, not only the
// ones inferred at a call site.

pub(super) const CLASS_BOUND_REJECTED: &str = r"
from typing import MutableSequence

class Kilnload[TEmber: MutableSequence[int]]:
    def __init__(self, embers: TEmber) -> None:
        self.embers = embers

def fire(load: Kilnload[str]) -> int:
    return len(load.embers)
";

pub(super) const CLASS_BOUND_ACCEPTED: &str = r"
from typing import MutableSequence

class Kilnload[TEmber: MutableSequence[int]]:
    def __init__(self, embers: TEmber) -> None:
        self.embers = embers

def fire(load: Kilnload[list[int]]) -> int:
    return len(load.embers)
";

pub(super) const CLASS_BOUND_REJECTED_ALIASED: &str = r"
from typing import MutableSequence as GrowableRow

class Kilnload[TEmber: GrowableRow[int]]:
    def __init__(self, embers: TEmber) -> None:
        self.embers = embers

def fire(load: Kilnload[str]) -> int:
    return len(load.embers)
";

pub(super) const CLASS_BOUND_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc

class Kilnload[TEmber: collections.abc.MutableSequence[int]]:
    def __init__(self, embers: TEmber) -> None:
        self.embers = embers

def fire(load: Kilnload[list[int]]) -> int:
    return len(load.embers)
";

#[test]
fn class_bound_binds_explicit_type_arguments() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`str` is not a `MutableSequence[int]`, so it cannot specialise a \
                      parameter bounded by it",
        rejected: CLASS_BOUND_REJECTED,
        accepted: CLASS_BOUND_ACCEPTED,
        rejected_variants: &[aliased(CLASS_BOUND_REJECTED_ALIASED)],
        accepted_variants: &[import_form(CLASS_BOUND_ACCEPTED_IMPORT_FORM)],
    }
    .assert("class-scoped upper bound")
}

// ── A bound may not reference a sibling type parameter ───────────────────
// Spec (PEP 695 scoping): the bound of a type parameter is evaluated in a scope
// that does not contain the other parameters of the same list, so naming one is
// invalid however it is spelled.

pub(super) const SIBLING_BOUND_REJECTED: &str = r"
def plumb[TWithy, TBollard: TWithy](
    stave: TWithy, hoop: TBollard
) -> tuple[TWithy, TBollard]:
    return (stave, hoop)
";

pub(super) const SIBLING_BOUND_ACCEPTED: &str = r"
from typing import Reversible

def plumb[TWithy, TBollard: Reversible[str]](
    stave: TWithy, hoop: TBollard
) -> tuple[TWithy, TBollard]:
    return (stave, hoop)
";

pub(super) const SIBLING_BOUND_REJECTED_RENAMED: &str = r"
def careen[TPennon, TGirth: TPennon](
    shaft: TPennon, collar: TGirth
) -> tuple[TPennon, TGirth]:
    return (shaft, collar)
";

pub(super) const SIBLING_BOUND_REJECTED_REFORMATTED: &str = r"
# the bound below names a parameter of its own list
def plumb[TWithy, TBollard: TWithy](stave: TWithy, hoop: TBollard) -> tuple[TWithy, TBollard]:
        return ((stave, hoop))
";

pub(super) const SIBLING_BOUND_ACCEPTED_ALIASED: &str = r"
from typing import Reversible as CanRunBackwards

def plumb[TWithy, TBollard: CanRunBackwards[str]](
    stave: TWithy, hoop: TBollard
) -> tuple[TWithy, TBollard]:
    return (stave, hoop)
";

#[test]
fn bound_may_not_name_a_sibling_type_parameter() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a type parameter's bound is evaluated outside the scope of the other \
                      parameters in the same list, so `TBollard: TWithy` is invalid",
        rejected: SIBLING_BOUND_REJECTED,
        accepted: SIBLING_BOUND_ACCEPTED,
        rejected_variants: &[
            renamed(SIBLING_BOUND_REJECTED_RENAMED),
            reformatted(SIBLING_BOUND_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[aliased(SIBLING_BOUND_ACCEPTED_ALIASED)],
    }
    .assert("bound referencing a sibling parameter")
}

// ── Constraints: only the listed types, never an arbitrary other ─────────
// Spec: a constrained type parameter is solved to one of its listed
// constraints. `str` is neither `bytes` nor `bool`, so no solution exists.

pub(super) const CONSTRAINT_REJECTED: &str = r#"
def swage[TGirth: (bytes, bool)](fitting: TGirth) -> TGirth:
    return fitting

swage("cordage")
"#;

pub(super) const CONSTRAINT_ACCEPTED: &str = r#"
def swage[TGirth: (bytes, bool)](fitting: TGirth) -> TGirth:
    return fitting

swage(b"cordage")
"#;

pub(super) const CONSTRAINT_REJECTED_RENAMED: &str = r#"
def bevel[TSpindle: (bytes, bool)](collar: TSpindle) -> TSpindle:
    return collar

bevel("cordage")
"#;

pub(super) const CONSTRAINT_REJECTED_REFORMATTED: &str = "
def swage[
        TGirth: (bytes, bool),
](fitting: TGirth) -> TGirth:  # solved to exactly one constraint
        return fitting

swage('cordage')
";
