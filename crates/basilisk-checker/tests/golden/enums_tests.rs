//! Enumerations — `Flag`, `IntEnum`, `StrEnum`, `auto`, `member`, `nonmember`,
//! `unique` — judged against the typing spec's enum chapter.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `conformance/tests/` reaches enums almost entirely through the plain `Enum`
//! base and the bare spellings `Flag`, `member`, `nonmember`; it never mentions
//! `IntEnum`, `StrEnum` or `unique` at all. Every case below builds on the
//! `enum` module reached by attribute (`enum.IntEnum`) or under an alias, so a
//! rule keyed to a base-class *spelling* cannot answer any of them. Identifiers
//! come from a vocabulary disjoint from the suite's 913 names, and the one
//! quarantined `typing` symbol used here (`Never`) appears only aliased.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── Members are immutable ────────────────────────────────────────────────
// The spec binds each member name once, in the class body, and treats it as
// effectively `Final`. Assigning over that name is an error; reading it into a
// fresh name is not.

const REBIND_REJECTED: &str = r"
import enum
class Sluice(enum.IntEnum):
    SHUT = 0
    OPEN = 1
Sluice.OPEN = 5
";

const REBIND_ACCEPTED: &str = r"
import enum
class Sluice(enum.IntEnum):
    SHUT = 0
    OPEN = 1
latch = Sluice.OPEN
";

const REBIND_REJECTED_ALIASED: &str = r"
from enum import IntEnum as OrdinalEnum
class Sluice(OrdinalEnum):
    SHUT = 0
    OPEN = 1
Sluice.OPEN = 5
";

const REBIND_REJECTED_REFORMATTED: &str = r"
import enum

class Sluice(enum.IntEnum):  # a gate on a millrace

      SHUT = 0

      OPEN = 1
# the rebinding, one line down
Sluice.OPEN = (5)
";

#[test]
fn enum_member_cannot_be_rebound() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an enum member is immutable, so assigning over it is an error",
        rejected: REBIND_REJECTED,
        accepted: REBIND_ACCEPTED,
        rejected_variants: &[
            aliased(REBIND_REJECTED_ALIASED),
            reformatted(REBIND_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("enum member immutability")
}

// ── `.value` follows the assigned literal ────────────────────────────────
// A member's `_value_` is the type of the expression assigned in the class
// body: `SCOURED = 9` gives `.value` type `int`. `@enum.unique` constrains
// which values may repeat, never what type they have.

const VALUE_REJECTED: &str = r"
import enum
@enum.unique
class Culvert(enum.IntEnum):
    SILTED = 4
    SCOURED = 9
soundings: str = Culvert.SCOURED.value
";

const VALUE_ACCEPTED: &str = r"
import enum
@enum.unique
class Culvert(enum.IntEnum):
    SILTED = 4
    SCOURED = 9
soundings: int = Culvert.SCOURED.value
";

const VALUE_REJECTED_IMPORT_FORM: &str = r"
from enum import IntEnum, unique
@unique
class Culvert(IntEnum):
    SILTED = 4
    SCOURED = 9
soundings: str = Culvert.SCOURED.value
";

const VALUE_REJECTED_RENAMED: &str = r"
import enum
@enum.unique
class Hogshead(enum.IntEnum):
    TIERCE = 4
    BUTT = 9
plumbline: str = Hogshead.BUTT.value
";

#[test]
fn member_value_type_follows_the_assigned_literal() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`SCOURED = 9` makes the member's `.value` an `int`, never a `str`",
        rejected: VALUE_REJECTED,
        accepted: VALUE_ACCEPTED,
        rejected_variants: &[
            import_form(VALUE_REJECTED_IMPORT_FORM),
            renamed(VALUE_REJECTED_RENAMED),
        ],
        accepted_variants: &[],
    }
    .assert("member value type")
}

// ── `auto()` in a `StrEnum` ──────────────────────────────────────────────
// `StrEnum._generate_next_value_` returns the member name, so `auto()` under a
// `StrEnum` produces a `str` value — not the `int` it produces elsewhere.

const AUTO_REJECTED: &str = r"
import enum
class Kiln(enum.StrEnum):
    BISQUE = enum.auto()
    GLOST = enum.auto()
firing: int = Kiln.GLOST.value
";

const AUTO_ACCEPTED: &str = r"
import enum
class Kiln(enum.StrEnum):
    BISQUE = enum.auto()
    GLOST = enum.auto()
firing: str = Kiln.GLOST.value
";

const AUTO_REJECTED_ALIASED: &str = r"
from enum import StrEnum as TextEnum, auto as next_value
class Kiln(TextEnum):
    BISQUE = next_value()
    GLOST = next_value()
firing: int = Kiln.GLOST.value
";

const AUTO_REJECTED_REFORMATTED: &str = r"
import enum

class Kiln(
      enum.StrEnum,
):
      # two firings, both auto-valued
      BISQUE = enum.auto()
      GLOST = enum.auto()

firing: int = (Kiln.GLOST).value
";

#[test]
fn str_enum_auto_yields_a_str_value() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`enum.auto()` under `StrEnum` yields the member name, a `str`",
        rejected: AUTO_REJECTED,
        accepted: AUTO_ACCEPTED,
        rejected_variants: &[
            aliased(AUTO_REJECTED_ALIASED),
            reformatted(AUTO_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("StrEnum auto value")
}

// ── Members of two enums are never equal ─────────────────────────────────
// Distinct enum classes share no members, so `lever is Gantry.RIGGED` can never
// hold for a `Sluice`: that branch is unreachable and `lever` narrows to `Never`
// inside it. Testing against a member of `lever`'s own enum narrows to that
// member instead, which is emphatically not `Never`.

const DISJOINT_ACCEPTED: &str = r"
import enum
from typing import Never as Uninhabited
class Sluice(enum.IntEnum):
    SHUT = 0
    OPEN = 1
class Gantry(enum.IntEnum):
    RIGGED = 1
def wicket(lever: Sluice) -> None:
    if lever is Gantry.RIGGED:
        sink: Uninhabited = lever
";

const DISJOINT_REJECTED: &str = r"
import enum
from typing import Never as Uninhabited
class Sluice(enum.IntEnum):
    SHUT = 0
    OPEN = 1
class Gantry(enum.IntEnum):
    RIGGED = 1
def wicket(lever: Sluice) -> None:
    if lever is Sluice.OPEN:
        sink: Uninhabited = lever
";

const DISJOINT_REJECTED_RENAMED: &str = r"
import enum
from typing import Never as Uninhabited
class Windlass(enum.IntEnum):
    TURNS = 0
    PAWLS = 1
class Capstan(enum.IntEnum):
    DRUM = 1
def pierce(barrel: Windlass) -> None:
    if barrel is Windlass.PAWLS:
        awl: Uninhabited = barrel
";

#[test]
fn cross_enum_identity_is_never_satisfied() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a member of one enum never equals a member of another, so that \
                      branch is unreachable and the subject narrows to `Never`",
        rejected: DISJOINT_REJECTED,
        accepted: DISJOINT_ACCEPTED,
        rejected_variants: &[renamed(DISJOINT_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("cross-enum identity narrowing")
}

// ── `StrEnum` members are `str`s; `IntEnum` members are not ──────────────
// `StrEnum` mixes `str` into every member, so a member is accepted wherever a
// `str` is required. `IntEnum` mixes in `int`, which is not a `str`.

const MIXIN_REJECTED: &str = r#"
import enum
class Kiln(enum.StrEnum):
    GLOST = "glost"
class Sluice(enum.IntEnum):
    OPEN = 1
def emboss(sigil: str) -> str:
    return sigil.upper()
emboss(Sluice.OPEN)
"#;

const MIXIN_ACCEPTED: &str = r#"
import enum
class Kiln(enum.StrEnum):
    GLOST = "glost"
class Sluice(enum.IntEnum):
    OPEN = 1
def emboss(sigil: str) -> str:
    return sigil.upper()
emboss(Kiln.GLOST)
"#;

const MIXIN_REJECTED_ALIASED: &str = r#"
from enum import IntEnum as OrdinalEnum, StrEnum as TextEnum
class Kiln(TextEnum):
    GLOST = "glost"
class Sluice(OrdinalEnum):
    OPEN = 1
def emboss(sigil: str) -> str:
    return sigil.upper()
emboss(Sluice.OPEN)
"#;

const MIXIN_REJECTED_REFORMATTED: &str = "
import enum

class Kiln(enum.StrEnum):
      GLOST = 'glost'   # single-quoted, same string

class Sluice(enum.IntEnum):
      OPEN = 1

def emboss(sigil: str) -> str:
      return sigil.upper()

emboss(
      Sluice.OPEN,
)
";

#[test]
fn str_enum_member_is_a_str_and_int_enum_member_is_not() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`StrEnum` mixes `str` into its members; `IntEnum` mixes `int`, \
                      which is not assignable to `str`",
        rejected: MIXIN_REJECTED,
        accepted: MIXIN_ACCEPTED,
        rejected_variants: &[
            aliased(MIXIN_REJECTED_ALIASED),
            reformatted(MIXIN_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("StrEnum str mixin")
}

// ── `nonmember` and `member` decide the member list ──────────────────────
// `enum.nonmember(x)` keeps the attribute off the member list: `Culvert.TARE`
// is then the plain wrapped value, not a `Culvert`, so passing it where the
// enum is required is an error. `enum.member(x)` makes the same attribute a
// member, which repairs the call.

const NONMEMBER_REJECTED: &str = r"
import enum
class Culvert(enum.IntEnum):
    SILTED = 4
    TARE = enum.nonmember(11)
def dredge(state: Culvert) -> int:
    return state.value
dredge(Culvert.TARE)
";

const NONMEMBER_ACCEPTED: &str = r"
import enum
class Culvert(enum.IntEnum):
    SILTED = 4
    TARE = enum.member(11)
def dredge(state: Culvert) -> int:
    return state.value
dredge(Culvert.TARE)
";

const NONMEMBER_REJECTED_ALIASED: &str = r"
from enum import IntEnum as OrdinalEnum, nonmember as kept_off_the_roll
class Culvert(OrdinalEnum):
    SILTED = 4
    TARE = kept_off_the_roll(11)
def dredge(state: Culvert) -> int:
    return state.value
dredge(Culvert.TARE)
";

const NONMEMBER_REJECTED_RENAMED: &str = r"
import enum
class Bodkin(enum.IntEnum):
    EYED = 4
    SPARE = enum.nonmember(11)
def pierce(awl: Bodkin) -> int:
    return awl.value
pierce(Bodkin.SPARE)
";

#[test]
fn nonmember_is_absent_from_the_member_list() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`enum.nonmember` excludes the attribute from the member list, so it \
                      is not a value of the enum type",
        rejected: NONMEMBER_REJECTED,
        accepted: NONMEMBER_ACCEPTED,
        rejected_variants: &[
            aliased(NONMEMBER_REJECTED_ALIASED),
            renamed(NONMEMBER_REJECTED_RENAMED),
        ],
        accepted_variants: &[],
    }
    .assert("nonmember exclusion")
}

// ── `Flag` combines only within one flag class ───────────────────────────
// `Flag.__or__` is typed over the enclosing flag class, so `|` between members
// of two different `Flag` subclasses has no applicable operand type.

const FLAG_REJECTED: &str = r"
import enum
class Portcullis(enum.Flag):
    RAISED = 1
    BARRED = 2
class Trebuchet(enum.Flag):
    LOOSED = 2
blend = Portcullis.RAISED | Trebuchet.LOOSED
";

const FLAG_ACCEPTED: &str = r"
import enum
class Portcullis(enum.Flag):
    RAISED = 1
    BARRED = 2
class Trebuchet(enum.Flag):
    LOOSED = 2
blend = Portcullis.RAISED | Portcullis.BARRED
";

const FLAG_REJECTED_IMPORT_FORM: &str = r"
from enum import Flag
class Portcullis(Flag):
    RAISED = 1
    BARRED = 2
class Trebuchet(Flag):
    LOOSED = 2
blend = Portcullis.RAISED | Trebuchet.LOOSED
";

const FLAG_REJECTED_REFORMATTED: &str = r"
import enum

class Portcullis(enum.Flag):
      RAISED = 1; BARRED = 2

class Trebuchet(enum.Flag):
      LOOSED = 2
# combining across two flag classes
blend = (
      Portcullis.RAISED
      | Trebuchet.LOOSED
)
";

#[test]
fn flag_or_requires_one_flag_class() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Flag.__or__` accepts only members of the same flag class",
        rejected: FLAG_REJECTED,
        accepted: FLAG_ACCEPTED,
        rejected_variants: &[
            import_form(FLAG_REJECTED_IMPORT_FORM),
            reformatted(FLAG_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("Flag operand class")
}

// ── An enum with members is implicitly final ─────────────────────────────
// A subclass may add members only to a base that declares none. Once
// `Wapentake` has members it is implicitly final and cannot be a base at all.

const SUBCLASS_REJECTED: &str = r"
import enum
class Wapentake(enum.IntEnum):
    HUNDRED = 100
    RIDING = 3
class Bailiwick(Wapentake):
    SHIRE = 40
";

const SUBCLASS_ACCEPTED: &str = r"
import enum
class Wapentake(enum.IntEnum):
    def writ(self) -> str:
        return self.name
class Bailiwick(Wapentake):
    HUNDRED = 100
    SHIRE = 40
";

const SUBCLASS_REJECTED_ALIASED: &str = r"
from enum import IntEnum as OrdinalEnum
class Wapentake(OrdinalEnum):
    HUNDRED = 100
    RIDING = 3
class Bailiwick(Wapentake):
    SHIRE = 40
";

const SUBCLASS_ACCEPTED_RENAMED: &str = r"
import enum
class Firkin(enum.IntEnum):
    def tally(self) -> str:
        return self.name
class Trundle(Firkin):
    NINE = 9
    EIGHTEEN = 18
";

#[test]
fn enum_with_members_cannot_be_subclassed() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an enum that defines members is implicitly final and cannot be a base",
        rejected: SUBCLASS_REJECTED,
        accepted: SUBCLASS_ACCEPTED,
        rejected_variants: &[aliased(SUBCLASS_REJECTED_ALIASED)],
        accepted_variants: &[renamed(SUBCLASS_ACCEPTED_RENAMED)],
    }
    .assert("implicitly final enum")
}
