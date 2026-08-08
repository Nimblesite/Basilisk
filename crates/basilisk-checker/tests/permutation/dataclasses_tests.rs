//! Dataclass construction obligations — `field`, `KW_ONLY`, `InitVar`,
//! `frozen` and `order`. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `dataclasses` barely overlaps the 55 `typing`/`typing_extensions` symbols
//! `conformance/tests/` imports, so `dataclass`, `field`, `KW_ONLY`, `InitVar`
//! and `replace` are used bare. The one quarantined symbol here, `ClassVar`,
//! is reached only under an alias ([PERMTEST-VOCABULARY-RULES]); every class,
//! field, parameter and local name is disjoint from the suite's 913
//! identifiers, and both import forms carry cases so A6 and A7 are exercised.
//!
//! The obligations are those `dataclasses` states for the members it
//! synthesises — the typing spec defers to it for the generated `__init__`,
//! `__setattr__` and ordering methods.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── Field ordering ───────────────────────────────────────────────────────
// A field without a default may not follow one with a default: the synthesised
// `__init__` would carry a required parameter after an optional one.

const ORDERING_REJECTED: &str = r"
from dataclasses import dataclass

@dataclass
class Culvert:
    bore_mm: int = 300
    grade_permille: int
";

const ORDERING_ACCEPTED: &str = r"
from dataclasses import dataclass

@dataclass
class Culvert:
    grade_permille: int
    bore_mm: int = 300
";

const ORDERING_REJECTED_ALIASED: &str = r"
from dataclasses import dataclass as moulded

@moulded
class Culvert:
    bore_mm: int = 300
    grade_permille: int
";

#[test]
fn bare_field_may_not_follow_a_defaulted_field() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a field without a default may not follow one with a default",
        rejected: ORDERING_REJECTED,
        accepted: ORDERING_ACCEPTED,
        rejected_variants: &[aliased(ORDERING_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("dataclass field ordering")
}

// ── ClassVar is not a field ──────────────────────────────────────────────
// A `ClassVar`-annotated name is excluded from the field list: it takes no
// `__init__` parameter, and its default does not make a following bare field
// an ordering error.

const CLASSVAR_REJECTED: &str = r#"
from dataclasses import dataclass
from typing import ClassVar as PerClassOnly

@dataclass
class Tanner:
    tannery_town: PerClassOnly[str] = "Bermondsey"
    pit_count: int

Tanner("Ludlow", 6)
"#;

const CLASSVAR_ACCEPTED: &str = r#"
from dataclasses import dataclass
from typing import ClassVar as PerClassOnly

@dataclass
class Tanner:
    tannery_town: PerClassOnly[str] = "Bermondsey"
    pit_count: int

Tanner(6)
"#;

const CLASSVAR_REJECTED_REFORMATTED: &str = "
from dataclasses import dataclass
from typing import ClassVar as PerClassOnly

@dataclass
class Tanner:  # a hide-tanning yard

        tannery_town: PerClassOnly[str] = 'Bermondsey'

        pit_count: int

Tanner(
        'Ludlow',
        6,
)
";

#[test]
fn classvar_takes_no_init_parameter() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a ClassVar is not a field, so it is neither an __init__ \
                      parameter nor a defaulted field for ordering purposes",
        rejected: CLASSVAR_REJECTED,
        accepted: CLASSVAR_ACCEPTED,
        rejected_variants: &[reformatted(CLASSVAR_REJECTED_REFORMATTED)],
        accepted_variants: &[],
    }
    .assert("ClassVar excluded from the field list")
}

// ── KW_ONLY ──────────────────────────────────────────────────────────────
// A pseudo-field annotated `KW_ONLY` marks every field after it keyword-only in
// the synthesised `__init__`. Supplying such a field positionally is an error.

const KWONLY_REJECTED: &str = r"
from dataclasses import KW_ONLY, dataclass

@dataclass
class Trebuchet:
    arm_metres: float
    _: KW_ONLY
    counterweight_kg: float

Trebuchet(4.0, 900.0)
";

const KWONLY_ACCEPTED: &str = r"
from dataclasses import KW_ONLY, dataclass

@dataclass
class Trebuchet:
    arm_metres: float
    _: KW_ONLY
    counterweight_kg: float

Trebuchet(4.0, counterweight_kg=900.0)
";

const KWONLY_REJECTED_IMPORT_FORM: &str = r"
import dataclasses

@dataclasses.dataclass
class Trebuchet:
    arm_metres: float
    _: dataclasses.KW_ONLY
    counterweight_kg: float

Trebuchet(4.0, 900.0)
";

#[test]
fn kw_only_sentinel_forbids_positional_supply() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "fields following a KW_ONLY pseudo-field are keyword-only in __init__",
        rejected: KWONLY_REJECTED,
        accepted: KWONLY_ACCEPTED,
        rejected_variants: &[import_form(KWONLY_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("KW_ONLY boundary")
}

// ── frozen: attribute assignment ─────────────────────────────────────────
// `frozen=True` synthesises `__setattr__`/`__delattr__` that raise, so writing
// an attribute on an instance is an error. `replace` is the sanctioned repair.

const FROZEN_REJECTED: &str = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class Wapentake:
    hide_count: int

manor = Wapentake(12)
manor.hide_count = 13
";

const FROZEN_ACCEPTED: &str = r"
from dataclasses import dataclass, replace

@dataclass(frozen=True)
class Wapentake:
    hide_count: int

manor = Wapentake(12)
print(replace(manor, hide_count=13).hide_count)
";

const FROZEN_REJECTED_RENAMED: &str = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class Sokeland:
    carucate_count: int

estate = Sokeland(12)
estate.carucate_count = 13
";

#[test]
fn frozen_instance_rejects_attribute_assignment() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "frozen=True makes instance attributes read-only after __init__",
        rejected: FROZEN_REJECTED,
        accepted: FROZEN_ACCEPTED,
        rejected_variants: &[renamed(FROZEN_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("frozen attribute assignment")
}

// ── frozen: inheritance ──────────────────────────────────────────────────
// Frozen-ness may not change across a dataclass hierarchy: a frozen dataclass
// cannot inherit from a non-frozen one, nor the reverse.

const FROZEN_INHERIT_REJECTED: &str = r"
from dataclasses import dataclass

@dataclass
class Quernstone:
    diameter_mm: int

@dataclass(frozen=True)
class Grindstone(Quernstone):
    dressing: str
";

const FROZEN_INHERIT_ACCEPTED: &str = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class Quernstone:
    diameter_mm: int

@dataclass(frozen=True)
class Grindstone(Quernstone):
    dressing: str
";

const FROZEN_INHERIT_REJECTED_IMPORT_FORM: &str = r"
import dataclasses

@dataclasses.dataclass
class Quernstone:
    diameter_mm: int

@dataclasses.dataclass(frozen=True)
class Grindstone(Quernstone):
    dressing: str
";

#[test]
fn frozen_dataclass_may_not_extend_a_mutable_one() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a frozen dataclass cannot inherit from a non-frozen dataclass",
        rejected: FROZEN_INHERIT_REJECTED,
        accepted: FROZEN_INHERIT_ACCEPTED,
        rejected_variants: &[import_form(FROZEN_INHERIT_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("frozen inheritance")
}

// ── order ────────────────────────────────────────────────────────────────
// Ordering methods are synthesised only under `order=True`. With the default
// `order=False` no `__lt__` exists, so `<` between two instances is unsupported.

const ORDER_REJECTED: &str = r"
from dataclasses import dataclass

@dataclass
class Firkin:
    gallons: int

print(Firkin(9) < Firkin(11))
";

const ORDER_ACCEPTED: &str = r"
from dataclasses import dataclass

@dataclass(order=True)
class Firkin:
    gallons: int

print(Firkin(9) < Firkin(11))
";

const ORDER_REJECTED_IMPORT_FORM: &str = r"
import dataclasses

@dataclasses.dataclass
class Firkin:
    gallons: int

print(Firkin(9) < Firkin(11))
";

#[test]
fn default_dataclass_supports_no_less_than() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "order defaults to False, so no __lt__ is synthesised",
        rejected: ORDER_REJECTED,
        accepted: ORDER_ACCEPTED,
        rejected_variants: &[import_form(ORDER_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("order=False leaves < undefined")
}

// ── InitVar: not an attribute ────────────────────────────────────────────
// An `InitVar` field becomes an `__init__` parameter and is forwarded to
// `__post_init__`, but it is never stored — reading it off an instance fails.

const INITVAR_REJECTED: &str = r"
from dataclasses import InitVar, dataclass, field

@dataclass
class Bailiwick:
    hearths: int
    tithe_rate: InitVar[float]
    tithe_due: float = field(init=False)

    def __post_init__(self, tithe_rate: float) -> None:
        self.tithe_due = self.hearths * tithe_rate

shire = Bailiwick(40, 0.1)
print(shire.tithe_rate)
";

const INITVAR_ACCEPTED: &str = r"
from dataclasses import InitVar, dataclass, field

@dataclass
class Bailiwick:
    hearths: int
    tithe_rate: InitVar[float]
    tithe_due: float = field(init=False)

    def __post_init__(self, tithe_rate: float) -> None:
        self.tithe_due = self.hearths * tithe_rate

shire = Bailiwick(40, 0.1)
print(shire.tithe_due)
";

const INITVAR_REJECTED_ALIASED: &str = r"
from dataclasses import InitVar as Seeded, dataclass as moulded, field as slot

@moulded
class Bailiwick:
    hearths: int
    tithe_rate: Seeded[float]
    tithe_due: float = slot(init=False)

    def __post_init__(self, tithe_rate: float) -> None:
        self.tithe_due = self.hearths * tithe_rate

shire = Bailiwick(40, 0.1)
print(shire.tithe_rate)
";

#[test]
fn initvar_is_not_an_instance_attribute() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an InitVar is passed to __post_init__ and never stored on the instance",
        rejected: INITVAR_REJECTED,
        accepted: INITVAR_ACCEPTED,
        rejected_variants: &[aliased(INITVAR_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("InitVar is not an attribute")
}

// ── InitVar: constructor arity ───────────────────────────────────────────
// The other half of the same rule: an `InitVar` without a default *is* a
// required `__init__` parameter, so omitting it at the call site is an error.

const INITVAR_ARITY_REJECTED: &str = r"
from dataclasses import InitVar, dataclass

@dataclass
class Kiln:
    chamber_count: int
    firing_hours: InitVar[int]

    def __post_init__(self, firing_hours: int) -> None:
        print(self.chamber_count * firing_hours)

Kiln(3)
";

const INITVAR_ARITY_ACCEPTED: &str = r"
from dataclasses import InitVar, dataclass

@dataclass
class Kiln:
    chamber_count: int
    firing_hours: InitVar[int]

    def __post_init__(self, firing_hours: int) -> None:
        print(self.chamber_count * firing_hours)

Kiln(3, 18)
";

const INITVAR_ARITY_REJECTED_RENAMED: &str = r"
from dataclasses import InitVar, dataclass

@dataclass
class Sluice:
    gate_count: int
    sluicing_hours: InitVar[int]

    def __post_init__(self, sluicing_hours: int) -> None:
        print(self.gate_count * sluicing_hours)

Sluice(3)
";

#[test]
fn initvar_is_a_required_init_parameter() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an InitVar without a default is a required __init__ parameter",
        rejected: INITVAR_ARITY_REJECTED,
        accepted: INITVAR_ARITY_ACCEPTED,
        rejected_variants: &[renamed(INITVAR_ARITY_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("InitVar constructor arity")
}

// ── Mutable defaults ─────────────────────────────────────────────────────
// A field default is shared by every instance, so a mutable default is
// rejected; `field(default_factory=…)` is the sanctioned form.

const MUTABLE_DEFAULT_REJECTED: &str = r"
from dataclasses import dataclass

@dataclass
class Coppice:
    stools: list[str] = []
";

const MUTABLE_DEFAULT_ACCEPTED: &str = r"
from dataclasses import dataclass, field

@dataclass
class Coppice:
    stools: list[str] = field(default_factory=list)
";

const MUTABLE_DEFAULT_REJECTED_IMPORT_FORM: &str = r"
import dataclasses

@dataclasses.dataclass
class Coppice:
    stools: list[str] = []
";

const MUTABLE_DEFAULT_ACCEPTED_ALIASED: &str = r"
from dataclasses import dataclass as moulded, field as slot

@moulded
class Coppice:
    stools: list[str] = slot(default_factory=list)
";

#[test]
fn mutable_default_must_use_a_factory() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a mutable default is shared across instances and must be a default_factory",
        rejected: MUTABLE_DEFAULT_REJECTED,
        accepted: MUTABLE_DEFAULT_ACCEPTED,
        rejected_variants: &[import_form(MUTABLE_DEFAULT_REJECTED_IMPORT_FORM)],
        accepted_variants: &[aliased(MUTABLE_DEFAULT_ACCEPTED_ALIASED)],
    }
    .assert("mutable field default")
}
