//! `assert_type`, `cast`, and `reveal_type` — the spec's static-introspection
//! directives. [PERMTEST-FAMILY-A] / [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! These are the sharpest permutation targets in the whole spec. Every one takes
//! a *type expression* as an argument, so a checker that inspects the argument's
//! source text rather than its resolved type passes the canonical spelling and
//! fails every respelling of it.
//!
//! `assert_type` is sharper still: it demands type *identity*, not assignability.
//! An alias, a `from … import … as`, and a dotted attribute path must all compare
//! equal to the type they name — and two programs that differ only in what an
//! alias points at must get opposite verdicts from byte-identical call sites.

use super::harness::{
    assert_invariant, aliased, import_form, reformatted, renamed, SpecObligation,
};

// ── assert_type compares the inferred type ───────────────────────────────

#[test]
fn assert_type_rejects_a_mismatched_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`assert_type(v, T)` is an error unless the inferred type of `v` is exactly \
                      `T`; `1` infers `int`, not `str`",
        rejected: r#"
import typing


def assay() -> None:
    feldspar = 1
    typing.assert_type(feldspar, str)
"#,
        accepted: r#"
import typing


def assay() -> None:
    feldspar = 1
    typing.assert_type(feldspar, int)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import str as Text


def assay() -> None:
    feldspar = 1
    claim(feldspar, Text)
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def assay() -> None:
    feldspar = 1
    typing.assert_type(feldspar, builtins.str)
"#,
            ),
            renamed(
                r#"
import typing


def probe() -> None:
    gabbro = 1
    typing.assert_type(gabbro, str)
"#,
            ),
            reformatted(
                "
import typing

def assay() -> None:

        feldspar = 1
        typing.assert_type(
            feldspar ,
            str ,   # <- inferred int, asserted str
        )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import int as Whole


def assay() -> None:
    feldspar = 1
    claim(feldspar, Whole)
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def assay() -> None:
    feldspar = 1
    typing.assert_type(feldspar, builtins.int)
"#,
            ),
            renamed(
                r#"
import typing


def probe() -> None:
    gabbro = 1
    typing.assert_type(gabbro, int)
"#,
            ),
        ],
    }
    .assert("assert_type rejects a mismatched type")
}

// ── the declared type wins over the assigned value ───────────────────────

#[test]
fn assert_type_uses_the_declared_type_not_the_runtime_class()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a declared annotation fixes the static type of the binding; `basalt: object \
                      = 1` has static type `object`, so asserting `int` is an error even though \
                      the value is an `int` at runtime",
        rejected: r#"
import typing


def assay() -> None:
    basalt: object = 1
    typing.assert_type(basalt, int)
"#,
        accepted: r#"
import typing


def assay() -> None:
    basalt: object = 1
    typing.assert_type(basalt, object)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import object as Base
from builtins import int as Whole


def assay() -> None:
    basalt: Base = 1
    claim(basalt, Whole)
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def assay() -> None:
    basalt: builtins.object = 1
    typing.assert_type(basalt, builtins.int)
"#,
            ),
            renamed(
                r#"
import typing


def probe() -> None:
    gneiss: object = 1
    typing.assert_type(gneiss, int)
"#,
            ),
            reformatted(
                "
import typing

def assay() -> None:
        basalt : object = 1

        # declared `object`; the value being an int is irrelevant
        typing.assert_type( basalt , int )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import object as Base


def assay() -> None:
    basalt: Base = 1
    claim(basalt, Base)
"#,
            ),
            renamed(
                r#"
import typing


def probe() -> None:
    gneiss: object = 1
    typing.assert_type(gneiss, object)
"#,
            ),
        ],
    }
    .assert("assert_type uses the declared type, not the runtime class")
}

// ── alias identity: same call site, opposite verdicts ────────────────────

#[test]
fn assert_type_resolves_the_alias_target_not_the_alias_name()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a type alias is transparent: `Grain = str` makes `assert_type(1, Grain)` the \
                      same error as `assert_type(1, str)`, while `Grain = int` makes it correct. \
                      The call site is byte-identical in both programs, so only the resolved \
                      target can decide",
        rejected: r#"
import typing

Grain = str


def assay() -> None:
    typing.assert_type(1, Grain)
"#,
        accepted: r#"
import typing

Grain = int


def assay() -> None:
    typing.assert_type(1, Grain)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import str as Text

Grain = Text


def assay() -> None:
    claim(1, Grain)
"#,
            ),
            import_form(
                r#"
import typing
import builtins

Grain = builtins.str


def assay() -> None:
    typing.assert_type(1, Grain)
"#,
            ),
            renamed(
                r#"
import typing

Fabric = str


def probe() -> None:
    typing.assert_type(1, Fabric)
"#,
            ),
            reformatted(
                "
import typing

Grain = str   # <- alias points at str

def assay() -> None:

        typing.assert_type( 1 , Grain )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import int as Whole

Grain = Whole


def assay() -> None:
    claim(1, Grain)
"#,
            ),
            renamed(
                r#"
import typing

Fabric = int


def probe() -> None:
    typing.assert_type(1, Fabric)
"#,
            ),
        ],
    }
    .assert("assert_type resolves the alias target, not the alias name")
}

// ── cast is an escape hatch, but still a two-argument call ───────────────

#[test]
fn cast_takes_a_type_and_a_value() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`cast(typ, val)` is declared with two positional parameters; one argument \
                      does not bind `val`. The value's own type is deliberately unchecked — that \
                      is what makes `cast` an escape hatch — so the two-argument form is clean",
        rejected: r#"
import typing


def assay() -> None:
    typing.cast(int)
"#,
        accepted: r#"
import typing


def assay() -> None:
    typing.cast(int, "olivine")
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import cast as coerce
from builtins import int as Whole


def assay() -> None:
    coerce(Whole)
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def assay() -> None:
    typing.cast(builtins.int)
"#,
            ),
            renamed(
                r#"
import typing


def probe() -> None:
    typing.cast(int)
"#,
            ),
            reformatted(
                "
import typing

def assay() -> None:
        typing.cast(
            int   # <- no value argument
        )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import cast as coerce
from builtins import int as Whole


def assay() -> None:
    coerce(Whole, "olivine")
"#,
            ),
            reformatted(
                "
import typing

def assay() -> None:
        typing.cast(
            int ,
            'olivine' ,
        )
",
            ),
        ],
    }
    .assert("cast takes a type and a value")
}

// ── reveal_type: invariance only, since it always reports ────────────────

#[test]
fn reveal_type_reports_identically_however_it_is_spelled()
-> Result<(), Box<dyn std::error::Error>> {
    // `reveal_type` always emits, so neither directed leg applies; the property
    // under test is purely Family A — the same program respelled must produce
    // the same number of revelations, no more and no fewer.
    let canonical = r#"
import typing


def assay() -> None:
    quartzite = 1
    typing.reveal_type(quartzite)
"#;

    assert_invariant(
        "reveal_type reports identically however it is spelled",
        canonical,
        &[
            aliased(
                r#"
from typing import reveal_type as show


def assay() -> None:
    quartzite = 1
    show(quartzite)
"#,
            ),
            renamed(
                r#"
import typing


def probe() -> None:
    obsidian = 1
    typing.reveal_type(obsidian)
"#,
            ),
            reformatted(
                "
import typing

def assay() -> None:

        quartzite = 1

        typing.reveal_type(
            quartzite   # one revelation, however it is laid out
        )
",
            ),
        ],
    )
}
