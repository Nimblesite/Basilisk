//! TypedDict key obligations — unknown keys, non-literal keys, qualifier
//! conflicts, and `closed=True`. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! The spec pairs each prohibition with an explicit *permission*, and the
//! permissions are where a rule written as "reject subscripting with anything
//! I can't read literally" goes wrong:
//!
//! * "The use of a key that is not known to exist should be reported as an
//!   error" — but
//! * "`d.get(e)` and `e in d` should be allowed … for an arbitrary expression
//!   `e` with type `str`."
//!
//! So the same non-literal expression is an error under `[]` and legal under
//! `.get()` / `in`. Both directions are asserted below.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── a key that is not known to exist ─────────────────────────────────────

#[test]
fn subscripting_an_unknown_key_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec requires the use of a key that is not known to exist to be reported \
                      as an error, even where it would not raise at runtime",
        rejected: r#"
import typing


class Survey(typing.TypedDict):
    hachure: int


def read(chart: Survey) -> int:
    return chart["isogon"]
"#,
        accepted: r#"
import typing


class Survey(typing.TypedDict):
    hachure: int


def read(chart: Survey) -> int:
    return chart["hachure"]
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record


class Survey(Record):
    hachure: int


def read(chart: Survey) -> int:
    return chart["isogon"]
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Survey(typing_extensions.TypedDict):
    hachure: int


def read(chart: Survey) -> int:
    return chart["isogon"]
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    contour: int


def scan(sheet: Plat) -> int:
    return sheet["graticule"]
"#,
            ),
            reformatted(
                "
import typing

class Survey( typing.TypedDict ):
        hachure : int

def read( chart : Survey ) -> int :

        return chart[
            'isogon'   # <- no such item
        ]
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record


class Survey(Record):
    hachure: int


def read(chart: Survey) -> int:
    return chart["hachure"]
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    contour: int


def scan(sheet: Plat) -> int:
    return sheet["contour"]
"#,
            ),
        ],
    }
    .assert("subscripting an unknown key is an error")
}

// ── non-literal subscript vs the get/in permission ───────────────────────

#[test]
fn a_non_literal_key_is_rejected_under_subscript_but_allowed_by_get_and_in()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec rejects a non-literal key because its value is unknown during type \
                      checking, yet explicitly allows `d.get(e)` and `e in d` for an arbitrary \
                      `str` expression; the permission is as normative as the prohibition",
        rejected: r#"
import typing


class Survey(typing.TypedDict):
    hachure: int


def read(chart: Survey, label: str) -> int:
    return chart[label]
"#,
        accepted: r#"
import typing


class Survey(typing.TypedDict):
    hachure: int


def read(chart: Survey, label: str) -> None:
    chart.get(label)
    if label in chart:
        return None
    return None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from builtins import str as Text


class Survey(Record):
    hachure: int


def read(chart: Survey, label: Text) -> int:
    return chart[label]
"#,
            ),
            import_form(
                r#"
import typing
import builtins


class Survey(typing.TypedDict):
    hachure: int


def read(chart: Survey, label: builtins.str) -> int:
    return chart[label]
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    contour: int


def scan(sheet: Plat, caption: str) -> int:
    return sheet[caption]
"#,
            ),
            reformatted(
                "
import typing

class Survey( typing.TypedDict ):
        hachure : int

def read( chart : Survey , label : str ) -> int :
        return chart[
            label   # <- value unknown statically
        ]
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from builtins import str as Text


class Survey(Record):
    hachure: int


def read(chart: Survey, label: Text) -> None:
    chart.get(label)
    if label in chart:
        return None
    return None
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    contour: int


def scan(sheet: Plat, caption: str) -> None:
    sheet.get(caption)
    if caption in sheet:
        return None
    return None
"#,
            ),
        ],
    }
    .assert("a non-literal key is rejected under subscript but allowed by get and in")
}

// ── Required and NotRequired cannot both qualify one item ────────────────

#[test]
fn required_and_notrequired_cannot_qualify_the_same_item()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec states it is an error to use both `Required` and `NotRequired` in \
                      the same item definition; either alone is well-formed",
        rejected: r#"
import typing


class Survey(typing.TypedDict):
    hachure: typing.Required[typing.NotRequired[int]]
"#,
        accepted: r#"
import typing


class Survey(typing.TypedDict):
    hachure: typing.NotRequired[int]
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from typing import Required as Mandatory
from typing import NotRequired as Optional_


class Survey(Record):
    hachure: Mandatory[Optional_[int]]
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Survey(typing_extensions.TypedDict):
    hachure: typing_extensions.Required[typing_extensions.NotRequired[int]]
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    contour: typing.Required[typing.NotRequired[int]]
"#,
            ),
            reformatted(
                "
import typing

class Survey( typing.TypedDict ):

        hachure : typing.Required[
            typing.NotRequired[ int ]   # <- both qualifiers on one item
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
    hachure: Optional_[int]
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict):
    contour: typing.NotRequired[int]
"#,
            ),
        ],
    }
    .assert("Required and NotRequired cannot qualify the same item")
}

// ── an explicit qualifier outranks total= ────────────────────────────────

#[test]
fn an_explicit_qualifier_outranks_the_total_argument()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec's procedure checks the qualifier first: `Required` wins over \
                      `total=False`, so the item stays required and omitting it from a literal is \
                      an error, while a sibling governed only by `total=False` may be omitted",
        rejected: r#"
import typing


class Survey(typing.TypedDict, total=False):
    hachure: typing.Required[int]
    isogon: int


chart: Survey = {"isogon": 2}
"#,
        accepted: r#"
import typing


class Survey(typing.TypedDict, total=False):
    hachure: typing.Required[int]
    isogon: int


chart: Survey = {"hachure": 1}
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from typing import Required as Mandatory


class Survey(Record, total=False):
    hachure: Mandatory[int]
    isogon: int


chart: Survey = {"isogon": 2}
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Survey(typing_extensions.TypedDict, total=False):
    hachure: typing_extensions.Required[int]
    isogon: int


chart: Survey = {"isogon": 2}
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict, total=False):
    contour: typing.Required[int]
    graticule: int


sheet: Plat = {"graticule": 2}
"#,
            ),
            reformatted(
                "
import typing

class Survey( typing.TypedDict , total = False ):
        hachure : typing.Required[ int ]
        isogon  : int

chart : Survey = {
    'isogon' : 2 ,   # <- required `hachure` omitted
}
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record
from typing import Required as Mandatory


class Survey(Record, total=False):
    hachure: Mandatory[int]
    isogon: int


chart: Survey = {"hachure": 1}
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict, total=False):
    contour: typing.Required[int]
    graticule: int


sheet: Plat = {"contour": 1}
"#,
            ),
        ],
    }
    .assert("an explicit qualifier outranks the total argument")
}

// ── closed=True admits nothing beyond the declared items ─────────────────

#[test]
fn a_closed_typeddict_admits_no_extra_keys() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "with `closed=True` the spec forbids any key beyond those explicitly \
                      specified; the same literal is well-typed against the open form",
        rejected: r#"
import typing


class Survey(typing.TypedDict, closed=True):
    hachure: int


chart: Survey = {"hachure": 1, "isogon": 2}
"#,
        accepted: r#"
import typing


class Survey(typing.TypedDict, closed=True):
    hachure: int


chart: Survey = {"hachure": 1}
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record


class Survey(Record, closed=True):
    hachure: int


chart: Survey = {"hachure": 1, "isogon": 2}
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Survey(typing_extensions.TypedDict, closed=True):
    hachure: int


chart: Survey = {"hachure": 1, "isogon": 2}
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict, closed=True):
    contour: int


sheet: Plat = {"contour": 1, "graticule": 2}
"#,
            ),
            reformatted(
                "
import typing

class Survey( typing.TypedDict , closed = True ):
        hachure : int

chart : Survey = {
    'hachure' : 1 ,
    'isogon'  : 2 ,   # <- beyond the closed set
}
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypedDict as Record


class Survey(Record, closed=True):
    hachure: int


chart: Survey = {"hachure": 1}
"#,
            ),
            renamed(
                r#"
import typing


class Plat(typing.TypedDict, closed=True):
    contour: int


sheet: Plat = {"contour": 1}
"#,
            ),
        ],
    }
    .assert("a closed TypedDict admits no extra keys")
}
