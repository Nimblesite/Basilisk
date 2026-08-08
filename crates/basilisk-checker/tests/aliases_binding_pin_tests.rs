//! Pins for [`aliases_newtype`] and [`aliases_implicit`] under
//! [ASTREBUILD-LAW] ([CHKARCH-DIAG-STRUCTURAL]): every verdict must come from
//! resolved bindings and the semantic relation layer
//! ([RESOLV-CANONICAL-RELATION]), never from identifier spellings.
//!
//! Spec sources: PEP 484 (`NewType` semantics — the constructor accepts only
//! values of the base type, and the returned object is not a class), PEP 613
//! (`TypeAlias`), and bounded `TypeVar`s
//! (<https://typing.python.org/en/latest/spec/generics.html#typevar>).
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs,
    dead_code
)]

mod common;

fn messages(source: &str) -> Vec<String> {
    let diags = common::run(source).expect("source must check");
    diags.into_iter().map(|d| d.message).collect()
}

// --- aliases_newtype: PEP 484 `NewType` -----------------------------------

#[test]
fn aliased_base_type_still_relates_constructor_arguments() {
    // PEP 484: "the module type checker will treat UserId as if it were a
    // subclass of int" — the constructor accepts only base-type values.
    let msgs = messages(
        r#"
from typing import NewType
from builtins import int as I

UserId = NewType("UserId", I)
bad = UserId("user")
"#,
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("not compatible with its base type")),
        "an aliased import of `int` IS `int`; the str argument must be \
         flagged regardless of the base's spelling — got: {msgs:?}"
    );
}

#[test]
fn aliased_isinstance_with_newtype_is_flagged() {
    // PEP 484: the object returned by NewType is not a real class; it
    // "cannot be used in isinstance() checks".
    let msgs = messages(
        r#"
from typing import NewType
from builtins import isinstance as chk

UserId = NewType("UserId", int)
ok = chk(1, UserId)
"#,
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("cannot be used as the second argument")),
        "an aliased import of `isinstance` IS `isinstance` — got: {msgs:?}"
    );
}

#[test]
fn shadowed_isinstance_with_newtype_is_not_flagged() {
    let msgs = messages(
        r#"
from typing import NewType

UserId = NewType("UserId", int)

def isinstance(a, b):
    return True

ok = isinstance(1, UserId)
"#,
    );
    assert!(
        !msgs
            .iter()
            .any(|m| m.contains("cannot be used as the second argument")),
        "a module-level `def isinstance` shadows the builtin; the call is \
         not a runtime type check — got: {msgs:?}"
    );
}

#[test]
fn qualified_type_annotation_flags_newtype_assignment() {
    // PEP 484: `NewType(...)` returns a callable, not a class object, so it
    // is not assignable to a `type`-annotated variable.
    let msgs = messages(
        r#"
import builtins
from typing import NewType

UserId = NewType("UserId", int)
t: builtins.type = UserId
"#,
    );
    assert!(
        msgs.iter().any(|m| m.contains("does not return a class object")),
        "`builtins.type` IS `type`; qualification must not change the \
         verdict — got: {msgs:?}"
    );
}

#[test]
fn shadowed_type_annotation_does_not_flag_newtype_assignment() {
    let msgs = messages(
        r#"
from typing import NewType

UserId = NewType("UserId", int)

class type:
    pass

t: type = UserId
"#,
    );
    assert!(
        !msgs.iter().any(|m| m.contains("does not return a class object")),
        "a module-level `class type` shadows the builtin; the annotation \
         does not denote the builtin `type` — got: {msgs:?}"
    );
}

// --- aliases_implicit: bounded TypeVar arguments (PEP 484) -----------------

#[test]
fn aliased_bound_and_argument_still_check() {
    // Typing spec, generics: "the type parameter must be a subtype of the
    // bound" — identity comes from bindings, not spelling.
    let msgs = messages(
        r#"
from typing import TypeAlias, TypeVar
from builtins import int as I, str as S

T = TypeVar("T", bound=I)
Alias: TypeAlias = list[T]
x: Alias[S] = []
"#,
    );
    assert!(
        msgs.iter().any(|m| m.contains("does not satisfy bound")),
        "aliased `str` is not a subtype of aliased `int`; the bound \
         violation must be flagged regardless of spellings — got: {msgs:?}"
    );
}

#[test]
fn plain_bound_violation_still_flagged() {
    let msgs = messages(
        r#"
from typing import TypeAlias, TypeVar

T = TypeVar("T", bound=int)
Alias: TypeAlias = list[T]
y: Alias[str] = []
"#,
    );
    assert!(
        msgs.iter().any(|m| m.contains("does not satisfy bound")),
        "`str` is not a subtype of bound `int` — got: {msgs:?}"
    );
}

#[test]
fn satisfied_bound_is_not_flagged() {
    // `bool` is a subclass of `int`, so the bound is satisfied (PEP 484
    // numeric tower / nominal subtyping).
    let msgs = messages(
        r#"
from typing import TypeAlias, TypeVar

T = TypeVar("T", bound=int)
Alias: TypeAlias = list[T]
z: Alias[bool] = []
"#,
    );
    assert!(
        !msgs.iter().any(|m| m.contains("does not satisfy bound")),
        "`bool` is a subtype of `int`; no violation exists — got: {msgs:?}"
    );
}

#[test]
fn unresolved_bound_or_argument_abstains() {
    // Unresolvable imports lower to Unknown; the relation abstains rather
    // than inventing a verdict ([RESOLV-CANONICAL-RELATION]).
    let msgs = messages(
        r#"
from typing import TypeAlias, TypeVar
from somewhere import Base, Impl

T = TypeVar("T", bound=Base)
Alias: TypeAlias = list[T]
w: Alias[Impl] = []
"#,
    );
    assert!(
        !msgs.iter().any(|m| m.contains("does not satisfy bound")),
        "neither side resolves; the checker must abstain, not guess — \
         got: {msgs:?}"
    );
}
