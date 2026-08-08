//! `TypeIs` and `TypeGuard` narrowing (PEP 742 / PEP 647).
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! PEP 742 states two obligations a checker must enforce:
//!
//! * "Type narrowing functions must accept at least one positional argument."
//! * "The return type `R` must be consistent with `I`. The type checker should
//!   emit an error if this condition is not met."
//!
//! and one it must *implement*: unlike `TypeGuard`, `TypeIs` narrows the
//! negative branch — "when a `TypeIs` function returns `False`, type checkers
//! can narrow the type of the variable to exclude the `TypeIs` type", where
//! `TypeGuard` "cannot apply any additional narrowing".
//!
//! That last difference is asserted here with `assert_type` on both branches, so
//! the two forms must produce *different* verdicts on otherwise identical code.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── the narrowed type must be consistent with the parameter ──────────────

#[test]
fn a_typeis_return_must_be_consistent_with_its_parameter()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 742 requires the narrowed type `R` to be consistent with the parameter \
                      type `I` and says the checker should emit an error otherwise; `str` is not \
                      consistent with `int`",
        rejected: r#"
import typing


def is_measured(sample: int) -> typing.TypeIs[str]:
    return True
"#,
        accepted: r#"
import typing


def is_measured(sample: object) -> typing.TypeIs[int]:
    return True
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeIs as Narrows
from builtins import str as Text


def is_measured(sample: int) -> Narrows[Text]:
    return True
"#,
            ),
            import_form(
                r#"
import typing_extensions


def is_measured(sample: int) -> typing_extensions.TypeIs[str]:
    return True
"#,
            ),
            renamed(
                r#"
import typing


def is_sparged(wort: int) -> typing.TypeIs[str]:
    return True
"#,
            ),
            reformatted(
                "
import typing

def is_measured(
    sample : int ,
) -> typing.TypeIs[
    str   # <- not consistent with the declared parameter type
] :
        return True
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeIs as Narrows
from builtins import object as Base


def is_measured(sample: Base) -> Narrows[int]:
    return True
"#,
            ),
            renamed(
                r#"
import typing


def is_sparged(wort: object) -> typing.TypeIs[int]:
    return True
"#,
            ),
        ],
    }
    .assert("a TypeIs return must be consistent with its parameter")
}

// ── a narrowing function needs something to narrow ───────────────────────

#[test]
fn a_typeis_function_needs_a_positional_parameter() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 742 requires type narrowing functions to accept at least one positional \
                      argument, since narrowing is applied to the first positional argument; a \
                      keyword-only parameter is not positional",
        rejected: r#"
import typing


def is_measured(*, sample: object) -> typing.TypeIs[int]:
    return True
"#,
        accepted: r#"
import typing


def is_measured(sample: object) -> typing.TypeIs[int]:
    return True
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeIs as Narrows


def is_measured(*, sample: object) -> Narrows[int]:
    return True
"#,
            ),
            import_form(
                r#"
import typing_extensions


def is_measured(*, sample: object) -> typing_extensions.TypeIs[int]:
    return True
"#,
            ),
            renamed(
                r#"
import typing


def is_sparged(*, wort: object) -> typing.TypeIs[int]:
    return True
"#,
            ),
            reformatted(
                "
import typing

def is_measured(
    * ,
    sample : object ,   # <- keyword-only, so nothing to narrow
) -> typing.TypeIs[ int ] :
        return True
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeIs as Narrows
from builtins import object as Base


def is_measured(sample: Base) -> Narrows[int]:
    return True
"#,
            ),
            renamed(
                r#"
import typing


def is_sparged(wort: object) -> typing.TypeIs[int]:
    return True
"#,
            ),
        ],
    }
    .assert("a TypeIs function needs a positional parameter")
}

// ── TypeIs narrows the negative branch ───────────────────────────────────

#[test]
fn typeis_narrows_the_negative_branch() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 742: when a `TypeIs` function returns `False` the checker narrows the \
                      variable to *exclude* the narrowed type, so the else branch of an \
                      `int | str` subject is `str`, not the original union",
        rejected: r#"
import typing


def is_whole(sample: int | str) -> typing.TypeIs[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    typing.assert_type(sample, int | str)
"#,
        accepted: r#"
import typing


def is_whole(sample: int | str) -> typing.TypeIs[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    typing.assert_type(sample, str)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeIs as Narrows
from typing import assert_type as claim


def is_whole(sample: int | str) -> Narrows[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    claim(sample, int | str)
"#,
            ),
            import_form(
                r#"
import typing
import typing_extensions


def is_whole(sample: int | str) -> typing_extensions.TypeIs[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    typing.assert_type(sample, int | str)
"#,
            ),
            renamed(
                r#"
import typing


def is_settled(trub: int | str) -> typing.TypeIs[int]:
    return True


def rack(trub: int | str) -> None:
    if is_settled(trub):
        return None
    typing.assert_type(trub, int | str)
"#,
            ),
            reformatted(
                "
import typing

def is_whole( sample : int | str ) -> typing.TypeIs[ int ] :
        return True

def brew( sample : int | str ) -> None :

        if is_whole( sample ):
            return None

        # TypeIs excludes int here, so the union is stale
        typing.assert_type( sample , int | str )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeIs as Narrows
from typing import assert_type as claim
from builtins import str as Text


def is_whole(sample: int | str) -> Narrows[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    claim(sample, Text)
"#,
            ),
            renamed(
                r#"
import typing


def is_settled(trub: int | str) -> typing.TypeIs[int]:
    return True


def rack(trub: int | str) -> None:
    if is_settled(trub):
        return None
    typing.assert_type(trub, str)
"#,
            ),
        ],
    }
    .assert("TypeIs narrows the negative branch")
}

// ── TypeGuard does not ───────────────────────────────────────────────────

#[test]
fn typeguard_leaves_the_negative_branch_unnarrowed() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 742 contrasts `TypeGuard`, which \"cannot apply any additional \
                      narrowing\" on the false path; the else branch keeps the declared union. \
                      This is the same program as the TypeIs case with one symbol changed, so a \
                      checker that treats the two forms alike must fail one of the two tests",
        rejected: r#"
import typing


def is_whole(sample: int | str) -> typing.TypeGuard[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    typing.assert_type(sample, str)
"#,
        accepted: r#"
import typing


def is_whole(sample: int | str) -> typing.TypeGuard[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    typing.assert_type(sample, int | str)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeGuard as Asserts
from typing import assert_type as claim
from builtins import str as Text


def is_whole(sample: int | str) -> Asserts[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    claim(sample, Text)
"#,
            ),
            import_form(
                r#"
import typing
import typing_extensions


def is_whole(sample: int | str) -> typing_extensions.TypeGuard[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    typing.assert_type(sample, str)
"#,
            ),
            renamed(
                r#"
import typing


def is_settled(trub: int | str) -> typing.TypeGuard[int]:
    return True


def rack(trub: int | str) -> None:
    if is_settled(trub):
        return None
    typing.assert_type(trub, str)
"#,
            ),
            reformatted(
                "
import typing

def is_whole( sample : int | str ) -> typing.TypeGuard[ int ] :
        return True

def brew( sample : int | str ) -> None :
        if is_whole( sample ):
            return None

        # TypeGuard narrows nothing here, so str is wrong
        typing.assert_type( sample , str )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeGuard as Asserts
from typing import assert_type as claim


def is_whole(sample: int | str) -> Asserts[int]:
    return True


def brew(sample: int | str) -> None:
    if is_whole(sample):
        return None
    claim(sample, int | str)
"#,
            ),
            renamed(
                r#"
import typing


def is_settled(trub: int | str) -> typing.TypeGuard[int]:
    return True


def rack(trub: int | str) -> None:
    if is_settled(trub):
        return None
    typing.assert_type(trub, int | str)
"#,
            ),
        ],
    }
    .assert("TypeGuard leaves the negative branch unnarrowed")
}
