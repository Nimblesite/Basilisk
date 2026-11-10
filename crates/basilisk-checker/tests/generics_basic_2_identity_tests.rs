//! Implements `generics_basic_2` — [PEP 484](https://peps.python.org/pep-0484/#generics)
//! and [PEP 544](https://peps.python.org/pep-0544/#generic-protocols): every
//! argument to `Generic[...]` and `Protocol[...]` must be a type variable.
//!
//! The rule this replaces matched RENDERED names on both sides — the base's
//! spelling against `"Generic"`, and each argument's spelling against a set of
//! type-variable names harvested from source. Every test here is a
//! semantics-preserving mutation of a case the rule must already get right, so
//! a rule that reads spellings fails at least one of them:
//!
//! * the type variable is reached under an alias, so its *name* at the use site
//!   is not the name it was declared with;
//! * `Generic` is reached under an aliased import and a qualified path;
//! * a local class is spelled exactly like a conventional type variable;
//! * two type variables are spelled alike and only one is in scope;
//! * the same program is reformatted across lines.
//!
//! None of the fixtures appear in the python/typing conformance suite.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

#[allow(dead_code)]
mod common;

use common::run;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// How many `generics_basic_2` diagnostics a source produces.
fn count(source: &str) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(run(source)?
        .iter()
        .filter(|diag| diag.code.code == "generics_basic_2")
        .count())
}

// ---------------------------------------------------------------------------
// The base case, in both spellings PEP 484 and PEP 544 define
// ---------------------------------------------------------------------------

/// A concrete builtin in `Generic[...]` is the error the rule exists for.
#[test]
fn a_builtin_class_in_generic_is_reported() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic

class Cairn(Generic[int]): ...
"
        )?,
        1,
        "`int` is a class, not a type variable"
    );
    Ok(())
}

/// PEP 544 says the same of `Protocol[...]`.
#[test]
fn a_builtin_class_in_protocol_is_reported() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Protocol

class Waypoint(Protocol[str]): ...
"
        )?,
        1,
        "`str` is a class, not a type variable"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The type variable is reached under a name it was not declared with
// ---------------------------------------------------------------------------

/// `Marker = Bearing` binds the very same `TypeVar` object. The use site
/// spells a name that appears in no `TypeVar(...)` call anywhere, and the rule
/// must still see a type variable.
///
/// The deleted rule reported here — an alias was invisible to a name set — so
/// this is a FALSE POSITIVE the spelling-based rule produced on valid code.
#[test]
fn a_type_variable_reached_through_an_assignment_alias_is_accepted() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic, TypeVar

Bearing = TypeVar(\"Bearing\")
Marker = Bearing

class Cairn(Generic[Marker]): ...
"
        )?,
        0,
        "`Marker` is the `Bearing` type variable under another name"
    );
    Ok(())
}

/// A chain of aliases is still one object.
#[test]
fn a_chain_of_assignment_aliases_still_reaches_the_type_variable() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic, TypeVar

Bearing = TypeVar(\"Bearing\")
Marker = Bearing
Sighting = Marker

class Cairn(Generic[Sighting]): ...
"
        )?,
        0,
        "three names, one `TypeVar` object"
    );
    Ok(())
}

/// The `TypeVar` factory itself reached under an aliased import: the
/// construction must still be recognised as making a type parameter.
#[test]
fn an_aliased_typevar_import_still_constructs_a_type_variable() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic as Parametric, TypeVar as Param

Bearing = Param(\"Bearing\")

class Cairn(Parametric[Bearing]): ...
"
        )?,
        0,
        "`Param` IS `typing.TypeVar`, and `Parametric` IS `typing.Generic`"
    );
    Ok(())
}

/// The qualified spelling denotes the same symbols.
#[test]
fn a_qualified_generic_and_typevar_are_the_same_symbols() -> TestResult {
    assert_eq!(
        count(
            "\
import typing

Bearing = typing.TypeVar(\"Bearing\")

class Cairn(typing.Generic[Bearing]): ...
"
        )?,
        0,
        "`typing.Generic[...]` is `Generic[...]`"
    );
    Ok(())
}

/// …and reports the error identically when the argument is concrete.
#[test]
fn a_qualified_generic_still_reports_a_concrete_argument() -> TestResult {
    assert_eq!(
        count(
            "\
import typing as tp

class Cairn(tp.Generic[bool]): ...
"
        )?,
        1,
        "the diagnostic must not depend on how `Generic` was imported"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// A spelling that LOOKS like a type variable but is not one
// ---------------------------------------------------------------------------

/// `class T` is a class. Conventional type-variable spelling gives it no
/// standing, and the deleted rule accepted it on exactly that basis.
///
/// This is the FALSE NEGATIVE the spelling-based rule produced on invalid code.
#[test]
fn a_local_class_spelled_like_a_type_variable_is_still_a_class() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic

class T: ...

class Cairn(Generic[T]): ...
"
        )?,
        1,
        "`T` here is a class statement, whatever it is called"
    );
    Ok(())
}

/// A name bound to an ordinary value is not a type variable either.
#[test]
fn a_name_bound_to_a_plain_value_is_not_a_type_variable() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic

T = 3

class Cairn(Generic[T]): ...
"
        )?,
        1,
        "`T` is bound to an integer"
    );
    Ok(())
}

/// A `TypeVar` name later REBOUND to a class: the base list resolves the
/// binding in force at its own position, which is the class.
#[test]
fn a_type_variable_rebound_to_a_class_before_its_use_is_a_class() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic, TypeVar

Bearing = TypeVar(\"Bearing\")

class Bearing: ...

class Cairn(Generic[Bearing]): ...
"
        )?,
        1,
        "the `class` statement rebinds the name before the base list runs"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// PEP 695 and PEP 646 spellings
// ---------------------------------------------------------------------------

/// A PEP 695 type parameter is bound by the class's own type-parameter list.
#[test]
fn a_pep695_type_parameter_is_a_type_variable() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Protocol

class Cairn[Bearing](Protocol[Bearing]): ...
"
        )?,
        0,
        "`Bearing` is declared by this class's type-parameter list"
    );
    Ok(())
}

/// A PEP 695 class whose `Protocol[...]` argument is concrete is still wrong.
#[test]
fn a_pep695_class_still_reports_a_concrete_argument() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Protocol

class Cairn[Bearing](Protocol[int]): ...
"
        )?,
        1,
        "declaring a type parameter does not license a concrete argument"
    );
    Ok(())
}

/// PEP 646 writes an unpacked `TypeVarTuple` two ways; both denote the same
/// thing and must be accepted identically.
#[test]
fn both_pep646_unpack_spellings_are_accepted() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic, TypeVarTuple, Unpack

Bearings = TypeVarTuple(\"Bearings\")

class Starred(Generic[*Bearings]): ...
class Wrapped(Generic[Unpack[Bearings]]): ...
"
        )?,
        0,
        "`*Ts` and `Unpack[Ts]` are the same unpack"
    );
    Ok(())
}

/// A `ParamSpec` is a type variable for PEP 484's purposes here (PEP 612).
#[test]
fn a_paramspec_is_a_type_variable() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic, ParamSpec

Approach = ParamSpec(\"Approach\")

class Cairn(Generic[Approach]): ...
"
        )?,
        0,
        "PEP 612 makes `ParamSpec` a type parameter"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Formatting and shadowing
// ---------------------------------------------------------------------------

/// Reformatting the same program changes nothing.
#[test]
fn reformatting_changes_no_diagnostic() -> TestResult {
    let tight = "\
from typing import Generic, TypeVar
Bearing = TypeVar(\"Bearing\")
class Cairn(Generic[Bearing, int]): ...
";
    let loose = "\
from typing import (
    Generic,
    TypeVar,
)

Bearing = TypeVar(
    \"Bearing\",
)


class Cairn(
    Generic[
        Bearing,
        int,
    ],
): ...
";
    assert_eq!(count(tight)?, 1, "`int` is the one bad argument");
    assert_eq!(
        count(tight)?,
        count(loose)?,
        "line breaks inside the base list are not semantics"
    );
    Ok(())
}

/// A module that declares its own `Generic` has shadowed `typing.Generic`, and
/// its subscript is not the PEP 484 form at all.
#[test]
fn a_locally_declared_generic_is_not_the_typing_one() -> TestResult {
    assert_eq!(
        count(
            "\
class Generic:
    def __class_getitem__(cls, item): ...

class Cairn(Generic[int]): ...
"
        )?,
        0,
        "this `Generic` is the module's own class; PEP 484 says nothing about it"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Abstention
// ---------------------------------------------------------------------------

/// An argument imported from a module the checker cannot see MAY be a type
/// variable. Reporting it would be an assertion about code never read
/// ([CHKARCH-CONFORMANCE-MODE]).
#[test]
fn an_unresolvable_argument_reports_nothing() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import Generic
from surveying import Bearing

class Cairn(Generic[Bearing]): ...
"
        )?,
        0,
        "`Bearing` comes from a module this run never resolved"
    );
    Ok(())
}

/// A class with no type-parameter base at all is not this rule's business.
#[test]
fn an_ordinary_base_list_reports_nothing() -> TestResult {
    assert_eq!(
        count(
            "\
class Waypoint: ...
class Cairn(Waypoint): ...
"
        )?,
        0,
        "no `Generic[...]` here"
    );
    Ok(())
}
