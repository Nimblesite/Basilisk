//! `cast` and `reveal_type` — the remaining static-introspection directives.
//! [PERMTEST-FAMILY-A] / [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! Split from [`directives_tests`](directives_tests.rs), which carries
//! `assert_type`. Both files share the same authoring hazard documented there:
//! a directive's subject must have a type the spec actually pins down.
//!
//! `cast` is the spec's escape hatch — it deliberately does *not* check the
//! value against the target type — so the only obligation it carries is its own
//! signature. `reveal_type` always emits, so neither directed leg applies to it
//! and its property is purely Family A.

use super::harness::{assert_invariant, aliased, import_form, reformatted, renamed, SpecObligation};

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
