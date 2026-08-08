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
#[allow(
    clippy::wildcard_imports,
    unused_imports,
    reason = "shared golden fixtures: each sibling uses the subset it references"
)]
use super::narrowing::*;

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
