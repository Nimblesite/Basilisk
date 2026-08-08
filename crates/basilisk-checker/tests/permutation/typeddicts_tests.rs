//! TypedDict totality and the `Required` / `NotRequired` / `ReadOnly`
//! qualifiers (PEP 589, PEP 655, PEP 705).
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `TypedDict`, `Required`, `NotRequired` and `ReadOnly` are all inside the 55
//! `typing` symbols `conformance/tests/` imports, so they are **quarantined,
//! not exempt**: every source below reaches them through an alias
//! (`from typing import TypedDict as MappingShape`) or through the attribute
//! form (`typing.TypedDict`), never bare. Class names, key names and locals are
//! drawn from a vocabulary disjoint from the suite's 913 identifiers, so no
//! hardcoded arm and no source-text match can recognise this file.

use super::harness::{
    aliased, analyse, assert_accepted, code_multiset, import_form, reformatted, renamed,
    SpecObligation,
};

// ── Totality: a total TypedDict demands every key ────────────────────────
// PEP 589: in a TypedDict declared without `total=False`, every declared item
// is required. A dict-literal assignment that omits one is ill-typed; one that
// adds a key the type never declared is ill-typed; a value whose type does not
// match the item's declared type is ill-typed. All three repair to the same
// well-typed program, which is why they share one `accepted` source.

const WAPENTAKE_ACCEPTED: &str = r#"
from typing import TypedDict as MappingShape
class Wapentake(MappingShape):
    hundred_court: str
    hide_count: int
sheaf: Wapentake = {"hundred_court": "Skyrack", "hide_count": 12}
"#;

const WAPENTAKE_ACCEPTED_IMPORT_FORM: &str = r#"
import typing
class Wapentake(typing.TypedDict):
    hundred_court: str
    hide_count: int
sheaf: Wapentake = {"hundred_court": "Skyrack", "hide_count": 12}
"#;

const WAPENTAKE_MISSING_REJECTED: &str = r#"
from typing import TypedDict as MappingShape
class Wapentake(MappingShape):
    hundred_court: str
    hide_count: int
sheaf: Wapentake = {"hundred_court": "Skyrack"}
"#;

const WAPENTAKE_MISSING_IMPORT_FORM: &str = r#"
import typing
class Wapentake(typing.TypedDict):
    hundred_court: str
    hide_count: int
sheaf: Wapentake = {"hundred_court": "Skyrack"}
"#;

const WAPENTAKE_MISSING_RENAMED: &str = r#"
from typing import TypedDict as KeyedShape
class Bailiwick(KeyedShape):
    moot_court: str
    croft_tally: int
gavel: Bailiwick = {"moot_court": "Skyrack"}
"#;

const WAPENTAKE_MISSING_REFORMATTED: &str = "
from typing import TypedDict as MappingShape
class Wapentake(MappingShape):  # one hundred of a shire

      hundred_court: str

      hide_count: int
sheaf: Wapentake = {
      'hundred_court': 'Skyrack',
}
";

#[test]
fn total_typeddict_rejects_a_missing_key() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "every item of a TypedDict declared without `total=False` is required",
        rejected: WAPENTAKE_MISSING_REJECTED,
        accepted: WAPENTAKE_ACCEPTED,
        rejected_variants: &[
            import_form(WAPENTAKE_MISSING_IMPORT_FORM),
            renamed(WAPENTAKE_MISSING_RENAMED),
            reformatted(WAPENTAKE_MISSING_REFORMATTED),
        ],
        accepted_variants: &[import_form(WAPENTAKE_ACCEPTED_IMPORT_FORM)],
    }
    .assert("total TypedDict, missing key")
}

const WAPENTAKE_UNKNOWN_REJECTED: &str = r#"
from typing import TypedDict as MappingShape
class Wapentake(MappingShape):
    hundred_court: str
    hide_count: int
sheaf: Wapentake = {"hundred_court": "Skyrack", "hide_count": 12, "reeve_name": "Osgood"}
"#;

const WAPENTAKE_UNKNOWN_REFORMATTED: &str = "
from typing import TypedDict as MappingShape

class Wapentake(MappingShape):
        hundred_court: str
        # the count of hides answering to the court
        hide_count: int

sheaf: Wapentake = {
    'hundred_court': 'Skyrack',
    'hide_count': 12,
    'reeve_name': 'Osgood',
}
";

#[test]
fn total_typeddict_rejects_an_undeclared_key() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a TypedDict is closed — a key it never declares may not be supplied",
        rejected: WAPENTAKE_UNKNOWN_REJECTED,
        accepted: WAPENTAKE_ACCEPTED,
        rejected_variants: &[reformatted(WAPENTAKE_UNKNOWN_REFORMATTED)],
        accepted_variants: &[],
    }
    .assert("total TypedDict, undeclared key")
}

const WAPENTAKE_WRONG_TYPE_REJECTED: &str = r#"
from typing import TypedDict as MappingShape
class Wapentake(MappingShape):
    hundred_court: str
    hide_count: int
sheaf: Wapentake = {"hundred_court": "Skyrack", "hide_count": "twelve"}
"#;

const WAPENTAKE_WRONG_TYPE_IMPORT_FORM: &str = r#"
import typing
class Wapentake(typing.TypedDict):
    hundred_court: str
    hide_count: int
sheaf: Wapentake = {"hundred_court": "Skyrack", "hide_count": "twelve"}
"#;

#[test]
fn typeddict_item_value_type_is_enforced() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`str` is not assignable to an item declared `int`",
        rejected: WAPENTAKE_WRONG_TYPE_REJECTED,
        accepted: WAPENTAKE_ACCEPTED,
        rejected_variants: &[import_form(WAPENTAKE_WRONG_TYPE_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("TypedDict item value type")
}

// ── NotRequired ──────────────────────────────────────────────────────────
// PEP 655: `NotRequired[X]` makes exactly that item omittable while its
// siblings stay required. Both halves are load-bearing — omitting the
// `NotRequired` item is legal, omitting its required sibling is not.

const COOPERAGE_REJECTED: &str = r#"
import typing
class Cooperage(typing.TypedDict):
    firkin_tally: int
    bung_diameter: typing.NotRequired[float]
cask: Cooperage = {"bung_diameter": 3.5}
"#;

const COOPERAGE_ACCEPTED: &str = r#"
import typing
class Cooperage(typing.TypedDict):
    firkin_tally: int
    bung_diameter: typing.NotRequired[float]
cask: Cooperage = {"firkin_tally": 9}
"#;

const COOPERAGE_ACCEPTED_ALIASED: &str = r#"
from typing import NotRequired as OmittableKey, TypedDict as MappingShape
class Cooperage(MappingShape):
    firkin_tally: int
    bung_diameter: OmittableKey[float]
cask: Cooperage = {"firkin_tally": 9}
"#;

#[test]
fn not_required_omits_only_its_own_item() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`NotRequired` makes one item omittable and leaves its siblings required",
        rejected: COOPERAGE_REJECTED,
        accepted: COOPERAGE_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[aliased(COOPERAGE_ACCEPTED_ALIASED)],
    }
    .assert("NotRequired item")
}

// ── Required inside `total=False` ────────────────────────────────────────
// PEP 655: `total=False` flips the default, and `Required[X]` re-imposes the
// obligation on a single item. Omitting that item stays ill-typed.

const MILLRACE_REJECTED: &str = r#"
from typing import Required as MandatoryKey, TypedDict as MappingShape
class Millrace(MappingShape, total=False):
    head_of_water: MandatoryKey[int]
    sluice_gate: str
leat: Millrace = {"sluice_gate": "shut"}
"#;

const MILLRACE_ACCEPTED: &str = r#"
from typing import Required as MandatoryKey, TypedDict as MappingShape
class Millrace(MappingShape, total=False):
    head_of_water: MandatoryKey[int]
    sluice_gate: str
leat: Millrace = {"head_of_water": 4}
"#;

const MILLRACE_REJECTED_RENAMED: &str = r#"
from typing import Required as NeedsKey, TypedDict as KeyedShape
class Weir(KeyedShape, total=False):
    head_of_race: NeedsKey[int]
    weir_gate: str
toft: Weir = {"weir_gate": "shut"}
"#;

#[test]
fn required_survives_total_false() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Required[X]` re-imposes the obligation on one item of a `total=False` \
                      TypedDict",
        rejected: MILLRACE_REJECTED,
        accepted: MILLRACE_ACCEPTED,
        rejected_variants: &[renamed(MILLRACE_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("Required inside total=False")
}

const KILN_ACCEPTED: &str = r#"
from typing import TypedDict as MappingShape
class Kiln(MappingShape, total=False):
    firing_hours: int
    flue_height: float
hurdle: Kiln = {}
"#;

#[test]
fn total_false_permits_the_empty_literal() -> Result<(), Box<dyn std::error::Error>> {
    assert_accepted(
        "total=False, empty literal",
        "under `total=False` every item is omittable, so `{}` inhabits the type",
        KILN_ACCEPTED,
    )
}

// ── ReadOnly ─────────────────────────────────────────────────────────────
// PEP 705: a `ReadOnly` item may be supplied at construction and read
// afterwards, but never written through the mapping. Its writable sibling may.

const QUERNSTONE_REJECTED: &str = r#"
from typing import ReadOnly as FrozenKey, TypedDict as MappingShape
class Quernstone(MappingShape):
    quern_weight: int
    grinding_face: FrozenKey[str]
stone: Quernstone = {"quern_weight": 40, "grinding_face": "nether"}
stone["grinding_face"] = "runner"
"#;

const QUERNSTONE_ACCEPTED: &str = r#"
from typing import ReadOnly as FrozenKey, TypedDict as MappingShape
class Quernstone(MappingShape):
    quern_weight: int
    grinding_face: FrozenKey[str]
stone: Quernstone = {"quern_weight": 40, "grinding_face": "nether"}
stone["quern_weight"] = 41
"#;

const QUERNSTONE_REJECTED_IMPORT_FORM: &str = r#"
import typing
class Quernstone(typing.TypedDict):
    quern_weight: int
    grinding_face: typing.ReadOnly[str]
stone: Quernstone = {"quern_weight": 40, "grinding_face": "nether"}
stone["grinding_face"] = "runner"
"#;

#[test]
fn read_only_item_rejects_assignment() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `ReadOnly` item may be initialised but never assigned through the mapping",
        rejected: QUERNSTONE_REJECTED,
        accepted: QUERNSTONE_ACCEPTED,
        rejected_variants: &[import_form(QUERNSTONE_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("ReadOnly item assignment")
}

// ── Inheritance ──────────────────────────────────────────────────────────
// PEP 589: a subclass may add items, but may not redeclare an inherited item
// with an incompatible type. Adding a fresh item is always legal.

const BARBICAN_REJECTED: &str = r#"
from typing import TypedDict as MappingShape
class Barbican(MappingShape):
    embrasure_count: int
class Bailiwick(Barbican):
    embrasure_count: str
"#;

const BARBICAN_ACCEPTED: &str = r#"
from typing import TypedDict as MappingShape
class Barbican(MappingShape):
    embrasure_count: int
class Bailiwick(Barbican):
    hoarding_length: int
"#;

const BARBICAN_REJECTED_REFORMATTED: &str = "
from typing import TypedDict as MappingShape
# the outer work
class Barbican(MappingShape):
        embrasure_count: int
class Bailiwick(Barbican):

        # redeclared, and not with the inherited type
        embrasure_count: str
";

#[test]
fn subclass_may_not_retype_an_inherited_item() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a TypedDict subclass may not redeclare an inherited item with an \
                      incompatible type",
        rejected: BARBICAN_REJECTED,
        accepted: BARBICAN_ACCEPTED,
        rejected_variants: &[reformatted(BARBICAN_REJECTED_REFORMATTED)],
        accepted_variants: &[],
    }
    .assert("TypedDict inheritance, retyped item")
}

// PEP 705 refines that rule: a writable item's value type is invariant across
// inheritance, while a `ReadOnly` item's value type may be narrowed. The two
// programs below differ only in the qualifier, so a checker that reads the
// qualifier separates them and one that reads the text cannot.

const PALISADE_REJECTED: &str = r#"
from typing import TypedDict as MappingShape
class Palisade(MappingShape):
    stave_gauge: int | str
class Hoarding(Palisade):
    stave_gauge: int
"#;

const PALISADE_ACCEPTED: &str = r#"
from typing import ReadOnly as FrozenKey, TypedDict as MappingShape
class Palisade(MappingShape):
    stave_gauge: FrozenKey[int | str]
class Hoarding(Palisade):
    stave_gauge: FrozenKey[int]
"#;

#[test]
fn only_a_read_only_item_may_be_narrowed() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a writable item's value type is invariant; a `ReadOnly` item's may narrow",
        rejected: PALISADE_REJECTED,
        accepted: PALISADE_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("ReadOnly narrowing across inheritance")
}

// ── TypedDict is not a runtime class ─────────────────────────────────────
// The typing spec forbids a TypedDict type as the second argument to
// `isinstance` — it has no runtime representation to check against. Checking
// against `dict` itself is ordinary and legal.

const CHANDLERY_REJECTED: &str = r#"
from typing import TypedDict as MappingShape
class Chandlery(MappingShape):
    tallow_stock: int
def holds_tallow(offering: object) -> bool:
    return isinstance(offering, Chandlery)
"#;

const CHANDLERY_ACCEPTED: &str = r#"
from typing import TypedDict as MappingShape
class Chandlery(MappingShape):
    tallow_stock: int
def holds_tallow(offering: object) -> bool:
    return isinstance(offering, dict)
"#;

const CHANDLERY_REJECTED_IMPORT_FORM: &str = r#"
import typing
class Chandlery(typing.TypedDict):
    tallow_stock: int
def holds_tallow(offering: object) -> bool:
    return isinstance(offering, Chandlery)
"#;

#[test]
fn typeddict_is_not_an_isinstance_target() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a TypedDict type may not be the second argument to `isinstance`",
        rejected: CHANDLERY_REJECTED,
        accepted: CHANDLERY_ACCEPTED,
        rejected_variants: &[import_form(CHANDLERY_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("TypedDict under isinstance")
}

// ── Functional syntax ────────────────────────────────────────────────────
// PEP 589 gives a second spelling of the same declaration. It declares the
// same type, so it carries the same obligations — the totality defect below is
// the one from the first section, respelled.

const WAPENTAKE_FUNCTIONAL_REJECTED: &str = r#"
from typing import TypedDict as MappingShape
Wapentake = MappingShape("Wapentake", {"hundred_court": str, "hide_count": int})
sheaf: Wapentake = {"hundred_court": "Skyrack"}
"#;

const WAPENTAKE_FUNCTIONAL_ACCEPTED: &str = r#"
from typing import TypedDict as MappingShape
Wapentake = MappingShape("Wapentake", {"hundred_court": str, "hide_count": int})
sheaf: Wapentake = {"hundred_court": "Skyrack", "hide_count": 12}
"#;

const WAPENTAKE_FUNCTIONAL_RENAMED: &str = r#"
from typing import TypedDict as KeyedShape
Bailiwick = KeyedShape("Bailiwick", {"moot_court": str, "croft_tally": int})
gavel: Bailiwick = {"moot_court": "Skyrack"}
"#;

#[test]
fn functional_syntax_carries_the_same_totality() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the functional TypedDict call declares the same required items as the \
                      class form",
        rejected: WAPENTAKE_FUNCTIONAL_REJECTED,
        accepted: WAPENTAKE_FUNCTIONAL_ACCEPTED,
        rejected_variants: &[renamed(WAPENTAKE_FUNCTIONAL_RENAMED)],
        accepted_variants: &[],
    }
    .assert("functional TypedDict totality")
}

/// The two syntaxes declare one type, so they must draw one verdict.
///
/// Nothing but the declaration form differs between these pairs: same alias,
/// same class name, same items, same defect. A rule that recognises only the
/// `class` statement — or only the call — separates them here.
#[test]
fn functional_and_class_syntax_agree() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        code_multiset(&analyse(WAPENTAKE_MISSING_REJECTED)?),
        code_multiset(&analyse(WAPENTAKE_FUNCTIONAL_REJECTED)?),
        "the same omitted required item must be judged the same whether the TypedDict is \
         declared by class statement or by call. See [PERMTEST-FAMILY-A]."
    );
    assert_eq!(
        code_multiset(&analyse(WAPENTAKE_ACCEPTED)?),
        code_multiset(&analyse(WAPENTAKE_FUNCTIONAL_ACCEPTED)?),
        "a complete literal must be accepted under both declaration forms. \
         See [PERMTEST-FAMILY-A]."
    );
    Ok(())
}
