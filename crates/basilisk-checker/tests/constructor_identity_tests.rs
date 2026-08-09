//! Pins the constructor rules to RESOLVED IDENTITY rather than source text.
//!
//! Two verdict paths in the constructor rules decided from characters:
//!
//! * `calls_argument_count` sliced a metaclass `__call__`'s return annotation
//!   out of the file and compared the trimmed characters against the *name* a
//!   `TypeVar` was declared with, to decide whether the metaclass passes
//!   construction through to `__new__`/`__init__`;
//! * `constructors_call_type` recognised `type[X]` only when `type` was
//!   spelled as that exact bare word, then sliced `X`'s source text and looked
//!   it up in name-keyed class/`TypeVar`/bound maps.
//!
//! Every fixture below is a semantics-preserving mutation of a case those
//! rules already got right, so a rule reading spellings fails at least one.
//! None of them appear in the python/typing conformance suite.
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

/// How many diagnostics with `code` a source produces.
fn count(source: &str, code: &str) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(run(source)?
        .iter()
        .filter(|diag| diag.code.code == code)
        .count())
}

// ---------------------------------------------------------------------------
// `calls_argument_count`: the metaclass `__call__` pass-through judgment
// ---------------------------------------------------------------------------

/// The control. `-> T` names the module's `TypeVar`, so `Meta.__call__` is the
/// pass-through form and `Widget`'s own `__init__` governs the call.
#[test]
fn a_pass_through_metaclass_still_checks_the_constructor() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import TypeVar

Bearing = TypeVar(\"Bearing\")

class Meta(type):
    def __call__(cls: type[Bearing], *args, **kwargs) -> Bearing: ...

class Cairn(metaclass=Meta):
    def __init__(self, height: int) -> None: ...

Cairn()
",
            "calls_argument_count"
        )?,
        1,
        "the metaclass passes through, so `__init__`'s required `height` is missing"
    );
    Ok(())
}

/// The same program with the type variable reached under an assignment alias.
///
/// `Marker = Bearing` binds the very same `TypeVar` object, so `-> Marker` is
/// the identical annotation. The text path compared `"Marker"` against the
/// declared name `"Bearing"`, concluded the metaclass fully controls
/// construction, and silently dropped the diagnostic.
#[test]
fn an_aliased_type_variable_return_is_still_pass_through() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import TypeVar

Bearing = TypeVar(\"Bearing\")
Marker = Bearing

class Meta(type):
    def __call__(cls: type[Marker], *args, **kwargs) -> Marker: ...

class Cairn(metaclass=Meta):
    def __init__(self, height: int) -> None: ...

Cairn()
",
            "calls_argument_count"
        )?,
        1,
        "`Marker` IS `Bearing`; renaming the reference changes no semantics"
    );
    Ok(())
}

/// A metaclass `__call__` returning a concrete class controls construction
/// itself, so the class's own constructor is never consulted.
#[test]
fn a_concrete_metaclass_return_suppresses_the_constructor_check() -> TestResult {
    assert_eq!(
        count(
            "\
class Token: ...

class Meta(type):
    def __call__(cls, *args, **kwargs) -> Token: ...

class Cairn(metaclass=Meta):
    def __init__(self, height: int) -> None: ...

Cairn()
",
            "calls_argument_count"
        )?,
        0,
        "`Meta.__call__` returns a `Token`, so `Cairn.__init__` never runs"
    );
    Ok(())
}

/// A name REBOUND away from its `TypeVar` before the metaclass is declared no
/// longer denotes a type variable, whatever it is still spelled.
#[test]
fn a_type_variable_name_rebound_to_a_value_is_not_pass_through() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import TypeVar

Bearing = TypeVar(\"Bearing\")
Bearing = 3

class Meta(type):
    def __call__(cls, *args, **kwargs) -> Bearing: ...

class Cairn(metaclass=Meta):
    def __init__(self, height: int) -> None: ...

Cairn()
",
            "calls_argument_count"
        )?,
        0,
        "at the `def`, `Bearing` is bound to an integer, not to the type variable"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// `constructors_call_type`: recognising `type[X]` and resolving `X`
// ---------------------------------------------------------------------------

/// The control. `cls: type[Cairn]` called with no arguments, where `Cairn`
/// requires one.
#[test]
fn a_bare_type_subscript_is_recognised() -> TestResult {
    assert_eq!(
        count(
            "\
class Cairn:
    def __init__(self, height: int) -> None: ...

def raise_marker(cls: type[Cairn]) -> None:
    cls()
",
            "constructors_call_type"
        )?,
        1,
        "`cls()` omits `Cairn.__init__`'s required `height`"
    );
    Ok(())
}

/// `builtins.type` is the identical object, written as an attribute access.
/// The text path required the bare word and saw nothing here.
#[test]
fn a_qualified_type_subscript_is_the_same_object() -> TestResult {
    assert_eq!(
        count(
            "\
import builtins

class Cairn:
    def __init__(self, height: int) -> None: ...

def raise_marker(cls: builtins.type[Cairn]) -> None:
    cls()
",
            "constructors_call_type"
        )?,
        1,
        "`builtins.type[X]` is `type[X]`"
    );
    Ok(())
}

/// A module that declares its own `type` has shadowed the builtin, and its
/// subscript is not the typing form at all. The text path matched the word and
/// reported against a constructor contract that does not apply.
#[test]
fn a_locally_declared_type_is_not_the_builtin() -> TestResult {
    assert_eq!(
        count(
            "\
class Cairn:
    def __init__(self, height: int) -> None: ...

class type:
    def __class_getitem__(cls, item): ...

def raise_marker(cls: type[Cairn]) -> None:
    cls()
",
            "constructors_call_type"
        )?,
        0,
        "this `type` is the module's own class; the `type[T]` rules say nothing about it"
    );
    Ok(())
}

/// The class inside `type[...]` reached under an assignment alias is the same
/// class. The name-keyed lookup missed it entirely.
#[test]
fn an_aliased_class_inside_type_is_the_same_class() -> TestResult {
    assert_eq!(
        count(
            "\
class Cairn:
    def __init__(self, height: int) -> None: ...

Marker = Cairn

def raise_marker(cls: type[Marker]) -> None:
    cls()
",
            "constructors_call_type"
        )?,
        1,
        "`Marker` IS `Cairn`"
    );
    Ok(())
}

/// A `TypeVar` bound reached under an alias resolves to the same class, so the
/// bound class's constructor is the contract.
#[test]
fn an_aliased_typevar_bound_resolves_to_its_class() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import TypeVar

class Cairn:
    def __init__(self, height: int) -> None: ...

Marker = Cairn
Bearing = TypeVar(\"Bearing\", bound=Marker)

def raise_marker(cls: type[Bearing]) -> None:
    cls()
",
            "constructors_call_type"
        )?,
        1,
        "`bound=Marker` bounds `Bearing` by `Cairn`"
    );
    Ok(())
}

/// An UNBOUND type variable's constructor signature is unknown, so passing
/// arguments to it is the error — and the type variable must be recognised
/// under an alias just the same.
#[test]
fn an_aliased_unbound_type_variable_still_rejects_arguments() -> TestResult {
    assert_eq!(
        count(
            "\
from typing import TypeVar

Bearing = TypeVar(\"Bearing\")
Marker = Bearing

def raise_marker(cls: type[Marker]) -> None:
    cls(1)
",
            "constructors_call_type"
        )?,
        1,
        "`Marker` is the unbound `Bearing`; its constructor takes no known arguments"
    );
    Ok(())
}

/// A class merely SPELLED like a conventional type variable is a class, and
/// `type[T]` then means "the class `T`", not "some unbound type variable".
#[test]
fn a_class_spelled_like_a_type_variable_is_a_class() -> TestResult {
    assert_eq!(
        count(
            "\
class T:
    def __init__(self, height: int) -> None: ...

def raise_marker(cls: type[T]) -> None:
    cls(1)
",
            "constructors_call_type"
        )?,
        0,
        "`T` is a class statement whose `__init__` accepts exactly this call"
    );
    Ok(())
}

/// Reformatting the subscript changes nothing.
#[test]
fn reformatting_a_type_subscript_changes_no_diagnostic() -> TestResult {
    let tight = "\
class Cairn:
    def __init__(self, height: int) -> None: ...
def raise_marker(cls: type[Cairn]) -> None:
    cls()
";
    let loose = "\
class Cairn:
    def __init__(self, height: int) -> None: ...


def raise_marker(
    cls: type[
        Cairn
    ],
) -> None:
    cls()
";
    assert_eq!(count(tight, "constructors_call_type")?, 1);
    assert_eq!(
        count(tight, "constructors_call_type")?,
        count(loose, "constructors_call_type")?,
        "line breaks inside the subscript are not semantics"
    );
    Ok(())
}

/// An argument the checker cannot resolve is not evidence of anything.
#[test]
fn an_unresolvable_type_argument_reports_nothing() -> TestResult {
    assert_eq!(
        count(
            "\
from surveying import Cairn

def raise_marker(cls: type[Cairn]) -> None:
    cls()
",
            "constructors_call_type"
        )?,
        0,
        "`Cairn` comes from a module this run never resolved"
    );
    Ok(())
}
