//! `Literal` and `LiteralString` obligations — the spec's value-level types.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `Literal` is the one place where the *value* of an expression is part of its
//! type, which makes it the easiest construct in the spec to fake: matching the
//! text between the brackets looks exactly like checking the type. It is not.
//! `Literal[1]` and `Literal[True]` are distinct types spelled almost the same,
//! and an alias holding a `Literal` must behave identically to the `Literal`
//! written inline.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── a Literal parameter admits only its own members ──────────────────────

#[test]
fn a_literal_parameter_admits_only_its_declared_members() -> Result<(), Box<dyn std::error::Error>>
{
    SpecObligation {
        spec_reason: "`Literal[\"warp\", \"weft\"]` is the type whose only values are those two \
                      strings; any other string is not a member",
        rejected: r#"
import typing


def thread(direction: typing.Literal["warp", "weft"]) -> None:
    return None


thread("selvedge")
"#,
        accepted: r#"
import typing


def thread(direction: typing.Literal["warp", "weft"]) -> None:
    return None


thread("weft")
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Literal as Exact


def thread(direction: Exact["warp", "weft"]) -> None:
    return None


thread("selvedge")
"#,
            ),
            import_form(
                r#"
import typing_extensions


def thread(direction: typing_extensions.Literal["warp", "weft"]) -> None:
    return None


thread("selvedge")
"#,
            ),
            renamed(
                r#"
import typing


def spin(bearing: typing.Literal["warp", "weft"]) -> None:
    return None


spin("selvedge")
"#,
            ),
            reformatted(
                "
import typing

def thread( direction : typing.Literal[ 'warp' , 'weft' ] ) -> None :

        return None

thread(
    'selvedge'   # <- not one of the two members
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Literal as Exact


def thread(direction: Exact["warp", "weft"]) -> None:
    return None


thread("weft")
"#,
            ),
            reformatted(
                "
import typing

def thread( direction : typing.Literal[ 'warp' , 'weft' , ] ) -> None :
        return None

thread( 'weft' )
",
            ),
        ],
    }
    .assert("a Literal parameter admits only its declared members")
}

// ── an alias holding a Literal behaves like the Literal ──────────────────

#[test]
fn a_literal_reached_through_an_alias_keeps_its_members() -> Result<(), Box<dyn std::error::Error>>
{
    SpecObligation {
        spec_reason: "an alias is transparent, so `Bearing = Literal[\"warp\", \"weft\"]` admits \
                      exactly what the inline form admits — the call sites here are identical and \
                      only the alias target differs",
        rejected: r#"
import typing

Bearing = typing.Literal["warp", "weft"]


def thread(direction: Bearing) -> None:
    return None


thread("roving")
"#,
        accepted: r#"
import typing

Bearing = typing.Literal["warp", "weft", "roving"]


def thread(direction: Bearing) -> None:
    return None


thread("roving")
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Literal as Exact

Bearing = Exact["warp", "weft"]


def thread(direction: Bearing) -> None:
    return None


thread("roving")
"#,
            ),
            import_form(
                r#"
import typing_extensions

Bearing = typing_extensions.Literal["warp", "weft"]


def thread(direction: Bearing) -> None:
    return None


thread("roving")
"#,
            ),
            renamed(
                r#"
import typing

Heading = typing.Literal["warp", "weft"]


def spin(bearing: Heading) -> None:
    return None


spin("roving")
"#,
            ),
            reformatted(
                "
import typing

Bearing = typing.Literal[
    'warp' ,
    'weft' ,
]

def thread( direction : Bearing ) -> None :
        return None

thread( 'roving' )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Literal as Exact

Bearing = Exact["warp", "weft", "roving"]


def thread(direction: Bearing) -> None:
    return None


thread("roving")
"#,
            ),
            renamed(
                r#"
import typing

Heading = typing.Literal["warp", "weft", "roving"]


def spin(bearing: Heading) -> None:
    return None


spin("roving")
"#,
            ),
        ],
    }
    .assert("a Literal reached through an alias keeps its members")
}

// ── Literal takes values, not types ──────────────────────────────────────

#[test]
fn literal_is_parameterised_by_values_not_types() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec enumerates what may appear inside `Literal[...]`: ints, strings, \
                      bytes, bools, enum members and `None`. A *type* is not a literal value, so \
                      `Literal[int]` is not a valid type expression",
        rejected: r#"
import typing


def thread(count: typing.Literal[int]) -> None:
    return None
"#,
        accepted: r#"
import typing


def thread(count: typing.Literal[3]) -> None:
    return None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Literal as Exact
from builtins import int as Whole


def thread(count: Exact[Whole]) -> None:
    return None
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def thread(count: typing.Literal[builtins.int]) -> None:
    return None
"#,
            ),
            renamed(
                r#"
import typing


def spin(tally: typing.Literal[int]) -> None:
    return None
"#,
            ),
            reformatted(
                "
import typing

def thread(
    count : typing.Literal[
        int   # <- a type, where a value is required
    ]
) -> None :
        return None
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Literal as Exact


def thread(count: Exact[3]) -> None:
    return None
"#,
            ),
            renamed(
                r#"
import typing


def spin(tally: typing.Literal[3]) -> None:
    return None
"#,
            ),
        ],
    }
    .assert("Literal is parameterised by values, not types")
}

// ── Literal[1] and Literal[True] are different types ─────────────────────

#[test]
fn literal_one_and_literal_true_are_distinct_types() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec requires `Literal[1]` and `Literal[True]` to be distinguished even \
                      though `True == 1` at runtime; the member's type is part of the literal type",
        rejected: r#"
import typing


def tension(setting: typing.Literal[1]) -> None:
    return None


tension(True)
"#,
        accepted: r#"
import typing


def tension(setting: typing.Literal[True]) -> None:
    return None


tension(True)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Literal as Exact


def tension(setting: Exact[1]) -> None:
    return None


tension(True)
"#,
            ),
            import_form(
                r#"
import typing_extensions


def tension(setting: typing_extensions.Literal[1]) -> None:
    return None


tension(True)
"#,
            ),
            renamed(
                r#"
import typing


def draw(notch: typing.Literal[1]) -> None:
    return None


draw(True)
"#,
            ),
            reformatted(
                "
import typing

def tension( setting : typing.Literal[ 1 ] ) -> None :
        return None

tension(
    True   # <- bool literal, not the int literal 1
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Literal as Exact


def tension(setting: Exact[True]) -> None:
    return None


tension(True)
"#,
            ),
            renamed(
                r#"
import typing


def draw(notch: typing.Literal[True]) -> None:
    return None


draw(True)
"#,
            ),
        ],
    }
    .assert("Literal[1] and Literal[True] are distinct types")
}

// ── LiteralString admits literals and their compositions ─────────────────

#[test]
fn literalstring_rejects_a_runtime_constructed_string() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`LiteralString` admits string literals and values built only from string \
                      literals; a string derived from arbitrary runtime input is a plain `str` \
                      and is not assignable to it",
        rejected: r#"
import typing


def weave(pattern: typing.LiteralString) -> None:
    return None


def loom(shuttle: str) -> None:
    weave(shuttle)
"#,
        accepted: r#"
import typing


def weave(pattern: typing.LiteralString) -> None:
    return None


def loom() -> None:
    weave("herring" + "bone")
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import LiteralString as Fixed


def weave(pattern: Fixed) -> None:
    return None


def loom(shuttle: str) -> None:
    weave(shuttle)
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def weave(pattern: typing.LiteralString) -> None:
    return None


def loom(shuttle: builtins.str) -> None:
    weave(shuttle)
"#,
            ),
            renamed(
                r#"
import typing


def plait(motif: typing.LiteralString) -> None:
    return None


def frame(bobbin: str) -> None:
    plait(bobbin)
"#,
            ),
            reformatted(
                "
import typing

def weave( pattern : typing.LiteralString ) -> None :
        return None

def loom( shuttle : str ) -> None :

        weave(
            shuttle   # <- provenance unknown, so plain str
        )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import LiteralString as Fixed


def weave(pattern: Fixed) -> None:
    return None


def loom() -> None:
    weave("herring" + "bone")
"#,
            ),
            renamed(
                r#"
import typing


def plait(motif: typing.LiteralString) -> None:
    return None


def frame() -> None:
    plait("herring" + "bone")
"#,
            ),
        ],
    }
    .assert("LiteralString rejects a runtime-constructed string")
}
