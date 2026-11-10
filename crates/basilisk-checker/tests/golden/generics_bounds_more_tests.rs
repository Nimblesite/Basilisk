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

#[allow(
    clippy::wildcard_imports,
    unused_imports,
    reason = "shared golden fixtures: each sibling uses the subset it references"
)]
use super::generics_bounds::*;
use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

#[test]
fn constrained_parameter_rejects_an_unlisted_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a constrained type parameter solves to one of its listed constraints, \
                      and `str` is neither `bytes` nor `bool`",
        rejected: CONSTRAINT_REJECTED,
        accepted: CONSTRAINT_ACCEPTED,
        rejected_variants: &[
            renamed(CONSTRAINT_REJECTED_RENAMED),
            reformatted(CONSTRAINT_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("constrained parameter, unlisted type")
}

// ── Constraints: no solving to a union of two constraints ────────────────
// Spec: two uses of the same constrained parameter must agree on a single
// constraint. A `list[int]` and a `set[int]` select different ones, and their
// union is not a permitted solution.

const UNION_SOLVE_REJECTED: &str = r"
from typing import AbstractSet, MutableSequence

def truss[TGirth: (MutableSequence[int], AbstractSet[int])](
    fore: TGirth, aft: TGirth
) -> TGirth:
    return fore

truss([1, 2], {3, 4})
";

const UNION_SOLVE_ACCEPTED: &str = r"
from typing import AbstractSet, MutableSequence

def truss[TGirth: (MutableSequence[int], AbstractSet[int])](
    fore: TGirth, aft: TGirth
) -> TGirth:
    return fore

truss([1, 2], [3, 4])
";

const UNION_SOLVE_REJECTED_ALIASED: &str = r"
from typing import AbstractSet as UnorderedOf, MutableSequence as GrowableRow

def truss[TGirth: (GrowableRow[int], UnorderedOf[int])](
    fore: TGirth, aft: TGirth
) -> TGirth:
    return fore

truss([1, 2], {3, 4})
";

const UNION_SOLVE_ACCEPTED_IMPORT_FORM: &str = r"
import typing

def truss[TGirth: (typing.MutableSequence[int], typing.AbstractSet[int])](
    fore: TGirth, aft: TGirth
) -> TGirth:
    return fore

truss([1, 2], [3, 4])
";

#[test]
fn constrained_parameter_cannot_solve_to_a_union() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "both occurrences of a constrained parameter must select the same \
                      constraint; a sequence and a set do not",
        rejected: UNION_SOLVE_REJECTED,
        accepted: UNION_SOLVE_ACCEPTED,
        rejected_variants: &[aliased(UNION_SOLVE_REJECTED_ALIASED)],
        accepted_variants: &[import_form(UNION_SOLVE_ACCEPTED_IMPORT_FORM)],
    }
    .assert("constrained parameter, union solution")
}

// ── Variance: a supertype does not satisfy a subtype's bound ─────────────
// `MutableSequence` derives from `Reversible`, so the assignability runs one
// way only. A value known merely to be `Reversible[int]` cannot be passed where
// the bound demands `MutableSequence[int]`.

const DIRECTION_REJECTED: &str = r"
from typing import MutableSequence, Reversible

def garner[TQuarry: MutableSequence[int]](rows: TQuarry) -> TQuarry:
    rows.append(0)
    return rows

def scatter(chaff: Reversible[int]) -> None:
    garner(chaff)
";

const DIRECTION_ACCEPTED: &str = r"
from typing import MutableSequence

def garner[TQuarry: MutableSequence[int]](rows: TQuarry) -> TQuarry:
    rows.append(0)
    return rows

def scatter(chaff: MutableSequence[int]) -> None:
    garner(chaff)
";

const DIRECTION_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc

def garner[TQuarry: collections.abc.MutableSequence[int]](rows: TQuarry) -> TQuarry:
    rows.append(0)
    return rows

def scatter(chaff: collections.abc.MutableSequence[int]) -> None:
    garner(chaff)
";

#[test]
fn supertype_does_not_satisfy_a_subtype_bound() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`MutableSequence[int]` derives from `Reversible[int]`, so the reverse \
                      assignment is not permitted",
        rejected: DIRECTION_REJECTED,
        accepted: DIRECTION_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[import_form(DIRECTION_ACCEPTED_IMPORT_FORM)],
    }
    .assert("bound direction")
}

// ── Variance: an invariant parameter refuses a widened argument ──────────
// `MutableSequence` is invariant, so `MutableSequence[int]` is not a
// `MutableSequence[object]`. `Reversible` is covariant, which is the repair.

const INVARIANCE_REJECTED: &str = r"
from typing import MutableSequence

def hoard(vault: MutableSequence[object]) -> int:
    return len(vault)

def stow(coins: MutableSequence[int]) -> int:
    return hoard(coins)
";

const INVARIANCE_ACCEPTED: &str = r"
from typing import MutableSequence, Reversible

def hoard(vault: Reversible[object]) -> int:
    return len(list(vault))

def stow(coins: MutableSequence[int]) -> int:
    return hoard(coins)
";

const INVARIANCE_REJECTED_ALIASED: &str = r"
from typing import MutableSequence as GrowableRow

def hoard(vault: GrowableRow[object]) -> int:
    return len(vault)

def stow(coins: GrowableRow[int]) -> int:
    return hoard(coins)
";

const INVARIANCE_ACCEPTED_RENAMED: &str = r"
from typing import MutableSequence, Reversible

def gather(bailiwick: Reversible[object]) -> int:
    return len(list(bailiwick))

def lodge(pennies: MutableSequence[int]) -> int:
    return gather(pennies)
";

#[test]
fn invariant_parameter_refuses_a_widened_argument() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`MutableSequence` is invariant, so `MutableSequence[int]` is not a \
                      `MutableSequence[object]`, while covariant `Reversible` accepts it",
        rejected: INVARIANCE_REJECTED,
        accepted: INVARIANCE_ACCEPTED,
        rejected_variants: &[aliased(INVARIANCE_REJECTED_ALIASED)],
        accepted_variants: &[renamed(INVARIANCE_ACCEPTED_RENAMED)],
    }
    .assert("invariant parameter")
}

// ── Arity: a generic class takes exactly its declared parameter count ────

const ARITY_REJECTED: &str = r"
class Wapentake[TRadix]:
    def hold(self, spoil: TRadix) -> TRadix:
        return spoil

def enrol(shire: Wapentake[int, str]) -> None:
    return None
";

const ARITY_ACCEPTED: &str = r"
class Wapentake[TRadix]:
    def hold(self, spoil: TRadix) -> TRadix:
        return spoil

def enrol(shire: Wapentake[int]) -> None:
    return None
";

const ARITY_REJECTED_RENAMED: &str = r"
class Bailiwick[TEmber]:
    def keep(self, plunder: TEmber) -> TEmber:
        return plunder

def muster_in(hundred: Bailiwick[int, str]) -> None:
    return None
";

const ARITY_REJECTED_REFORMATTED: &str = r"
class Wapentake[TRadix]:

        def hold(self, spoil: TRadix) -> TRadix:
                return spoil
def enrol(
        shire: Wapentake[
                int,
                str,
        ],
) -> None:  # two arguments for one parameter
        return None
";

#[test]
fn generic_class_rejects_a_surplus_type_argument() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a class declaring one type parameter must be specialised with exactly \
                      one type argument",
        rejected: ARITY_REJECTED,
        accepted: ARITY_ACCEPTED,
        rejected_variants: &[
            renamed(ARITY_REJECTED_RENAMED),
            reformatted(ARITY_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("generic class arity")
}

// ── Arity: the same rule applies in a base-class list ────────────────────

const BASE_ARITY_REJECTED: &str = r"
class Sluicegate[TGrist]:
    def drain(self, load: TGrist) -> TGrist:
        return load

class Millpond(Sluicegate[int, bytes]):
    pass
";

const BASE_ARITY_ACCEPTED: &str = r"
class Sluicegate[TGrist]:
    def drain(self, load: TGrist) -> TGrist:
        return load

class Millpond(Sluicegate[int]):
    pass
";

const BASE_ARITY_REJECTED_RENAMED: &str = r"
class Trebuchet[TPennon]:
    def loose(self, shot: TPennon) -> TPennon:
        return shot

class Cordwain(Trebuchet[int, bytes]):
    pass
";

#[test]
fn generic_base_class_arity_is_checked() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "specialising a base class obeys the same arity rule as any other use \
                      of the generic class",
        rejected: BASE_ARITY_REJECTED,
        accepted: BASE_ARITY_ACCEPTED,
        rejected_variants: &[renamed(BASE_ARITY_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("generic base-class arity")
}
