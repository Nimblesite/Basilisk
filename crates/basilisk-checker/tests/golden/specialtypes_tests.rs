//! `Never` / `NoReturn` and the numeric tower — the spec's two special-cased
//! assignability rules. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! Both are pure type-relation obligations with no syntactic tell. The numeric
//! tower in particular is *directional*: `int` is acceptable where `float` is
//! expected and `float` where `complex` is expected, never the reverse. A rule
//! that pattern-matches on the pair of names without honouring the direction
//! passes half of these and fails the other half.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── a `Never`-returning function must not return ─────────────────────────

#[test]
fn a_never_returning_function_must_not_return_normally()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Never` is the empty type: a function annotated `-> Never` has no possible \
                      return value, so a body that can fall off its end is ill-typed",
        rejected: r#"
import typing


def seize() -> typing.Never:
    escapement = 1
"#,
        accepted: r#"
import typing


def seize() -> typing.Never:
    raise RuntimeError("mainspring parted")
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Never as Bottom


def seize() -> Bottom:
    escapement = 1
"#,
            ),
            import_form(
                r#"
import typing_extensions


def seize() -> typing_extensions.Never:
    escapement = 1
"#,
            ),
            renamed(
                r#"
import typing


def arrest() -> typing.Never:
    detent = 1
"#,
            ),
            reformatted(
                "
import typing

def seize() -> typing.Never :

        escapement = 1   # <- falls off the end of an uninhabited return type
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Never as Bottom


def seize() -> Bottom:
    raise RuntimeError("mainspring parted")
"#,
            ),
            renamed(
                r#"
import typing


def arrest() -> typing.Never:
    raise RuntimeError("hairspring fouled")
"#,
            ),
        ],
    }
    .assert("a Never-returning function must not return normally")
}

// ── `NoReturn` is the same obligation under its older spelling ───────────

#[test]
fn noreturn_carries_the_same_obligation_as_never() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`NoReturn` and `Never` denote the same type; the older spelling carries the \
                      identical obligation, so a checker keyed to one name and not the other is \
                      wrong on half of real code",
        rejected: r#"
import typing


def stall() -> typing.NoReturn:
    return None
"#,
        accepted: r#"
import typing


def stall() -> typing.NoReturn:
    raise RuntimeError("verge seized")
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import NoReturn as Bottom


def stall() -> Bottom:
    return None
"#,
            ),
            import_form(
                r#"
import typing_extensions


def stall() -> typing_extensions.NoReturn:
    return None
"#,
            ),
            renamed(
                r#"
import typing


def jam() -> typing.NoReturn:
    return None
"#,
            ),
            reformatted(
                "
import typing

def stall() -> typing.NoReturn:

        return (
            None   # <- returning at all is the error
        )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import NoReturn as Bottom


def stall() -> Bottom:
    raise RuntimeError("verge seized")
"#,
            ),
            renamed(
                r#"
import typing


def jam() -> typing.NoReturn:
    raise RuntimeError("pallet chipped")
"#,
            ),
        ],
    }
    .assert("NoReturn carries the same obligation as Never")
}

// ── `Never` flows into anything ──────────────────────────────────────────

#[test]
fn never_is_assignable_to_every_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Never` is a subtype of every type, so a `Never`-typed expression satisfies \
                      any annotation; nothing is assignable *to* `Never` in the other direction",
        rejected: r#"
import typing


def wind(tension: typing.Never) -> None:
    return None


wind(1)
"#,
        accepted: r#"
import typing


def sink() -> typing.Never:
    raise RuntimeError("fusee stripped")


def hold(gauge: int) -> None:
    return None


hold(sink())
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Never as Bottom


def wind(tension: Bottom) -> None:
    return None


wind(1)
"#,
            ),
            import_form(
                r#"
import typing
import builtins


def wind(tension: typing.Never) -> None:
    return None


spring: builtins.int = 1
wind(spring)
"#,
            ),
            renamed(
                r#"
import typing


def crank(load: typing.Never) -> None:
    return None


crank(1)
"#,
            ),
            reformatted(
                "
import typing

def wind( tension : typing.Never ) -> None :
        return None

wind(
    1   # <- nothing inhabits Never, so no argument can be valid
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Never as Bottom


def sink() -> Bottom:
    raise RuntimeError("fusee stripped")


def hold(gauge: int) -> None:
    return None


hold(sink())
"#,
            ),
            renamed(
                r#"
import typing


def drain() -> typing.Never:
    raise RuntimeError("barrel burst")


def clamp(reading: int) -> None:
    return None


clamp(drain())
"#,
            ),
        ],
    }
    .assert("Never is assignable to every type")
}

// ── the numeric tower promotes upward only ───────────────────────────────

#[test]
fn int_promotes_to_float_but_float_does_not_demote() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec's numeric tower makes `int` acceptable where `float` is expected; \
                      the converse does not hold, so a `float` argument to an `int` parameter is \
                      ill-typed",
        rejected: r#"
def wind(tension: int) -> None:
    return None


wind(1.5)
"#,
        accepted: r#"
def wind(tension: float) -> None:
    return None


wind(1)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def wind(tension: Whole) -> None:
    return None


wind(1.5)
"#,
            ),
            import_form(
                r#"
import builtins


def wind(tension: builtins.int) -> None:
    return None


wind(1.5)
"#,
            ),
            renamed(
                r#"
def crank(load: int) -> None:
    return None


crank(1.5)
"#,
            ),
            reformatted(
                "
def wind( tension : int ) -> None :
        return None

wind(
    1.5   # <- float does not demote to int
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import float as Real


def wind(tension: Real) -> None:
    return None


wind(1)
"#,
            ),
            import_form(
                r#"
import builtins


def wind(tension: builtins.float) -> None:
    return None


wind(1)
"#,
            ),
            renamed(
                r#"
def crank(load: float) -> None:
    return None


crank(1)
"#,
            ),
        ],
    }
    .assert("int promotes to float but float does not demote")
}

#[test]
fn float_promotes_to_complex_but_complex_does_not_demote()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the tower extends one more step: `float` is acceptable where `complex` is \
                      expected, and `complex` is acceptable nowhere below it",
        rejected: r#"
def balance(reading: float) -> None:
    return None


balance(1j)
"#,
        accepted: r#"
def balance(reading: complex) -> None:
    return None


balance(1.5)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import float as Real


def balance(reading: Real) -> None:
    return None


balance(1j)
"#,
            ),
            import_form(
                r#"
import builtins


def balance(reading: builtins.float) -> None:
    return None


balance(1j)
"#,
            ),
            renamed(
                r#"
def poise(sample: float) -> None:
    return None


poise(1j)
"#,
            ),
            reformatted(
                "
def balance( reading : float ) -> None :

        return None

balance(
    1j   # <- complex sits above float, not below
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import complex as Phasor


def balance(reading: Phasor) -> None:
    return None


balance(1.5)
"#,
            ),
            renamed(
                r#"
def poise(sample: complex) -> None:
    return None


poise(1.5)
"#,
            ),
        ],
    }
    .assert("float promotes to complex but complex does not demote")
}

// ── promotion does not reach `bool` from `int` ───────────────────────────

#[test]
fn int_does_not_narrow_to_bool() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`bool` is a nominal subclass of `int`, so `bool` flows into `int` by \
                      ordinary subtyping. PEP 484's numeric tower covers only int/float/complex \
                      and grants nothing here, so an `int` argument to a `bool` parameter is \
                      ill-typed — the tower must not be over-applied",
        rejected: r#"
def latch(engaged: bool) -> None:
    return None


latch(1)
"#,
        accepted: r#"
def latch(engaged: int) -> None:
    return None


latch(True)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import bool as Flag


def latch(engaged: Flag) -> None:
    return None


latch(1)
"#,
            ),
            import_form(
                r#"
import builtins


def latch(engaged: builtins.bool) -> None:
    return None


latch(1)
"#,
            ),
            renamed(
                r#"
def catch(seated: bool) -> None:
    return None


catch(1)
"#,
            ),
            reformatted(
                "
def latch( engaged : bool ) -> None :
        return None

latch(
    1   # <- int is the supertype here, not the subtype
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def latch(engaged: Whole) -> None:
    return None


latch(True)
"#,
            ),
            renamed(
                r#"
def catch(seated: int) -> None:
    return None


catch(True)
"#,
            ),
        ],
    }
    .assert("int does not narrow to bool")
}
