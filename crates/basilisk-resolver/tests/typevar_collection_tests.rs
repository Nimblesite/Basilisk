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
from typing import TYPE_CHECKING as ANALYSIS_BRANCH
from typing import TypeVar as parameter_factory

if ANALYSIS_BRANCH:
    SedimentKind = parameter_factory("SedimentKind")
"#,
    );
    assert_eq!(
        names,
        vec!["SedimentKind"],
        "a typing-only branch still contributes exactly its module-scoped TypeVar declaration"
    );
}

#[test]
fn typevar_inside_try_is_collected() {
    let names = typevar_names(
        r#"
from typing import TypeVar as parameter_factory

try:
    CoreSampleKind = parameter_factory("CoreSampleKind")
except ImportError:
    pass
"#,
    );
    assert_eq!(
        names,
        vec!["CoreSampleKind"],
        "a module-level try body contributes exactly its TypeVar declaration"
    );
}

#[test]
fn bare_typevar_bound_is_flagged_as_parameterized() {
    let resolved = common::resolve_src(
        r#"
from typing import TypeVar as parameter_factory

ElementKind = parameter_factory("ElementKind")
ContainerKind = parameter_factory(
    "ContainerKind",
    bound=ElementKind,
)
"#,
    )
    .expect("source must resolve");
    assert_eq!(
        resolved.typevar_calls.len(),
        2,
        "both renamed declarations must be collected exactly once"
    );
    let u = resolved
        .typevar_calls
        .iter()
        .find(|tv| tv.name == "ContainerKind")
        .expect("ContainerKind must be collected");
    assert!(
        u.has_parameterized_bound,
        "the bound resolves to `ElementKind`, another TypeVar; spelling and layout cannot hide the forbidden parameterized bound"
    );
}

#[test]
fn concrete_bound_is_not_flagged() {
    let resolved = common::resolve_src(
        r#"
import builtins as runtime_types
import typing as type_contracts

SampleKind = type_contracts.TypeVar(
    "SampleKind",
    bound=runtime_types.int,
)
"#,
    )
    .expect("source must resolve");
    assert_eq!(
        resolved.typevar_calls.len(),
        1,
        "the qualified and reformatted declaration must be collected exactly once"
    );
    let t = resolved
        .typevar_calls
        .iter()
        .find(|tv| tv.name == "SampleKind")
        .expect("SampleKind must be collected");
    assert!(
        !t.has_parameterized_bound,
        "`bound=int` is a concrete bound, exactly what PEP 484 permits"
    );
}
