//! `ReadOnly` TypedDict items (PEP 705). [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! The spec's rules here are all *relational*, which is what makes them a good
//! test of resolution rather than spelling:
//!
//! * "Items that are read-only may not be mutated (added, modified, or removed)."
//! * "A read-only item in a superclass may be redeclared as mutable … in a
//!   subclass."
//! * "If an item is read-only in the superclass, the subclass may redeclare it
//!   with a different type that is assignable to the superclass type."
//!
//! The last two are *permissions*, so they are written here as accepted legs —
//! a rule that treats every `ReadOnly` redeclaration as an error passes the
//! prohibition and fails these.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── a read-only item may not be modified ─────────────────────────────────

#[test]
fn a_read_only_item_may_not_be_assigned() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec states read-only items may not be mutated; assigning through the \
                      subscript modifies the item",
        rejected: r#"
import typing


class Survey(typing.TypedDict):
    datum: typing.ReadOnly[str]
    hachure: int


def revise(chart: Survey) -> None:
    chart["datum"] = "clarke"
"#,
        accepted: r#"
import typing


class Survey(typing.TypedDict):
    datum: typing.ReadOnly[str]
    hachure: int


def revise(chart: Survey) -> None:
    chart["hachure"] = 3
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from typing import ReadOnly as Frozen


class Survey(Record):
    datum: Frozen[str]
    hachure: int


def revise(chart: Survey) -> None:
    chart["datum"] = "clarke"
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Survey(typing_extensions.TypedDict):
    datum: typing_extensions.ReadOnly[str]
    hachure: int


def revise(chart: Survey) -> None:
    chart["datum"] = "clarke"
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    origin: typing.ReadOnly[str]
    isogon: int


def amend(sheet: Plat) -> None:
    sheet["origin"] = "clarke"
"#,
            ),
            reformatted(
                "
import typing

class Survey( typing.TypedDict ):

        datum   : typing.ReadOnly[ str ]
        hachure : int

def revise( chart : Survey ) -> None :

        chart[ 'datum' ] = 'clarke'   # <- read-only item
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from typing import ReadOnly as Frozen


class Survey(Record):
    datum: Frozen[str]
    hachure: int


def revise(chart: Survey) -> None:
    chart["hachure"] = 3
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    origin: typing.ReadOnly[str]
    isogon: int


def amend(sheet: Plat) -> None:
    sheet["isogon"] = 3
"#,
            ),
            import_form(
                r#"
import typing_extensions as schema_forms


class Survey(schema_forms.TypedDict):
    datum: schema_forms.ReadOnly[str]
    hachure: int


def revise(chart: Survey) -> None:
    chart["hachure"] = 3
"#,
            ),
            reformatted(
                "
from typing import TypedDict as Record, ReadOnly as Frozen

class Survey( Record ):

        datum   : Frozen[ str ]
        hachure : int

def revise( chart : Survey ) -> None :

        chart[ 'hachure' ] = 3
",
            ),
        ],
    }
    .assert_by(
        "a read-only item may not be assigned",
        "typeddicts_readonly",
    )
}

// ── removal counts as mutation too ───────────────────────────────────────

#[test]
fn a_read_only_item_may_not_be_deleted() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason:
            "the spec counts removal as mutation, and separately rejects `del` unless the \
                      key is non-required *and* mutable; a read-only key fails the second \
                      condition even when it is declared `NotRequired`",
        rejected: r#"
import typing


class Survey(typing.TypedDict):
    datum: typing.ReadOnly[typing.NotRequired[str]]
    hachure: int


def revise(chart: Survey) -> None:
    del chart["datum"]
"#,
        accepted: r#"
import typing


class Survey(typing.TypedDict):
    datum: typing.NotRequired[str]
    hachure: int


def revise(chart: Survey) -> None:
    del chart["datum"]
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from typing import ReadOnly as Frozen
from typing import NotRequired as Optional_


class Survey(Record):
    datum: Frozen[Optional_[str]]
    hachure: int


def revise(chart: Survey) -> None:
    del chart["datum"]
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Survey(typing_extensions.TypedDict):
    datum: typing_extensions.ReadOnly[typing_extensions.NotRequired[str]]
    hachure: int


def revise(chart: Survey) -> None:
    del chart["datum"]
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    origin: typing.ReadOnly[typing.NotRequired[str]]
    isogon: int


def amend(sheet: Plat) -> None:
    del sheet["origin"]
"#,
            ),
            reformatted(
                "
import typing

class Survey( typing.TypedDict ):
        datum : typing.ReadOnly[ typing.NotRequired[ str ] ]
        hachure : int

def revise( chart : Survey ) -> None :
        del chart[
            'datum'   # <- non-required, but still read-only
        ]
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from typing import NotRequired as Optional_


class Survey(Record):
    datum: Optional_[str]
    hachure: int


def revise(chart: Survey) -> None:
    del chart["datum"]
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    origin: typing.NotRequired[str]
    isogon: int


def amend(sheet: Plat) -> None:
    del sheet["origin"]
"#,
            ),
            import_form(
                r#"
import typing_extensions as schema_forms


class Survey(schema_forms.TypedDict):
    datum: schema_forms.NotRequired[str]
    hachure: int


def revise(chart: Survey) -> None:
    del chart["datum"]
"#,
            ),
            reformatted(
                "
from typing import TypedDict as Record, NotRequired as Optional_

class Survey( Record ):
        datum   : Optional_[ str ]
        hachure : int

def revise( chart : Survey ) -> None :
        del chart[
            'datum'
        ]
",
            ),
        ],
    }
    .assert_by("a read-only item may not be deleted", "typeddicts_readonly")
}

// ── del is rejected on a required key even when mutable ──────────────────

#[test]
fn del_is_rejected_on_a_required_key() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec states `del obj['key']` should be rejected unless the key is \
                      non-required and mutable; a plain required item is mutable but not optional",
        rejected: r#"
import typing


class Survey(typing.TypedDict):
    hachure: int


def revise(chart: Survey) -> None:
    del chart["hachure"]
"#,
        accepted: r#"
import typing


class Survey(typing.TypedDict, total=False):
    hachure: int


def revise(chart: Survey) -> None:
    del chart["hachure"]
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record


class Survey(Record):
    hachure: int


def revise(chart: Survey) -> None:
    del chart["hachure"]
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Survey(typing_extensions.TypedDict):
    hachure: int


def revise(chart: Survey) -> None:
    del chart["hachure"]
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    isogon: int


def amend(sheet: Plat) -> None:
    del sheet["isogon"]
"#,
            ),
            reformatted(
                "
import typing

class Survey( typing.TypedDict ):
        hachure : int

def revise( chart : Survey ) -> None :

        del chart[ 'hachure' ]   # <- required key
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record


class Survey(Record, total=False):
    hachure: int


def revise(chart: Survey) -> None:
    del chart["hachure"]
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict, total=False):
    isogon: int


def amend(sheet: Plat) -> None:
    del sheet["isogon"]
"#,
            ),
            import_form(
                r#"
import typing_extensions as schema_forms


class Survey(schema_forms.TypedDict, total=False):
    hachure: int


def revise(chart: Survey) -> None:
    del chart["hachure"]
"#,
            ),
            reformatted(
                "
from typing import TypedDict as Record

class Survey(
        Record,
        total = False,
):
        hachure : int

def revise( chart : Survey ) -> None :
        del chart[
            'hachure'
        ]
",
            ),
        ],
    }
    .assert_by("del is rejected on a required key", "typeddicts_operations")
}

// ── the permissions the spec grants subclasses ───────────────────────────

#[test]
fn a_subclass_may_redeclare_a_read_only_item_as_mutable() -> Result<(), Box<dyn std::error::Error>>
{
    SpecObligation {
        spec_reason: "the spec permits a read-only item in a superclass to be redeclared as \
                      mutable in a subclass, and permits redeclaring it with a type assignable to \
                      the superclass type; redeclaring with an unrelated type is not permitted",
        rejected: r#"
import typing


class Survey(typing.TypedDict):
    datum: typing.ReadOnly[str]


class Plat(Survey):
    datum: int
"#,
        accepted: r#"
import typing


class Survey(typing.TypedDict):
    datum: typing.ReadOnly[object]


class Plat(Survey):
    datum: str
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from typing import ReadOnly as Frozen


class Survey(Record):
    datum: Frozen[str]


class Plat(Survey):
    datum: int
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Survey(typing_extensions.TypedDict):
    datum: typing_extensions.ReadOnly[str]


class Plat(Survey):
    datum: int
"#,
            ),
            renamed(
                r#"
import typing


class Sheet(typing.TypedDict):
    origin: typing.ReadOnly[str]


class Tracing(Sheet):
    origin: int
"#,
            ),
            reformatted(
                "
import typing

class Survey( typing.TypedDict ):
        datum : typing.ReadOnly[ str ]

class Plat( Survey ):

        datum : int   # <- int is not assignable to str
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from typing import ReadOnly as Frozen
from builtins import object as Base


class Survey(Record):
    datum: Frozen[Base]


class Plat(Survey):
    datum: str
"#,
            ),
            renamed(
                r#"
import typing


class Sheet(typing.TypedDict):
    origin: typing.ReadOnly[object]


class Tracing(Sheet):
    origin: str
"#,
            ),
            import_form(
                r#"
import typing_extensions as schema_forms
from builtins import object as BroadValue


class Survey(schema_forms.TypedDict):
    datum: schema_forms.ReadOnly[BroadValue]


class Plat(Survey):
    datum: str
"#,
            ),
            reformatted(
                "
from typing import TypedDict as Record, ReadOnly as Frozen
from builtins import object as BroadValue

class Survey( Record ):
        datum : Frozen[
            BroadValue
        ]

class Plat( Survey ):

        datum : str
",
            ),
        ],
    }
    .assert_by(
        "a subclass may redeclare a read-only item as mutable",
        "typeddicts_inheritance",
    )
}
