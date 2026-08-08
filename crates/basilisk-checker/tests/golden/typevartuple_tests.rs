//! `TypeVarTuple` and `Unpack` — variadic generics (PEP 646; typing spec,
//! *Generics* chapter, "Type variable tuples").
//! [PERMTEST-FAMILY-A] / [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! The normative sentences enforced here, quoted:
//!
//! * "only a single type variable tuple may appear in a type parameter list"
//!   (`class Array(Generic[*Ts1, *Ts2]): ...  # Error`)
//! * "As with `TypeVarTuple`s, only one unpacking may appear in a tuple"
//!   (`y: Tuple[int, *Tuple[int, ...], str, *Tuple[str, ...]]  # Error`)
//! * "type variable tuples must *always* be used unpacked (that is, prefixed by
//!   the star operator)" — with `*args` the sole place a bare `*Ts` annotation
//!   is admitted, and "annotating `*args` as being a plain type variable tuple
//!   instance is *not* allowed" (`def foo(*args: Ts): ...  # NOT valid`).
//! * "Unpacking an unbounded tuple preserves the unbounded tuple as it is …
//!   Using an unpacked unbounded tuple is equivalent to the PEP 484 behavior of
//!   `*args: int`, which accepts zero or more values of type `int`" — so the
//!   *single* unbounded unpacking is well-typed and is the accepted pair above.
//! * a type variable tuple binds "zero or more" type arguments, so a signature
//!   whose only variadic parameter is `*args: *Ts` is still satisfied by a call
//!   that supplies none — and the non-variadic parameters remain required.
//! * "The behavior of a `Callable` containing an unpacked item … is to treat
//!   the elements as if they were the type for `*args`", so `Callable[[*Ts],
//!   None]` linked to a `tuple[*Ts]` fixes that tuple's element order.
//!
//! Not asserted on, because PEP 646 leaves them open: the variance/bound
//! arguments it "leave[s] … to a future PEP", `**kwargs: *Ts` where it prefers
//! "to leave the ground fresh", and the "leeway" the acceptance note grants on
//! multiple unpackings in nested positions.
//!
//! **Surface-form pairing.** `*Ts` and `Unpack[Ts]` are two spellings of one
//! semantic model — the PEP introduces `Unpack` purely so the star form is
//! "back-portable to previous versions of Python". A variant that switches
//! between them is carried in the `import_form` class, since `Unpack` is
//! reached through an import and the star operator is not. A checker that
//! implements only one of the two spellings fails every test in this file.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── one type parameter list admits at most one variadic ──────────────────

#[test]
fn a_type_parameter_list_admits_at_most_one_type_variable_tuple(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 646 states that only a single type variable tuple may appear in a type \
                      parameter list, because multiple ones make it ambiguous which parameters \
                      bind to which; dropping the second variadic leaves the list legal",
        rejected: r#"
import typing

Bells = typing.TypeVarTuple("Bells")
Changes = typing.TypeVarTuple("Changes")


class Peal(typing.Generic[*Bells, *Changes]):
    pass
"#,
        accepted: r#"
import typing

Bells = typing.TypeVarTuple("Bells")
Changes = typing.TypeVarTuple("Changes")


class Peal(typing.Generic[*Bells]):
    pass
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeVarTuple as Variadic
from typing import Generic as Parametric

Bells = Variadic("Bells")
Changes = Variadic("Changes")


class Peal(Parametric[*Bells, *Changes]):
    pass
"#,
            ),
            import_form(
                r#"
import typing
import typing_extensions

Bells = typing_extensions.TypeVarTuple("Bells")
Changes = typing_extensions.TypeVarTuple("Changes")


class Peal(typing.Generic[typing.Unpack[Bells], typing.Unpack[Changes]]):
    pass
"#,
            ),
            renamed(
                r#"
import typing

Sallies = typing.TypeVarTuple("Sallies")
Strokes = typing.TypeVarTuple("Strokes")


class Belfry(typing.Generic[*Sallies, *Strokes]):
    pass
"#,
            ),
            reformatted(
                "
import typing

Bells   = typing.TypeVarTuple( 'Bells' )
Changes = typing.TypeVarTuple( 'Changes' )

class Peal(
    typing.Generic[
        *Bells ,
        *Changes ,   # <- a second variadic in one parameter list
    ]
):
        pass
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeVarTuple as Variadic
from typing import Generic as Parametric
from typing import Unpack as Splat

Bells = Variadic("Bells")
Changes = Variadic("Changes")


class Peal(Parametric[Splat[Bells]]):
    pass
"#,
            ),
            renamed(
                r#"
import typing

Sallies = typing.TypeVarTuple("Sallies")
Strokes = typing.TypeVarTuple("Strokes")


class Belfry(typing.Generic[*Sallies]):
    pass
"#,
            ),
        ],
    }
    .assert("a type parameter list admits at most one type variable tuple")
}

// ── one tuple type admits at most one unpacking ──────────────────────────

#[test]
fn a_tuple_type_admits_at_most_one_unpacking() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 646: \"only one unpacking may appear in a tuple\", giving \
                      `Tuple[int, *Tuple[int, ...], str, *Tuple[str, ...]]` as an error; the same \
                      type with a single unbounded unpacking is explicitly well-formed, since \
                      \"unpacking an unbounded tuple preserves the unbounded tuple as it is\"",
        rejected: r#"
def toll(chime: tuple[int, *tuple[int, ...], str, *tuple[str, ...]]) -> None:
    return None
"#,
        accepted: r#"
def toll(chime: tuple[int, *tuple[int, ...], str]) -> None:
    return None
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import tuple as Ring
from typing import Unpack as Splat


def toll(chime: Ring[int, Splat[Ring[int, ...]], str, Splat[Ring[str, ...]]]) -> None:
    return None
"#,
            ),
            import_form(
                r#"
import builtins
import typing


def toll(
    chime: builtins.tuple[
        int,
        typing.Unpack[builtins.tuple[int, ...]],
        str,
        typing.Unpack[builtins.tuple[str, ...]],
    ],
) -> None:
    return None
"#,
            ),
            renamed(
                r#"
def hunt(course: tuple[int, *tuple[int, ...], str, *tuple[str, ...]]) -> None:
    return None
"#,
            ),
            reformatted(
                "
def toll(
    chime : tuple[
        int ,
        *tuple[ int , ... ] ,
        str ,
        *tuple[ str , ... ] ,   # <- a second unpacking in one tuple type
    ] ,
) -> None :
        return None
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import tuple as Ring
from typing import Unpack as Splat


def toll(chime: Ring[int, Splat[Ring[int, ...]], str]) -> None:
    return None
"#,
            ),
            renamed(
                r#"
def hunt(course: tuple[int, *tuple[int, ...], str]) -> None:
    return None
"#,
            ),
            import_form(
                r#"
import builtins as core_shapes
import typing as type_forms


def toll(
    chime: core_shapes.tuple[
        int,
        type_forms.Unpack[core_shapes.tuple[int, ...]],
        str,
    ],
) -> None:
    return None
"#,
            ),
            reformatted(
                "
from builtins import tuple as Ring

def toll(
    chime : Ring[
        int ,
        *Ring[
            int ,
            ... ,
        ] ,
        str ,
    ] ,
) -> None :
        return None
",
            ),
        ],
    }
    .assert_by(
        "a tuple type admits at most one unpacking",
        "generics_typevartuple_specialization",
    )
}

// ── a variadic may never be used packed ──────────────────────────────────
//
// Alias-target discrimination: the two programs' `def` lines are byte-identical
// and only the factory that produced `Sallie` differs. `*args: T` for a plain
// `TypeVar` is ordinary PEP 484; `*args: Ts` for a variadic is "NOT valid".

#[test]
fn a_type_variable_tuple_may_never_appear_in_packed_form() -> Result<(), Box<dyn std::error::Error>>
{
    SpecObligation {
        spec_reason: "PEP 646 requires that \"type variable tuples must always be used unpacked \
                      (that is, prefixed by the star operator)\" and gives `def foo(*args: Ts)` as \
                      NOT valid; the identical signature over a plain `TypeVar` is the PEP 484 \
                      homogeneous `*args` form and is well-typed",
        rejected: r#"
import typing

Sallie = typing.TypeVarTuple("Sallie")


def peal(clapper: Sallie, *ropes: Sallie) -> Sallie:
    return clapper
"#,
        accepted: r#"
import typing

Sallie = typing.TypeVar("Sallie")


def peal(clapper: Sallie, *ropes: Sallie) -> Sallie:
    return clapper
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeVarTuple as Variadic

Sallie = Variadic("Sallie")


def peal(clapper: Sallie, *ropes: Sallie) -> Sallie:
    return clapper
"#,
            ),
            import_form(
                r#"
import typing_extensions

Sallie = typing_extensions.TypeVarTuple("Sallie")


def peal(clapper: Sallie, *ropes: Sallie) -> Sallie:
    return clapper
"#,
            ),
            renamed(
                r#"
import typing

Garter = typing.TypeVarTuple("Garter")


def hunt(stay: Garter, *wheels: Garter) -> Garter:
    return stay
"#,
            ),
            reformatted(
                "
import typing

Sallie = typing.TypeVarTuple( 'Sallie' )

def peal(
    clapper : Sallie ,   # <- a variadic used packed
    *ropes  : Sallie ,
) -> Sallie :
        return clapper
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeVar as Scalar

Sallie = Scalar("Sallie")


def peal(clapper: Sallie, *ropes: Sallie) -> Sallie:
    return clapper
"#,
            ),
            renamed(
                r#"
import typing

Garter = typing.TypeVar("Garter")


def hunt(stay: Garter, *wheels: Garter) -> Garter:
    return stay
"#,
            ),
            import_form(
                r#"
import typing_extensions as parameter_forms

Sallie = parameter_forms.TypeVar("Sallie")


def peal(clapper: Sallie, *ropes: Sallie) -> Sallie:
    return clapper
"#,
            ),
            reformatted(
                "
from typing import TypeVar as Scalar

Sallie = Scalar(
    'Sallie' ,
)

def peal(
    clapper : Sallie ,
    *ropes  : Sallie ,
) -> Sallie :
        return clapper
",
            ),
        ],
    }
    .assert_by(
        "a type variable tuple may never appear in packed form",
        "generics_typevartuple_basic_2",
    )
}

// ── an unbounded unpack cannot consume the fixed neighbours ─────────────

#[test]
fn an_unbounded_unpack_does_not_remove_fixed_type_arguments(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 646 states that an unpacked unbounded tuple may bind zero or more type \
                      arguments; the fixed prefix and suffix surrounding it still require their \
                      own arguments, so a one-argument specialization cannot satisfy two fixed \
                      positions while a two-argument specialization can",
        rejected: r#"
from typing import Any as UnknownValue
from typing import Generic as Shape
from typing import TypeVarTuple as DimensionPack

Dimensions = DimensionPack("Dimensions")


class Array(Shape[*Dimensions]):
    pass


def consume(value: Array[int, *tuple[UnknownValue, ...], str]) -> None:
    return None


def relay(value: Array[int]) -> None:
    consume(value)
"#,
        accepted: r#"
from typing import Any as UnknownValue
from typing import Generic as Shape
from typing import TypeVarTuple as DimensionPack

Dimensions = DimensionPack("Dimensions")


class Array(Shape[*Dimensions]):
    pass


def consume(value: Array[int, *tuple[UnknownValue, ...], str]) -> None:
    return None


def relay(value: Array[int, str]) -> None:
    consume(value)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import tuple as SequenceShape
from typing import Any as UnknownValue
from typing import Generic as Shape
from typing import TypeVarTuple as DimensionPack

Dimensions = DimensionPack("Dimensions")


class Array(Shape[*Dimensions]):
    pass


def consume(value: Array[int, *SequenceShape[UnknownValue, ...], str]) -> None:
    return None


def relay(value: Array[int]) -> None:
    consume(value)
"#,
            ),
            import_form(
                r#"
import builtins as core_shapes
import typing_extensions as parameter_forms
from typing import Any as UnknownValue
from typing import Unpack as Expand

Dimensions = parameter_forms.TypeVarTuple("Dimensions")


class Array(parameter_forms.Generic[Expand[Dimensions]]):
    pass


def consume(
    value: Array[int, Expand[core_shapes.tuple[UnknownValue, ...]], str],
) -> None:
    return None


def relay(value: Array[int]) -> None:
    consume(value)
"#,
            ),
            renamed(
                r#"
from typing import Any as Opaque
from typing import Generic as Parametric
from typing import TypeVarTuple as AxisBundle

Axes = AxisBundle("Axes")


class Lattice(Parametric[*Axes]):
    pass


def inspect(sample: Lattice[bytes, *tuple[Opaque, ...], float]) -> None:
    return None


def forward(sample: Lattice[bytes]) -> None:
    inspect(sample)
"#,
            ),
            reformatted(
                "
from typing import Any as UnknownValue, Generic as Shape, TypeVarTuple as DimensionPack

Dimensions = DimensionPack( 'Dimensions' )

class Array( Shape[ *Dimensions ] ):
        pass

def consume(
    value : Array[
        int ,
        * tuple[
            UnknownValue ,
            ... ,
        ] ,
        str ,
    ] ,
) -> None :
        return None

def relay( value : Array[ int ] ) -> None :
        consume( value )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import tuple as SequenceShape
from typing import Any as UnknownValue
from typing import Generic as Shape
from typing import TypeVarTuple as DimensionPack

Dimensions = DimensionPack("Dimensions")


class Array(Shape[*Dimensions]):
    pass


def consume(value: Array[int, *SequenceShape[UnknownValue, ...], str]) -> None:
    return None


def relay(value: Array[int, str]) -> None:
    consume(value)
"#,
            ),
            import_form(
                r#"
import builtins as core_shapes
import typing_extensions as parameter_forms
from typing import Any as UnknownValue
from typing import Unpack as Expand

Dimensions = parameter_forms.TypeVarTuple("Dimensions")


class Array(parameter_forms.Generic[Expand[Dimensions]]):
    pass


def consume(
    value: Array[int, Expand[core_shapes.tuple[UnknownValue, ...]], str],
) -> None:
    return None


def relay(value: Array[int, str]) -> None:
    consume(value)
"#,
            ),
            renamed(
                r#"
from typing import Any as Opaque
from typing import Generic as Parametric
from typing import TypeVarTuple as AxisBundle

Axes = AxisBundle("Axes")


class Lattice(Parametric[*Axes]):
    pass


def inspect(sample: Lattice[bytes, *tuple[Opaque, ...], float]) -> None:
    return None


def forward(sample: Lattice[bytes, float]) -> None:
    inspect(sample)
"#,
            ),
            reformatted(
                "
from typing import Any as UnknownValue, Generic as Shape, TypeVarTuple as DimensionPack

Dimensions = DimensionPack( 'Dimensions' )

class Array( Shape[ *Dimensions ] ):
        pass

def consume(
    value : Array[
        int ,
        * tuple[
            UnknownValue ,
            ... ,
        ] ,
        str ,
    ] ,
) -> None :
        return None

def relay( value : Array[ int , str ] ) -> None :
        consume( value )
",
            ),
        ],
    }
    .assert_by(
        "an unbounded unpack does not remove fixed type arguments",
        "generics_typevartuple_unpack",
    )
}

// ── Unpack needs something unpackable ────────────────────────────────────
//
// Alias-target discrimination again: identical annotation, opposite verdicts,
// decided solely by what `Sallie` was bound to.

#[test]
fn unpack_applied_to_a_scalar_type_variable_is_not_a_type() -> Result<(), Box<dyn std::error::Error>>
{
    SpecObligation {
        spec_reason: "PEP 646 introduces `Unpack` as the back-portable spelling of the star \
                      operator, whose operand is a type variable tuple or a tuple type; a plain \
                      `TypeVar` stands for a single type and has nothing to unpack, so \
                      `tuple[Unpack[T]]` is not a valid type while `tuple[Unpack[Ts]]` is",
        rejected: r#"
import typing

Sallie = typing.TypeVar("Sallie")


def toll(chime: tuple[typing.Unpack[Sallie]]) -> None:
    return None
"#,
        accepted: r#"
import typing

Sallie = typing.TypeVarTuple("Sallie")


def toll(chime: tuple[typing.Unpack[Sallie]]) -> None:
    return None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeVar as Scalar
from typing import Unpack as Splat

Sallie = Scalar("Sallie")


def toll(chime: tuple[Splat[Sallie]]) -> None:
    return None
"#,
            ),
            import_form(
                r#"
import typing

Sallie = typing.TypeVar("Sallie")


def toll(chime: tuple[*Sallie]) -> None:
    return None
"#,
            ),
            renamed(
                r#"
import typing

Garter = typing.TypeVar("Garter")


def strike(course: tuple[typing.Unpack[Garter]]) -> None:
    return None
"#,
            ),
            reformatted(
                "
import typing

Sallie = typing.TypeVar( 'Sallie' )

def toll(
    chime : tuple[
        typing.Unpack[ Sallie ]   # <- not a variadic, so nothing to unpack
    ] ,
) -> None :
        return None
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeVarTuple as Variadic
from typing import Unpack as Splat

Sallie = Variadic("Sallie")


def toll(chime: tuple[Splat[Sallie]]) -> None:
    return None
"#,
            ),
            import_form(
                r#"
import typing

Sallie = typing.TypeVarTuple("Sallie")


def toll(chime: tuple[*Sallie]) -> None:
    return None
"#,
            ),
        ],
    }
    .assert("Unpack applied to a scalar type variable is not a type")
}

// ── a variadic absorbs zero arguments; its neighbours stay required ──────

#[test]
fn a_variadic_args_parameter_absorbs_zero_arguments_without_excusing_the_rest(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a type variable tuple binds zero or more types, so `peal(3)` is well-typed \
                      with `Bells` bound to the empty tuple; the preceding non-variadic parameter \
                      is unaffected by the variadic and stays required, so `peal()` is missing an \
                      argument. A checker that treats `*args: *Ts` as swallowing the whole \
                      parameter list, or that demands at least one variadic argument, fails one \
                      of the two legs",
        rejected: r#"
import typing

Bells = typing.TypeVarTuple("Bells")


def peal(tenor: int, *ropes: *Bells) -> None:
    return None


def ring() -> None:
    peal()
"#,
        accepted: r#"
import typing

Bells = typing.TypeVarTuple("Bells")


def peal(tenor: int, *ropes: *Bells) -> None:
    return None


def ring() -> None:
    peal(3)
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeVarTuple as Variadic

Bells = Variadic("Bells")


def peal(tenor: int, *ropes: *Bells) -> None:
    return None


def ring() -> None:
    peal()
"#,
            ),
            import_form(
                r#"
import typing
import typing_extensions

Bells = typing_extensions.TypeVarTuple("Bells")


def peal(tenor: int, *ropes: typing.Unpack[Bells]) -> None:
    return None


def ring() -> None:
    peal()
"#,
            ),
            renamed(
                r#"
import typing

Garter = typing.TypeVarTuple("Garter")


def strike(hunt: int, *wheels: *Garter) -> None:
    return None


def lower() -> None:
    strike()
"#,
            ),
            reformatted(
                "
import typing

Bells = typing.TypeVarTuple( 'Bells' )

def peal( tenor : int , *ropes : *Bells ) -> None :
        return None

def ring() -> None :
        # the variadic absorbs nothing here, and tenor is still unfilled
        peal(  )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import TypeVarTuple as Variadic
from builtins import int as Whole

Bells = Variadic("Bells")


def peal(tenor: Whole, *ropes: *Bells) -> None:
    return None


def ring() -> None:
    peal(3)
"#,
            ),
            renamed(
                r#"
import typing

Garter = typing.TypeVarTuple("Garter")


def strike(hunt: int, *wheels: *Garter) -> None:
    return None


def lower() -> None:
    strike(3)
"#,
            ),
        ],
    }
    .assert("a variadic args parameter absorbs zero arguments without excusing the rest")
}

// ── Callable[[*Ts], R] fixes the order of a linked tuple[*Ts] ────────────

#[test]
fn an_unpacked_variadic_in_a_callable_fixes_the_order_of_the_linked_tuple(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 646: a `Callable` containing an unpacked item treats the elements \"as \
                      if they were the type for `*args`\", so `Callable[[*Bells], None]` solved \
                      against a two-parameter function binds `Bells` to `(int, str)`; the \
                      `tuple[*Bells]` argument must then be `(int, str)` in that order, and \
                      `(str, int)` is not assignable to it",
        rejected: r#"
import typing

Bells = typing.TypeVarTuple("Bells")


class Peal(typing.Generic[*Bells]):
    def __init__(self, stroke: typing.Callable[[*Bells], None], ropes: tuple[*Bells]) -> None:
        return None


def toll(course: int, sally: str) -> None:
    return None


chime = Peal(toll, ("handstroke", 3))
"#,
        accepted: r#"
import typing

Bells = typing.TypeVarTuple("Bells")


class Peal(typing.Generic[*Bells]):
    def __init__(self, stroke: typing.Callable[[*Bells], None], ropes: tuple[*Bells]) -> None:
        return None


def toll(course: int, sally: str) -> None:
    return None


chime = Peal(toll, (3, "handstroke"))
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import TypeVarTuple as Variadic
from typing import Generic as Parametric
from collections.abc import Callable as Signature

Bells = Variadic("Bells")


class Peal(Parametric[*Bells]):
    def __init__(self, stroke: Signature[[*Bells], None], ropes: tuple[*Bells]) -> None:
        return None


def toll(course: int, sally: str) -> None:
    return None


chime = Peal(toll, ("handstroke", 3))
"#,
            ),
            import_form(
                r#"
import typing
import collections.abc

Bells = typing.TypeVarTuple("Bells")


class Peal(typing.Generic[typing.Unpack[Bells]]):
    def __init__(
        self,
        stroke: collections.abc.Callable[[typing.Unpack[Bells]], None],
        ropes: tuple[typing.Unpack[Bells]],
    ) -> None:
        return None


def toll(course: int, sally: str) -> None:
    return None


chime = Peal(toll, ("handstroke", 3))
"#,
            ),
            renamed(
                r#"
import typing

Garter = typing.TypeVarTuple("Garter")


class Belfry(typing.Generic[*Garter]):
    def __init__(self, pull: typing.Callable[[*Garter], None], wheels: tuple[*Garter]) -> None:
        return None


def strike(hunt: int, stay: str) -> None:
    return None


course = Belfry(strike, ("handstroke", 3))
"#,
            ),
        ],
        accepted_variants: &[renamed(
            r#"
import typing

Garter = typing.TypeVarTuple("Garter")


class Belfry(typing.Generic[*Garter]):
    def __init__(self, pull: typing.Callable[[*Garter], None], wheels: tuple[*Garter]) -> None:
        return None


def strike(hunt: int, stay: str) -> None:
    return None


course = Belfry(strike, (3, "handstroke"))
"#,
        )],
    }
    .assert("an unpacked variadic in a Callable fixes the order of the linked tuple")
}
