//! Pins for the `TypeVar` census ([RESOLV-CANONICAL-BINDING]).
//!
//! PEP 484: type variables are declared by assignment at module (or class)
//! scope. Python's binding rules do not care whether the assignment sits
//! under `if TYPE_CHECKING:`, inside `try:`, or in a loop — the module frame
//! executes them all — so the census must walk compound statement bodies.
//! The typing spec's generics chapter also forbids a type variable inside a
//! bound; a BARE type variable (`bound=T`) is the simplest such case.
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

mod common;

fn typevar_names(src: &str) -> Vec<String> {
    let resolved = common::resolve_src(src).expect("source must resolve");
    resolved
        .typevar_calls
        .iter()
        .map(|tv| tv.name.clone())
        .collect()
}

#[test]
fn typevar_under_type_checking_guard_is_collected() {
    let names = typevar_names(
        r#"
from typing import TYPE_CHECKING, TypeVar

if TYPE_CHECKING:
    T = TypeVar("T")
"#,
    );
    assert!(
        names.contains(&"T".to_owned()),
        "the module frame executes the guarded body; the declaration is a \
         declaration — got: {names:?}"
    );
}

#[test]
fn typevar_inside_try_is_collected() {
    let names = typevar_names(
        r#"
from typing import TypeVar

try:
    T = TypeVar("T")
except ImportError:
    pass
"#,
    );
    assert!(
        names.contains(&"T".to_owned()),
        "a `try:` body runs in the module frame — got: {names:?}"
    );
}

#[test]
fn bare_typevar_bound_is_flagged_as_parameterized() {
    let resolved = common::resolve_src(
        r#"
from typing import TypeVar

T = TypeVar("T")
U = TypeVar("U", bound=T)
"#,
    )
    .expect("source must resolve");
    let u = resolved
        .typevar_calls
        .iter()
        .find(|tv| tv.name == "U")
        .expect("U must be collected");
    assert!(
        u.has_parameterized_bound,
        "`bound=T` embeds a type variable in a bound, which the generics \
         spec forbids; the bare case is the simplest instance"
    );
}

#[test]
fn concrete_bound_is_not_flagged() {
    let resolved = common::resolve_src(
        r#"
from typing import TypeVar

T = TypeVar("T", bound=int)
"#,
    )
    .expect("source must resolve");
    let t = resolved
        .typevar_calls
        .iter()
        .find(|tv| tv.name == "T")
        .expect("T must be collected");
    assert!(
        !t.has_parameterized_bound,
        "`bound=int` is a concrete bound, exactly what PEP 484 permits"
    );
}
