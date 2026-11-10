//! `assert_type` — the spec's sharpest static-introspection directive.
//! [PERMTEST-FAMILY-A] / [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `cast` and `reveal_type` are in [`directives_more_tests`](directives_more_tests.rs).
//!
//! These are the sharpest permutation targets in the whole spec. Every one takes
//! a *type expression* as an argument, so a checker that inspects the argument's
//! source text rather than its resolved type passes the canonical spelling and
//! fails every respelling of it.
//!
//! `assert_type` is sharper still: it demands *equivalence*, not assignability.
//! An alias, a `from … import … as`, and a dotted attribute path must all compare
//! equal to the type they name — and two programs that differ only in what an
//! alias points at must get opposite verdicts from byte-identical call sites.
//!
//! **Authoring hazard.** Because `assert_type` demands equivalence, its subject
//! must have a type the spec actually pins down. It must never be an unannotated
//! literal binding: the spec's type-inference section states "No particular
//! strategy is mandated", so `feldspar = 1` may lawfully infer `Literal[1]` *or*
//! `int`, and `Literal[1]` is not equivalent to `int`. An `assert_type(feldspar,
//! int)` fixture built on that would demand one lawful inference strategy and
//! call the other a bug. Every subject below therefore takes its type from a
//! *declared* source — an annotated return type or an annotated binding — which
//! the spec does fix.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── assert_type compares the inferred type ───────────────────────────────

#[test]
fn assert_type_rejects_a_mismatched_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`assert_type(v, T)` is an error unless the inferred type of `v` is \
                      *equivalent* to `T`. The subject is a call to a function declared \
                      `-> int`, so the spec fixes its type at `int` regardless of the \
                      checker's literal-inference strategy — asserting `str` is an error \
                      and asserting `int` is not",
        rejected: r#"
from typing import assert_type as verify_static_shape


def quarry() -> int:
    return 1


def assay() -> None:
    feldspar = quarry()
    verify_static_shape(feldspar, str)
"#,
        accepted: r#"
from typing import assert_type as verify_static_shape


def quarry() -> int:
    return 1


def assay() -> None:
    feldspar = quarry()
    verify_static_shape(feldspar, int)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import str as Text


def quarry() -> int:
    return 1


def assay() -> None:
    feldspar = quarry()
    claim(feldspar, Text)
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def quarry() -> builtins.int:
    return 1


def assay() -> None:
    feldspar = quarry()
    typing.assert_type(feldspar, builtins.str)
"#,
            ),
            renamed(
                r#"
import typing


def delve() -> int:
    return 1


def probe() -> None:
    gabbro = delve()
    typing.assert_type(gabbro, str)
"#,
            ),
            reformatted(
                "
import typing

def quarry() -> int :
        return 1

def assay() -> None:

        feldspar = quarry()
        typing.assert_type(
            feldspar ,
            str ,   # <- declared return int, asserted str
        )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import int as Whole


def quarry() -> Whole:
    return 1


def assay() -> None:
    feldspar = quarry()
    claim(feldspar, Whole)
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def quarry() -> builtins.int:
    return 1


def assay() -> None:
    feldspar = quarry()
    typing.assert_type(feldspar, builtins.int)
"#,
            ),
            renamed(
                r#"
import typing


def delve() -> int:
    return 1


def probe() -> None:
    gabbro = delve()
    typing.assert_type(gabbro, int)
"#,
            ),
        ],
    }
    .assert_by(
        "assert_type rejects a mismatched type",
        "directives_assert_type_2",
    )
}

// ── a declared return type fixes the call expression's type ──────────────

#[test]
fn assert_type_uses_a_declared_return_type_not_the_runtime_class(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the result of a call declared `-> object` has static type `object`; \
                      `assert_type` therefore rejects `int` and accepts `object` without relying \
                      on an implementation-specific assignment-narrowing strategy",
        rejected: r#"
from typing import assert_type as verify_static_shape


def quarry_sample() -> object:
    return 1


def assay() -> None:
    basalt = quarry_sample()
    verify_static_shape(basalt, int)
"#,
        accepted: r#"
from typing import assert_type as verify_static_shape


def quarry_sample() -> object:
    return 1


def assay() -> None:
    basalt = quarry_sample()
    verify_static_shape(basalt, object)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import object as Base
from builtins import int as Whole


def quarry_sample() -> Base:
    return 1


def assay() -> None:
    basalt = quarry_sample()
    claim(basalt, Whole)
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def quarry_sample() -> builtins.object:
    return 1


def assay() -> None:
    basalt = quarry_sample()
    typing.assert_type(basalt, builtins.int)
"#,
            ),
            renamed(
                r#"
import typing


def extract_record() -> object:
    return 1


def probe() -> None:
    gneiss = extract_record()
    typing.assert_type(gneiss, int)
"#,
            ),
            reformatted(
                "
import typing

def quarry_sample() -> object:
        return 1

def assay() -> None:
        basalt = quarry_sample()

        # declared return `object`; the runtime value is irrelevant
        typing.assert_type( basalt , int )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import object as Base


def quarry_sample() -> Base:
    return 1


def assay() -> None:
    basalt = quarry_sample()
    claim(basalt, Base)
"#,
            ),
            renamed(
                r#"
import typing


def extract_record() -> object:
    return 1


def probe() -> None:
    gneiss = extract_record()
    typing.assert_type(gneiss, object)
"#,
            ),
        ],
    }
    .assert_by(
        "assert_type uses a declared return type, not the runtime class",
        "directives_assert_type_2",
    )
}

// ── alias identity: same call site, opposite verdicts ────────────────────

#[test]
fn assert_type_resolves_the_alias_target_not_the_alias_name(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a type alias is transparent: `Grain = str` makes `assert_type(quarry(), \
                      Grain)` the same error as asserting `str` outright, while `Grain = int` \
                      makes it correct. The subject is a call declared `-> int`, so its type is \
                      fixed by the spec rather than by the checker's literal-inference strategy, \
                      and the assertion site is byte-identical in both programs — only the \
                      resolved alias target can decide the verdict",
        rejected: r#"
from typing import assert_type as verify_static_shape

Grain = str


def quarry() -> int:
    return 1


def assay() -> None:
    verify_static_shape(quarry(), Grain)
"#,
        accepted: r#"
from typing import assert_type as verify_static_shape

Grain = int


def quarry() -> int:
    return 1


def assay() -> None:
    verify_static_shape(quarry(), Grain)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import str as Text

Grain = Text


def quarry() -> int:
    return 1


def assay() -> None:
    claim(quarry(), Grain)
"#,
            ),
            import_form(
                r#"
import typing
import builtins

Grain = builtins.str


def quarry() -> builtins.int:
    return 1


def assay() -> None:
    typing.assert_type(quarry(), Grain)
"#,
            ),
            renamed(
                r#"
import typing

Fabric = str


def delve() -> int:
    return 1


def probe() -> None:
    typing.assert_type(delve(), Fabric)
"#,
            ),
            reformatted(
                "
import typing

Grain = str   # <- alias points at str

def quarry() -> int :
        return 1

def assay() -> None:

        typing.assert_type( quarry() , Grain )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import assert_type as claim
from builtins import int as Whole

Grain = Whole


def quarry() -> Whole:
    return 1


def assay() -> None:
    claim(quarry(), Grain)
"#,
            ),
            import_form(
                r#"
import typing
import builtins

Grain = builtins.int


def quarry() -> builtins.int:
    return 1


def assay() -> None:
    typing.assert_type(quarry(), Grain)
"#,
            ),
            renamed(
                r#"
import typing

Fabric = int


def delve() -> int:
    return 1


def probe() -> None:
    typing.assert_type(delve(), Fabric)
"#,
            ),
        ],
    }
    .assert_by(
        "assert_type resolves the alias target, not the alias name",
        "directives_assert_type_2",
    )
}
