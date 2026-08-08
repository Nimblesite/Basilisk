//! Pins the defects in the DELETED string-keyed nominal layer.
//!
//! `InferredType::Named(String)` carries a **rendered spelling**, and the
//! assignability core used to perform string surgery on it to reach a verdict:
//!
//! * `name == "object"` — the top type recognised by its builtin spelling, so
//!   a class the user names `object` was treated as `builtins.object`, and
//!   `from builtins import object as Anything` was not.
//! * `name == "type" || name.starts_with("type[")` — class-object-ness decided
//!   by a `starts_with` on a rendered generic.
//! * `a_name.split('[').next() == b_name.split('[').next()` — generic
//!   compatibility decided by splitting a rendered type at a bracket. This is
//!   parsing a type out of a string, which [CHKARCH] forbids outright.
//! * `judge::nominal_leaf` — a spelling TABLE mapping `InferredType::Int` to
//!   `"int"`, feeding string comparisons in `nominal_subclass_assignable`,
//!   which then decided enum membership with
//!   `sub.strip_prefix(sup).starts_with('.')` — a TEXT operation standing in
//!   for "is this member declared by that enum?".
//!
//! Every one of those decided a verdict from how a type happened to be
//! *rendered*, never from what it was *resolved to be*. They were deleted, not
//! repaired. These tests are the specification of the replacement, and they
//! MUST NOT be weakened, deleted, or made to pass by restoring any of the
//! above — see the banner in `src/types.rs`.
//!
//! Every case below is authored in vocabulary the python/typing conformance
//! suite does not contain, and every typing import is aliased, so a rule
//! fitted to a fixture cannot satisfy them.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

mod common;

use common::run;

/// Diagnostics naming `code`, for a source under test.
fn codes_for(source: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(run(source)?
        .into_iter()
        .map(|diag| diag.code.code.to_owned())
        .collect())
}

/// A user class named `object` is NOT `builtins.object`.
///
/// The deleted `special_named_assignable` matched the SPELLING `"object"` and
/// returned "assignable" for either side, so a locally-declared class stealing
/// that name silently became the top type and every assignment to it was
/// blessed. Assigning an `int` to a parameter of the user's own `object` class
/// is a type error the checker must not miss.
#[test]
fn a_locally_declared_class_named_object_is_not_the_top_type() {
    let source = r#"
class object:  # shadows the builtin; this is NOT builtins.object
    def moor(self) -> None: ...


def berth(bollard: object) -> None:
    bollard.moor()


berth(7)
"#;
    let codes = codes_for(source).expect("module must check");
    assert!(
        codes.iter().any(|code| code.starts_with("calls_argument")),
        "a class that merely SPELLS itself `object` is not the top type, so `7` does \
         not satisfy it. Got {codes:?}. The deleted `special_named_assignable` blessed \
         this by matching the spelling."
    );
}

/// The real top type, reached under an alias, must still behave as the top type.
///
/// The mirror of the case above: recognition keyed to the spelling `"object"`
/// fails the moment the top type arrives under any other name, so lawful code
/// gets a false positive.
#[test]
fn the_builtin_top_type_still_accepts_anything_under_an_alias() {
    let source = r#"
from builtins import object as AnythingAtAll


def stow(cargo: AnythingAtAll) -> None: ...


stow(7)
stow("kelp")
stow(None)
"#;
    let codes = codes_for(source).expect("module must check");
    assert!(
        codes.is_empty(),
        "`builtins.object` is the top type however it is spelled — every value is an \
         object. Got {codes:?}."
    );
}

/// Two unrelated generics that share a base SPELLING are not compatible.
///
/// The deleted `(Named, Named)` arm split both rendered names at `[` and
/// compared the prefixes, so any two types whose rendering happened to agree
/// before the bracket were declared compatible — including two genuinely
/// unrelated classes imported from different modules under the same name.
#[test]
fn distinct_classes_sharing_a_rendered_base_name_are_not_compatible() {
    let source = r#"
class Trawler:
    def haul(self) -> None: ...


class Dredger:
    def haul(self) -> None: ...


def unload(vessel: Trawler) -> None:
    vessel.haul()


unload(Dredger())
"#;
    let codes = codes_for(source).expect("module must check");
    assert!(
        codes.iter().any(|code| code.starts_with("calls_argument")),
        "`Dredger` is not a `Trawler` — structural similarity is not nominal identity, \
         and neither is a shared rendering. Got {codes:?}."
    );
}

/// An enum member belongs to its enum by DECLARATION, not by dotted spelling.
///
/// The deleted `nominal_subclass_assignable` decided this with
/// `sub.strip_prefix(sup).is_some_and(|rest| rest.starts_with('.'))` — so any
/// name that merely began with the target's name followed by a dot was
/// accepted as a member of it, and a genuine member reached under an alias was
/// not.
#[test]
fn an_unrelated_class_is_not_a_member_of_an_enum_that_prefixes_its_name() {
    let source = r#"
from enum import Enum as Enumeration


class Tide(Enumeration):
    EBB = 1
    FLOOD = 2


class TideGauge:  # name begins with `Tide`, but is no member of it
    def read(self) -> int:
        return 0


def record(phase: Tide) -> None: ...


record(TideGauge())
"#;
    let codes = codes_for(source).expect("module must check");
    assert!(
        codes.iter().any(|code| code.starts_with("calls_argument")),
        "`TideGauge` is a separate class whose NAME happens to share a prefix with \
         `Tide`. Membership is a question about declarations. Got {codes:?}."
    );
}

/// The same program, respelled, must draw the same verdict.
///
/// The single property every deleted helper violated. Whatever the replacement
/// decides here, it must decide identically for both spellings.
#[test]
fn the_verdict_does_not_move_when_the_types_are_renamed() {
    let canonical = r#"
class Trawler:
    def haul(self) -> None: ...


class Dredger:
    def haul(self) -> None: ...


def unload(vessel: Trawler) -> None: ...


unload(Dredger())
"#;
    let renamed = r#"
class Barquentine:
    def haul(self) -> None: ...


class Lugger:
    def haul(self) -> None: ...


def discharge(craft: Barquentine) -> None: ...


discharge(Lugger())
"#;
    let mut left = codes_for(canonical).expect("module must check");
    let mut right = codes_for(renamed).expect("module must check");
    left.sort();
    right.sort();
    assert_eq!(
        left, right,
        "a consistent alpha-rename changed the verdict — the judgment is reading \
         spellings, not resolved identities"
    );
}
