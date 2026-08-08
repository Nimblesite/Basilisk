//! `LiteralString` provenance and `Literal` flattening — the normative rules the
//! spec states as *constructions*, not as bare assignability.
//! [PERMTEST-FAMILY-A] / [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! The spec defines LiteralString compositionally: concatenation, `join` and
//! f-strings each propagate literal-ness only when every constituent is itself
//! literal. That makes provenance the thing under test — the same f-string shape
//! is legal or illegal depending on where its interpolated value came from, so
//! nothing about the expression's spelling can decide it.

use super::harness::{assert_invariant, aliased, import_form, reformatted, renamed, SpecObligation};

// ── f-strings propagate literal-ness from their interpolations ───────────

#[test]
fn an_f_string_is_literal_only_when_its_interpolations_are()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec states an f-string has type `LiteralString` if and only if its \
                      constituent expressions are literal strings; interpolating a plain `str` \
                      forfeits it, interpolating a literal keeps it",
        rejected: r#"
import typing


def weave(pattern: typing.LiteralString) -> None:
    return None


def loom(shuttle: str) -> None:
    weave(f"twill-{shuttle}")
"#,
        accepted: r#"
import typing


def weave(pattern: typing.LiteralString) -> None:
    return None


def loom() -> None:
    grade: typing.LiteralString = "fine"
    weave(f"twill-{grade}")
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import LiteralString as Fixed


def weave(pattern: Fixed) -> None:
    return None


def loom(shuttle: str) -> None:
    weave(f"twill-{shuttle}")
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def weave(pattern: typing.LiteralString) -> None:
    return None


def loom(shuttle: builtins.str) -> None:
    weave(f"twill-{shuttle}")
"#,
            ),
            renamed(
                r#"
import typing


def plait(motif: typing.LiteralString) -> None:
    return None


def frame(bobbin: str) -> None:
    plait(f"twill-{bobbin}")
"#,
            ),
            reformatted(
                "
import typing

def weave( pattern : typing.LiteralString ) -> None :
        return None

def loom( shuttle : str ) -> None :

        weave(
            f'twill-{shuttle}'   # <- interpolation of unknown provenance
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
    grade: Fixed = "fine"
    weave(f"twill-{grade}")
"#,
            ),
            renamed(
                r#"
import typing


def plait(motif: typing.LiteralString) -> None:
    return None


def frame() -> None:
    quality: typing.LiteralString = "fine"
    plait(f"twill-{quality}")
"#,
            ),
        ],
    }
    .assert("an f-string is literal only when its interpolations are")
}

// ── join propagates literal-ness from separator and elements ─────────────

#[test]
fn join_is_literal_only_when_separator_and_elements_are()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec states `sep.join(xs)` is `LiteralString` if `sep` is assignable to \
                      `LiteralString` and `xs` to `Iterable[LiteralString]`; a list holding a \
                      runtime `str` breaks the second condition",
        rejected: r#"
import typing


def weave(pattern: typing.LiteralString) -> None:
    return None


def loom(shuttle: str) -> None:
    weave("-".join(["twill", shuttle]))
"#,
        accepted: r#"
import typing


def weave(pattern: typing.LiteralString) -> None:
    return None


def loom() -> None:
    weave("-".join(["twill", "sateen"]))
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import LiteralString as Fixed


def weave(pattern: Fixed) -> None:
    return None


def loom(shuttle: str) -> None:
    weave("-".join(["twill", shuttle]))
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def weave(pattern: typing.LiteralString) -> None:
    return None


def loom(shuttle: builtins.str) -> None:
    weave("-".join(["twill", shuttle]))
"#,
            ),
            renamed(
                r#"
import typing


def plait(motif: typing.LiteralString) -> None:
    return None


def frame(bobbin: str) -> None:
    plait("-".join(["twill", bobbin]))
"#,
            ),
            reformatted(
                "
import typing

def weave( pattern : typing.LiteralString ) -> None :
        return None

def loom( shuttle : str ) -> None :
        weave(
            '-'.join(
                [ 'twill' , shuttle ]   # <- one element is not literal
            )
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
    weave("-".join(["twill", "sateen"]))
"#,
            ),
            reformatted(
                "
import typing

def weave( pattern : typing.LiteralString ) -> None :
        return None

def loom() -> None :
        weave(
            '-'.join(
                [
                    'twill' ,
                    'sateen' ,
                ]
            )
        )
",
            ),
        ],
    }
    .assert("join is literal only when separator and elements are")
}

// ── nested Literal forms flatten to the same type ────────────────────────

#[test]
fn nested_literal_forms_are_equivalent_to_their_flattening()
-> Result<(), Box<dyn std::error::Error>> {
    // The spec states `Literal[Literal[Literal[1, 2, 3], "foo"], 5, None]` is
    // *exactly equivalent* to `Literal[1, 2, 3, "foo", 5, None]`, and to
    // `Literal[1, 2, 3, "foo", 5] | None`. Equivalent types must produce an
    // equivalent verdict, so this is Family A over the type expression itself
    // rather than over the surrounding source.
    let canonical = r#"
import typing


def gauge(setting: typing.Literal[1, 2, 3, "sateen", 5, None]) -> None:
    return None


gauge("sateen")
gauge(2)
gauge(None)
"#;

    assert_invariant(
        "nested Literal forms are equivalent to their flattening",
        canonical,
        &[
            reformatted(
                r#"
import typing


def gauge(
    setting: typing.Literal[typing.Literal[typing.Literal[1, 2, 3], "sateen"], 5, None],
) -> None:
    return None


gauge("sateen")
gauge(2)
gauge(None)
"#,
            ),
            aliased(
                r#"
from typing import Literal as Exact


def gauge(setting: Exact[Exact[Exact[1, 2, 3], "sateen"], 5] | None) -> None:
    return None


gauge("sateen")
gauge(2)
gauge(None)
"#,
            ),
            renamed(
                r#"
import typing


def caliper(notch: typing.Literal[1, 2, 3, "sateen", 5, None]) -> None:
    return None


caliper("sateen")
caliper(2)
caliper(None)
"#,
            ),
        ],
    )
}

// ── a non-member is still rejected through the nested spelling ───────────

#[test]
fn the_nested_literal_spelling_rejects_a_non_member()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "flattening is an equivalence, not a widening: a value outside the flattened \
                      member set is rejected whichever nesting spells the type",
        rejected: r#"
import typing


def gauge(setting: typing.Literal[typing.Literal[1, 2], "sateen"]) -> None:
    return None


gauge(7)
"#,
        accepted: r#"
import typing


def gauge(setting: typing.Literal[typing.Literal[1, 2], "sateen"]) -> None:
    return None


gauge(2)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Literal as Exact


def gauge(setting: Exact[Exact[1, 2], "sateen"]) -> None:
    return None


gauge(7)
"#,
            ),
            import_form(
                r#"
import typing_extensions


def gauge(
    setting: typing_extensions.Literal[typing_extensions.Literal[1, 2], "sateen"],
) -> None:
    return None


gauge(7)
"#,
            ),
            renamed(
                r#"
import typing


def caliper(notch: typing.Literal[typing.Literal[1, 2], "sateen"]) -> None:
    return None


caliper(7)
"#,
            ),
            reformatted(
                "
import typing

def gauge(
    setting : typing.Literal[
        typing.Literal[ 1 , 2 ] ,
        'sateen' ,
    ]
) -> None :
        return None

gauge( 7 )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Literal as Exact


def gauge(setting: Exact[Exact[1, 2], "sateen"]) -> None:
    return None


gauge(2)
"#,
            ),
            renamed(
                r#"
import typing


def caliper(notch: typing.Literal[typing.Literal[1, 2], "sateen"]) -> None:
    return None


caliper(2)
"#,
            ),
        ],
    }
    .assert("the nested Literal spelling rejects a non-member")
}
