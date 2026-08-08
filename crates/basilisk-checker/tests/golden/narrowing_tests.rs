//! Type narrowing — `TypeIs`, `TypeGuard`, `isinstance`, `match`.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! Narrowing is decided from the *resolved* type of the subject and the
//! *resolved* identity of the narrowing form, never from how either is spelled.
//! `TypeIs`, `TypeGuard` and `assert_never` are inside the 55 typing symbols
//! `conformance/tests/` imports, so they are **quarantined**: every appearance
//! below is under an alias (`TypeIs as NarrowsTo`) or an alternate import form
//! (`typing.TypeIs`), never bare. The types being narrowed over —
//! `MutableSequence`, `Reversible`, `collections.OrderedDict`,
//! `decimal.Decimal` — are outside that vocabulary entirely, so no rule can
//! carry a hardcoded arm for them. Identifiers are drawn from a namespace
//! disjoint from the suite's 913.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── TypeIs: the narrowed type must fit inside the parameter type ─────────
// The spec constrains `def f(x: A) -> TypeIs[B]` with `B` assignable to `A`.
// `TypeIs` narrows *both* branches by intersection and subtraction, which is
// only sound when `B` is a subtype of the declared input. `TypeGuard` carries
// no such constraint — it is a one-way assertion — so the identical signature
// is well-typed under `TypeGuard` and ill-typed under `TypeIs`.

const NARROWED_OUTSIDE_PARAMETER: &str = r"
from typing import TypeIs as NarrowsTo
from decimal import Decimal

def bears_scale(quantum: str) -> NarrowsTo[Decimal]:
    return False
";

const NARROWED_WITHIN_PARAMETER: &str = r"
from typing import TypeIs as NarrowsTo
from decimal import Decimal

def bears_scale(quantum: object) -> NarrowsTo[Decimal]:
    return isinstance(quantum, Decimal)
";

const NARROWED_OUTSIDE_PARAMETER_ALIASED: &str = r"
from typing import TypeIs as ImpliesKind
from decimal import Decimal as FixedPoint

def bears_scale(quantum: str) -> ImpliesKind[FixedPoint]:
    return False
";

const NARROWED_OUTSIDE_PARAMETER_IMPORT_FORM: &str = r"
import decimal
import typing

def bears_scale(quantum: str) -> typing.TypeIs[decimal.Decimal]:
    return False
";

const NARROWED_OUTSIDE_PARAMETER_RENAMED: &str = r"
from typing import TypeIs as NarrowsTo
from decimal import Decimal

def holds_precision(magnitude: str) -> NarrowsTo[Decimal]:
    return False
";

const NARROWED_OUTSIDE_PARAMETER_REFORMATTED: &str = r"
from typing import TypeIs as NarrowsTo

from decimal import Decimal


# the narrowed type escapes the declared parameter type
def bears_scale(
        quantum: str,
) -> NarrowsTo[
        Decimal
]:
        return (False)
";

const NARROWED_WITHIN_PARAMETER_ALIASED: &str = r"
from typing import TypeIs as ImpliesKind
from decimal import Decimal as FixedPoint

def bears_scale(quantum: object) -> ImpliesKind[FixedPoint]:
    return isinstance(quantum, FixedPoint)
";

#[test]
fn type_is_return_type_must_fit_inside_its_parameter() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `TypeIs[B]` function whose first parameter is `A` requires `B` \
                      assignable to `A`; `Decimal` is not assignable to `str`",
        rejected: NARROWED_OUTSIDE_PARAMETER,
        accepted: NARROWED_WITHIN_PARAMETER,
        rejected_variants: &[
            aliased(NARROWED_OUTSIDE_PARAMETER_ALIASED),
            import_form(NARROWED_OUTSIDE_PARAMETER_IMPORT_FORM),
            renamed(NARROWED_OUTSIDE_PARAMETER_RENAMED),
            reformatted(NARROWED_OUTSIDE_PARAMETER_REFORMATTED),
        ],
        accepted_variants: &[aliased(NARROWED_WITHIN_PARAMETER_ALIASED)],
    }
    .assert("TypeIs return type within parameter type")
}

const ASSERTED_OUTSIDE_PARAMETER: &str = r"
from typing import TypeGuard as AssertsKind
from decimal import Decimal

def bears_scale(quantum: str) -> AssertsKind[Decimal]:
    return False
";

const ASSERTED_OUTSIDE_PARAMETER_IMPORT_FORM: &str = r"
import decimal
import typing

def bears_scale(quantum: str) -> typing.TypeGuard[decimal.Decimal]:
    return False
";

#[test]
fn type_guard_carries_no_assignability_restriction() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`TypeGuard` asserts one direction only, so its narrowed type need not \
                      be assignable to the parameter type — the same signature that `TypeIs` \
                      rejects is legal here",
        rejected: NARROWED_OUTSIDE_PARAMETER,
        accepted: ASSERTED_OUTSIDE_PARAMETER,
        rejected_variants: &[],
        accepted_variants: &[import_form(ASSERTED_OUTSIDE_PARAMETER_IMPORT_FORM)],
    }
    .assert("TypeGuard has no assignability restriction")
}

// ── TypeIs narrows both branches; TypeGuard narrows only the positive one ──
// For `quantum: Decimal | str` guarded by a `TypeIs[Decimal]`, the negative
// branch is `Decimal | str` minus `Decimal`, i.e. `str`. Under a
// `TypeGuard[Decimal]` the negative branch is left at `Decimal | str`.

const TYPE_IS_ELSE_TREATED_AS_DECIMAL: &str = r"
from typing import TypeIs as NarrowsTo
from decimal import Decimal

def bears_scale(quantum: Decimal | str) -> NarrowsTo[Decimal]:
    return isinstance(quantum, Decimal)

def transcribe(quantum: Decimal | str) -> str:
    if bears_scale(quantum):
        return quantum.to_eng_string()
    return quantum.to_eng_string()
";

const TYPE_IS_ELSE_TREATED_AS_STR: &str = r"
from typing import TypeIs as NarrowsTo
from decimal import Decimal

def bears_scale(quantum: Decimal | str) -> NarrowsTo[Decimal]:
    return isinstance(quantum, Decimal)

def transcribe(quantum: Decimal | str) -> str:
    if bears_scale(quantum):
        return quantum.to_eng_string()
    return quantum.casefold()
";

const TYPE_IS_ELSE_TREATED_AS_STR_ALIASED: &str = r"
from typing import TypeIs as ImpliesKind
from decimal import Decimal as FixedPoint

def bears_scale(quantum: FixedPoint | str) -> ImpliesKind[FixedPoint]:
    return isinstance(quantum, FixedPoint)

def transcribe(quantum: FixedPoint | str) -> str:
    if bears_scale(quantum):
        return quantum.to_eng_string()
    return quantum.casefold()
";

#[test]
fn type_is_narrows_the_negative_branch() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`TypeIs[Decimal]` subtracts `Decimal` from the negative branch, leaving \
                      `str`, which has `casefold` and not `to_eng_string`",
        rejected: TYPE_IS_ELSE_TREATED_AS_DECIMAL,
        accepted: TYPE_IS_ELSE_TREATED_AS_STR,
        rejected_variants: &[],
        accepted_variants: &[aliased(TYPE_IS_ELSE_TREATED_AS_STR_ALIASED)],
    }
    .assert("TypeIs negative-branch narrowing")
}

const TYPE_GUARD_ELSE_ASSUMED_NARROWED: &str = r"
from typing import TypeGuard as AssertsKind
from decimal import Decimal

def looks_scaled(quantum: Decimal | str) -> AssertsKind[Decimal]:
    return isinstance(quantum, Decimal)

def transcribe(quantum: Decimal | str) -> str:
    if looks_scaled(quantum):
        return quantum.to_eng_string()
    return quantum.casefold()
";

const TYPE_GUARD_ELSE_RECHECKED: &str = r"
from typing import TypeGuard as AssertsKind
from decimal import Decimal

def looks_scaled(quantum: Decimal | str) -> AssertsKind[Decimal]:
    return isinstance(quantum, Decimal)

def transcribe(quantum: Decimal | str) -> str:
    if looks_scaled(quantum):
        return quantum.to_eng_string()
    if isinstance(quantum, str):
        return quantum.casefold()
    return quantum.to_eng_string()
";

const TYPE_GUARD_ELSE_ASSUMED_NARROWED_IMPORT_FORM: &str = r"
import typing
from decimal import Decimal

def looks_scaled(quantum: Decimal | str) -> typing.TypeGuard[Decimal]:
    return isinstance(quantum, Decimal)

def transcribe(quantum: Decimal | str) -> str:
    if looks_scaled(quantum):
        return quantum.to_eng_string()
    return quantum.casefold()
";

#[test]
fn type_guard_leaves_the_negative_branch_unnarrowed() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`TypeGuard` narrows only positively, so the negative branch is still \
                      `Decimal | str` and `Decimal` has no `casefold`",
        rejected: TYPE_GUARD_ELSE_ASSUMED_NARROWED,
        accepted: TYPE_GUARD_ELSE_RECHECKED,
        rejected_variants: &[import_form(TYPE_GUARD_ELSE_ASSUMED_NARROWED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("TypeGuard positive-only narrowing")
}

// ── isinstance against an abstract base class ────────────────────────────
// `isinstance(x, C)` narrows to `C`, and membership is decided by `C`'s
// resolved member set. `MutableSequence` declares `append`; `Reversible`
// declares only `__reversed__`.

const APPEND_AFTER_REVERSIBLE: &str = r"
from collections.abc import Reversible

def stow(hopper: object, freight: int) -> None:
    if isinstance(hopper, Reversible):
        hopper.append(freight)
";

const APPEND_AFTER_MUTABLE_SEQUENCE: &str = r"
from collections.abc import MutableSequence

def stow(hopper: object, freight: int) -> None:
    if isinstance(hopper, MutableSequence):
        hopper.append(freight)
";

const APPEND_AFTER_MUTABLE_SEQUENCE_IMPORT_FORM: &str = r"
import collections.abc

def stow(hopper: object, freight: int) -> None:
    if isinstance(hopper, collections.abc.MutableSequence):
        hopper.append(freight)
";

#[test]
fn isinstance_mutable_sequence_admits_append() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "narrowing to `MutableSequence` supplies `append`; narrowing to \
                      `Reversible` supplies only `__reversed__`",
        rejected: APPEND_AFTER_REVERSIBLE,
        accepted: APPEND_AFTER_MUTABLE_SEQUENCE,
        rejected_variants: &[],
        accepted_variants: &[import_form(APPEND_AFTER_MUTABLE_SEQUENCE_IMPORT_FORM)],
    }
    .assert("isinstance MutableSequence append")
}

const ELSE_BRANCH_TREATED_AS_MAPPING: &str = r"
from collections import OrderedDict
from decimal import Decimal

def inscribe(dossier: OrderedDict[str, int] | Decimal) -> int:
    if isinstance(dossier, OrderedDict):
        return len(dossier)
    return len(dossier)
";

const ELSE_BRANCH_TREATED_AS_DECIMAL: &str = r"
from collections import OrderedDict
from decimal import Decimal

def inscribe(dossier: OrderedDict[str, int] | Decimal) -> int:
    if isinstance(dossier, OrderedDict):
        return len(dossier)
    return dossier.adjusted()
";

const ELSE_BRANCH_TREATED_AS_DECIMAL_RENAMED: &str = r"
from collections import OrderedDict
from decimal import Decimal

def tabulate(manifest: OrderedDict[str, int] | Decimal) -> int:
    if isinstance(manifest, OrderedDict):
        return len(manifest)
    return manifest.adjusted()
";

#[test]
fn isinstance_removes_the_matched_member_from_the_else_branch()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the negative branch of `isinstance(x, OrderedDict)` over \
                      `OrderedDict[str, int] | Decimal` is `Decimal`, which has no `__len__`",
        rejected: ELSE_BRANCH_TREATED_AS_MAPPING,
        accepted: ELSE_BRANCH_TREATED_AS_DECIMAL,
        rejected_variants: &[],
        accepted_variants: &[renamed(ELSE_BRANCH_TREATED_AS_DECIMAL_RENAMED)],
    }
    .assert("isinstance else-branch subtraction")
}

// ── match exhaustiveness ─────────────────────────────────────────────────
// Class patterns covering every member of a union narrow the subject to an
// uninhabited type at the fallthrough, so the implicit `return None` after the
// match is unreachable and cannot violate the declared `-> str`. Leave one
// member uncovered and the same function falls off the end.

const MATCH_LEAVES_A_MEMBER_UNCOVERED: &str = r#"
from collections import OrderedDict
from decimal import Decimal

def engrave(consignment: Decimal | OrderedDict[str, int] | str) -> str:
    match consignment:
        case Decimal():
            return "scaled"
        case OrderedDict():
            return "ordered"
"#;

const MATCH_COVERS_EVERY_MEMBER: &str = r#"
from collections import OrderedDict
from decimal import Decimal

def engrave(consignment: Decimal | OrderedDict[str, int]) -> str:
    match consignment:
        case Decimal():
            return "scaled"
        case OrderedDict():
            return "ordered"
"#;

const MATCH_COVERS_EVERY_MEMBER_RENAMED: &str = r#"
from collections import OrderedDict
from decimal import Decimal

def emboss(freightage: Decimal | OrderedDict[str, int]) -> str:
    match freightage:
        case Decimal():
            return "scaled"
        case OrderedDict():
            return "ordered"
"#;

#[test]
fn exhaustive_match_makes_the_fallthrough_unreachable() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "class patterns covering every union member leave the fallthrough \
                      uninhabited, so the declared `-> str` is satisfied; an uncovered \
                      member makes the implicit `return None` reachable",
        rejected: MATCH_LEAVES_A_MEMBER_UNCOVERED,
        accepted: MATCH_COVERS_EVERY_MEMBER,
        rejected_variants: &[],
        accepted_variants: &[renamed(MATCH_COVERS_EVERY_MEMBER_RENAMED)],
    }
    .assert("exhaustive match fallthrough")
}

// ── assert_never ─────────────────────────────────────────────────────────
// `assert_never(x)` requires `x` to have narrowed to an uninhabited type. A
// value that is still inhabited is an error, whichever narrowing form got it
// there — chained `isinstance` tests or a `match`.

const EXHAUSTION_CLAIMED_TOO_SOON: &str = r#"
from typing import assert_never as demand_exhausted
from collections import OrderedDict
from decimal import Decimal

def catalogue(consignment: Decimal | OrderedDict[str, int] | str) -> str:
    if isinstance(consignment, Decimal):
        return "scaled"
    if isinstance(consignment, OrderedDict):
        return "ordered"
    demand_exhausted(consignment)
"#;

const EXHAUSTION_ACTUALLY_REACHED: &str = r#"
from typing import assert_never as demand_exhausted
from collections import OrderedDict
from decimal import Decimal

def catalogue(consignment: Decimal | OrderedDict[str, int]) -> str:
    if isinstance(consignment, Decimal):
        return "scaled"
    if isinstance(consignment, OrderedDict):
        return "ordered"
    demand_exhausted(consignment)
"#;

const EXHAUSTION_ACTUALLY_REACHED_IMPORT_FORM: &str = r#"
import typing
from collections import OrderedDict
from decimal import Decimal

def catalogue(consignment: Decimal | OrderedDict[str, int]) -> str:
    if isinstance(consignment, Decimal):
        return "scaled"
    if isinstance(consignment, OrderedDict):
        return "ordered"
    typing.assert_never(consignment)
"#;

#[test]
fn assert_never_rejects_a_still_inhabited_value() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`assert_never` requires an uninhabited argument; after subtracting \
                      `Decimal` and `OrderedDict` the `str` member is still inhabited",
        rejected: EXHAUSTION_CLAIMED_TOO_SOON,
        accepted: EXHAUSTION_ACTUALLY_REACHED,
        rejected_variants: &[],
        accepted_variants: &[import_form(EXHAUSTION_ACTUALLY_REACHED_IMPORT_FORM)],
    }
    .assert("assert_never on an inhabited value")
}

const WILDCARD_STILL_INHABITED: &str = r#"
from typing import assert_never as demand_exhausted
from collections import OrderedDict
from decimal import Decimal

def emboss(consignment: Decimal | OrderedDict[str, int] | str) -> str:
    match consignment:
        case Decimal():
            return "scaled"
        case OrderedDict():
            return "ordered"
        case _:
            demand_exhausted(consignment)
"#;

const WILDCARD_UNINHABITED: &str = r#"
from typing import assert_never as demand_exhausted
from collections import OrderedDict
from decimal import Decimal

def emboss(consignment: Decimal | OrderedDict[str, int]) -> str:
    match consignment:
        case Decimal():
            return "scaled"
        case OrderedDict():
            return "ordered"
        case _:
            demand_exhausted(consignment)
"#;

const WILDCARD_UNINHABITED_ALIASED: &str = r#"
from typing import assert_never as ProveUnreachable
from collections import OrderedDict as RankedMapping
from decimal import Decimal as FixedPoint

def emboss(consignment: FixedPoint | RankedMapping[str, int]) -> str:
    match consignment:
        case FixedPoint():
            return "scaled"
        case RankedMapping():
            return "ordered"
        case _:
            ProveUnreachable(consignment)
"#;

#[test]
fn match_wildcard_after_full_coverage_is_uninhabited()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `case _` reached only after every union member matched is \
                      uninhabited, so `assert_never` there is legal — and illegal as soon \
                      as one member is left unmatched",
        rejected: WILDCARD_STILL_INHABITED,
        accepted: WILDCARD_UNINHABITED,
        rejected_variants: &[],
        accepted_variants: &[aliased(WILDCARD_UNINHABITED_ALIASED)],
    }
    .assert("match wildcard exhaustiveness")
}

// ── `is None` versus truthiness ──────────────────────────────────────────
// `x is None` partitions on identity, so the negative branch drops `None`
// exactly. `if x:` partitions on `__bool__`, which a class may define to
// return `False`; the negative branch therefore keeps both `None` and the
// class, and for a non-optional subject it stays fully inhabited.

const TRUTHINESS_ASSUMED_TO_DROP_NONE: &str = r#"
class Slipway:
    def __bool__(self) -> bool:
        return False

    def launch(self) -> str:
        return "away"

def dispatch(berth: Slipway | None) -> str:
    if berth:
        return berth.launch()
    return berth.launch()
"#;

const IDENTITY_TEST_DROPS_NONE: &str = r#"
class Slipway:
    def __bool__(self) -> bool:
        return False

    def launch(self) -> str:
        return "away"

def dispatch(berth: Slipway | None) -> str:
    if berth is None:
        return "idle"
    return berth.launch()
"#;

const IDENTITY_TEST_DROPS_NONE_RENAMED: &str = r#"
class Quernstone:
    def __bool__(self) -> bool:
        return False

    def grind(self) -> str:
        return "meal"

def withstand(hopper: Quernstone | None) -> str:
    if hopper is None:
        return "idle"
    return hopper.grind()
"#;

#[test]
fn is_none_drops_none_where_truthiness_does_not() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`if x:` cannot eliminate `None` from the negative branch, while \
                      `x is None` eliminates it exactly",
        rejected: TRUTHINESS_ASSUMED_TO_DROP_NONE,
        accepted: IDENTITY_TEST_DROPS_NONE,
        rejected_variants: &[],
        accepted_variants: &[renamed(IDENTITY_TEST_DROPS_NONE_RENAMED)],
    }
    .assert("is-None versus truthiness narrowing")
}

const FALSY_BRANCH_MISSES_A_MEMBER: &str = r#"
class Slipway:
    def __bool__(self) -> bool:
        return False

    def launch(self) -> str:
        return "away"

def dispatch(berth: Slipway) -> str:
    if berth:
        return berth.launch()
    return berth.moor()
"#;

const FALSY_BRANCH_STILL_INHABITED: &str = r#"
class Slipway:
    def __bool__(self) -> bool:
        return False

    def launch(self) -> str:
        return "away"

def dispatch(berth: Slipway) -> str:
    if berth:
        return berth.launch()
    return berth.launch()
"#;

const FALSY_BRANCH_STILL_INHABITED_REFORMATTED: &str = "
class Slipway:

        def __bool__(self) -> bool:
                # a Slipway is falsy until it is fitted out
                return False
        def launch(self) -> str:
                return 'away'


def dispatch(berth: Slipway) -> str:
        if (berth):
                return berth.launch()

        return berth.launch()
";

#[test]
fn truthiness_leaves_a_bool_defining_instance_inhabited()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a class defining `__bool__` may be falsy, so the negative branch of \
                      `if x:` is still that class — reachable, and without a `moor` member",
        rejected: FALSY_BRANCH_MISSES_A_MEMBER,
        accepted: FALSY_BRANCH_STILL_INHABITED,
        rejected_variants: &[],
        accepted_variants: &[reformatted(FALSY_BRANCH_STILL_INHABITED_REFORMATTED)],
    }
    .assert("truthiness keeps a falsy instance inhabited")
}
