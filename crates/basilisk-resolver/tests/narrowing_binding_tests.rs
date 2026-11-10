//! Pins for [TYPEINF-NARROWING-HASATTR] / [TYPEINF-NARROWING-TYPEOF] under
//! [ASTREBUILD-LAW]: narrowing-guard recognition must resolve `hasattr` and
//! `type` through the binding table, never by comparing identifier spellings.
//!
//! Each test asserts behaviour on Python whose *meaning* differs from its
//! *spelling*: an aliased import of a builtin must still narrow, and a
//! module-level shadowing definition must stop narrowing.
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use basilisk_resolver::{NarrowingGuard, NarrowingGuardKind};

mod common;

fn function_guards(src: &str, function: &str) -> Vec<NarrowingGuard> {
    let resolved = common::resolve_src(src).expect("source must resolve");
    resolved
        .functions
        .iter()
        .find(|f| f.name == function)
        .unwrap_or_else(|| panic!("function `{function}` not found"))
        .narrowing_guards
        .clone()
}

fn hasattr_guards(guards: &[NarrowingGuard]) -> Vec<(String, String)> {
    guards
        .iter()
        .filter_map(|guard| match &guard.kind {
            NarrowingGuardKind::HasAttr {
                variable,
                attribute,
                ..
            } => Some((variable.clone(), attribute.clone())),
            _ => None,
        })
        .collect()
}

fn type_of_guards(guards: &[NarrowingGuard]) -> Vec<(String, String)> {
    guards
        .iter()
        .filter_map(|guard| match &guard.kind {
            NarrowingGuardKind::TypeOfIs {
                variable,
                type_name,
                ..
            } => Some((variable.clone(), type_name.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn aliased_hasattr_still_narrows() {
    let guards = function_guards(
        r#"
from builtins import hasattr as attribute_probe

def inspect_record(specimen):
    if attribute_probe(specimen, "crystal_depth"):
        return specimen.crystal_depth
    return None
"#,
        "inspect_record",
    );
    assert_eq!(
        hasattr_guards(&guards),
        vec![("specimen".to_owned(), "crystal_depth".to_owned())],
        "an aliased import of `hasattr` is `hasattr`; recognition must come \
         from binding resolution, not the callee's spelling"
    );
}

#[test]
fn shadowed_hasattr_does_not_narrow() {
    let guards = function_guards(
        r#"
from builtins import hasattr as attribute_probe

def attribute_probe(candidate, label):
    return False

def inspect_record(specimen):
    if attribute_probe(specimen, "crystal_depth"):
        return specimen.crystal_depth
    return None
"#,
        "inspect_record",
    );
    assert_eq!(
        hasattr_guards(&guards),
        Vec::<(String, String)>::new(),
        "the later local definition replaces the imported builtin alias; treating its spelling as semantic fabricates narrowing"
    );
}

#[test]
fn shadowed_type_does_not_narrow() {
    let guards = function_guards(
        r#"
from builtins import type as category_probe

def category_probe(specimen):
    return 0

class MineralSample:
    pass

def inspect_record(specimen):
    if category_probe(specimen) is MineralSample:
        return specimen
    return None
"#,
        "inspect_record",
    );
    assert_eq!(
        type_of_guards(&guards),
        Vec::<(String, String)>::new(),
        "the later local definition replaces the imported type alias and must not narrow"
    );
}

#[test]
fn aliased_and_reformatted_type_still_narrows() {
    let guards = function_guards(
        r#"
from builtins import type as category_probe

class MineralSample:
    pass

def inspect_record(specimen):
    if category_probe(
        specimen,
    ) is MineralSample:
        return specimen
    return None
"#,
        "inspect_record",
    );
    assert_eq!(
        type_of_guards(&guards),
        vec![("specimen".to_owned(), "MineralSample".to_owned())],
        "the renamed builtin and reformatted call must resolve to the type narrowing guard"
    );
}

#[test]
fn assert_hasattr_narrows_after_binding_resolution() {
    let guards = function_guards(
        r#"
from builtins import hasattr as attribute_probe

def inspect_record(specimen):
    assert attribute_probe(
        specimen,
        "crystal_depth",
    )
    return specimen.crystal_depth
"#,
        "inspect_record",
    );
    let asserted: Vec<(String, String)> = guards
        .iter()
        .filter_map(|guard| match &guard.kind {
            NarrowingGuardKind::Assert { inner } => match inner.as_ref() {
                NarrowingGuardKind::HasAttr {
                    variable,
                    attribute,
                    ..
                } => Some((variable.clone(), attribute.clone())),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        asserted,
        vec![("specimen".to_owned(), "crystal_depth".to_owned())],
        "the assert guard must resolve the renamed builtin before narrowing subsequent flow"
    );
}
