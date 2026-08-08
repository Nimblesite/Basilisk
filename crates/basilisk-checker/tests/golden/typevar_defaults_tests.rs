//! TypeVar defaults (PEP 696). [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! Three normative rules, each quoted in the obligation it drives:
//!
//! * "a type parameter with no `default` cannot follow one with a `default`
//!   value"
//! * "If both `bound` and `default` are passed `default` must be a subtype of
//!   `bound`"
//! * for constrained type variables, "the default needs to be one of the
//!   constraints" — membership, not assignability, so a subtype of a constraint
//!   is still an error
//!
//! Each is exercised in both the legacy `TypeVar(...)` spelling and the PEP 695
//! syntax, which are different surface forms of one semantic model.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── ordering: a bare parameter may not follow a defaulted one ────────────

#[test]
fn a_parameter_without_a_default_may_not_follow_one_with_a_default()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 696 states a type parameter with no default cannot follow one with a \
                      default value, mirroring ordinary parameter ordering; reversing the two \
                      makes the same pair legal",
        rejected: r#"
import typing

Tercel = typing.TypeVar("Tercel", default=int)
Eyas = typing.TypeVar("Eyas")


class Mews(typing.Generic[Tercel, Eyas]):
    pass
"#,
        accepted: r#"
import typing

Tercel = typing.TypeVar("Tercel", default=int)
Eyas = typing.TypeVar("Eyas")


class Mews(typing.Generic[Eyas, Tercel]):
    pass
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeVar as Var
from typing import Generic as Parametric

Tercel = Var("Tercel", default=int)
Eyas = Var("Eyas")


class Mews(Parametric[Tercel, Eyas]):
    pass
"#,
            ),
            import_form(
                r#"
import typing
import typing_extensions

Tercel = typing_extensions.TypeVar("Tercel", default=int)
Eyas = typing_extensions.TypeVar("Eyas")


class Mews(typing.Generic[Tercel, Eyas]):
    pass
"#,
            ),
            renamed(
                r#"
import typing

Hood = typing.TypeVar("Hood", default=int)
Creance = typing.TypeVar("Creance")


class Perch(typing.Generic[Hood, Creance]):
    pass
"#,
            ),
            reformatted(
                "
import typing

Tercel = typing.TypeVar( 'Tercel' , default = int )
Eyas   = typing.TypeVar( 'Eyas' )

class Mews(
    typing.Generic[
        Tercel ,
        Eyas ,   # <- bare parameter after a defaulted one
    ]
):
        pass
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeVar as Var
from typing import Generic as Parametric

Tercel = Var("Tercel", default=int)
Eyas = Var("Eyas")


class Mews(Parametric[Eyas, Tercel]):
    pass
"#,
            ),
            renamed(
                r#"
import typing

Hood = typing.TypeVar("Hood", default=int)
Creance = typing.TypeVar("Creance")


class Perch(typing.Generic[Creance, Hood]):
    pass
"#,
            ),
        ],
    }
    .assert("a parameter without a default may not follow one with a default")
}

// ── the same ordering rule under PEP 695 syntax ──────────────────────────

#[test]
fn the_ordering_rule_holds_under_pep_695_syntax() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the native type-parameter syntax carries the identical PEP 696 constraint; a \
                      checker that implements the rule only for the `TypeVar(...)` call form \
                      misses every modern declaration",
        rejected: r#"
class Mews[Tercel = int, Eyas]:
    pass
"#,
        accepted: r#"
class Mews[Eyas, Tercel = int]:
    pass
"#,
        rejected_variants: &[
            renamed(
                r#"
class Perch[Hood = int, Creance]:
    pass
"#,
            ),
            reformatted(
                "
class Mews[
    Tercel = int ,
    Eyas ,   # <- bare parameter after a defaulted one
]:
        pass
",
            ),
            aliased(
                r#"
from builtins import int as Whole


class Mews[Tercel = Whole, Eyas]:
    pass
"#,
            ),
            import_form(
                r#"
import builtins


class Mews[Tercel = builtins.int, Eyas]:
    pass
"#,
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
class Perch[Creance, Hood = int]:
    pass
"#,
            ),
            aliased(
                r#"
from builtins import int as Whole


class Mews[Eyas, Tercel = Whole]:
    pass
"#,
            ),
        ],
    }
    .assert("the ordering rule holds under PEP 695 syntax")
}

// ── a default must satisfy the bound ─────────────────────────────────────

#[test]
fn a_default_must_be_a_subtype_of_the_bound() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 696 states that if both `bound` and `default` are passed, the default \
                      must be a subtype of the bound; `str` is not a subtype of `int`, while \
                      `bool` is",
        rejected: r#"
import typing

Tercel = typing.TypeVar("Tercel", bound=int, default=str)


class Mews(typing.Generic[Tercel]):
    pass
"#,
        accepted: r#"
import typing

Tercel = typing.TypeVar("Tercel", bound=int, default=bool)


class Mews(typing.Generic[Tercel]):
    pass
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeVar as Var
from typing import Generic as Parametric
from builtins import str as Text

Tercel = Var("Tercel", bound=int, default=Text)


class Mews(Parametric[Tercel]):
    pass
"#,
            ),
            import_form(
                r#"
import typing
import builtins

Tercel = typing.TypeVar("Tercel", bound=builtins.int, default=builtins.str)


class Mews(typing.Generic[Tercel]):
    pass
"#,
            ),
            renamed(
                r#"
import typing

Hood = typing.TypeVar("Hood", bound=int, default=str)


class Perch(typing.Generic[Hood]):
    pass
"#,
            ),
            reformatted(
                "
import typing

Tercel = typing.TypeVar(
    'Tercel' ,
    bound   = int ,
    default = str ,   # <- outside the bound
)

class Mews( typing.Generic[ Tercel ] ):
        pass
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeVar as Var
from typing import Generic as Parametric
from builtins import bool as Flag

Tercel = Var("Tercel", bound=int, default=Flag)


class Mews(Parametric[Tercel]):
    pass
"#,
            ),
            renamed(
                r#"
import typing

Hood = typing.TypeVar("Hood", bound=int, default=bool)


class Perch(typing.Generic[Hood]):
    pass
"#,
            ),
        ],
    }
    .assert("a default must be a subtype of the bound")
}

// ── a constrained default must *be* one of the constraints ───────────────

#[test]
fn a_constrained_default_must_be_one_of_the_constraints()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "for a constrained type variable PEP 696 requires the default to be one of \
                      the constraints — membership, not assignability. `bool` is a subtype of the \
                      constraint `int` and is still rejected; `int` itself is accepted",
        rejected: r#"
import typing

Tercel = typing.TypeVar("Tercel", int, str, default=bool)


class Mews(typing.Generic[Tercel]):
    pass
"#,
        accepted: r#"
import typing

Tercel = typing.TypeVar("Tercel", int, str, default=int)


class Mews(typing.Generic[Tercel]):
    pass
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeVar as Var
from typing import Generic as Parametric
from builtins import bool as Flag

Tercel = Var("Tercel", int, str, default=Flag)


class Mews(Parametric[Tercel]):
    pass
"#,
            ),
            import_form(
                r#"
import typing
import builtins

Tercel = typing.TypeVar(
    "Tercel", builtins.int, builtins.str, default=builtins.bool
)


class Mews(typing.Generic[Tercel]):
    pass
"#,
            ),
            renamed(
                r#"
import typing

Hood = typing.TypeVar("Hood", int, str, default=bool)


class Perch(typing.Generic[Hood]):
    pass
"#,
            ),
            reformatted(
                "
import typing

Tercel = typing.TypeVar(
    'Tercel' ,
    int ,
    str ,
    default = bool ,   # <- a subtype of a constraint, but not a constraint
)

class Mews( typing.Generic[ Tercel ] ):
        pass
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeVar as Var
from typing import Generic as Parametric
from builtins import int as Whole

Tercel = Var("Tercel", Whole, str, default=Whole)


class Mews(Parametric[Tercel]):
    pass
"#,
            ),
            renamed(
                r#"
import typing

Hood = typing.TypeVar("Hood", int, str, default=int)


class Perch(typing.Generic[Hood]):
    pass
"#,
            ),
        ],
    }
    .assert("a constrained default must be one of the constraints")
}
