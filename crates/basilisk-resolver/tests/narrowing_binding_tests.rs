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
from builtins import hasattr as ha

def probe(x):
    if ha(x, "field"):
        return x.field
    return None
"#,
        "probe",
    );
    assert_eq!(
        hasattr_guards(&guards),
        vec![("x".to_owned(), "field".to_owned())],
        "an aliased import of `hasattr` is `hasattr`; recognition must come \
         from binding resolution, not the callee's spelling"
    );
}

#[test]
fn shadowed_hasattr_does_not_narrow() {
    let guards = function_guards(
        r#"
def hasattr(obj, name):
    return False

def probe(x):
    if hasattr(x, "field"):
        return x.field
    return None
"#,
        "probe",
    );
    assert_eq!(
        hasattr_guards(&guards),
        Vec::<(String, String)>::new(),
        "a module-level `def hasattr` shadows the builtin; treating the \
         spelling as the builtin fabricates narrowing"
    );
}

#[test]
fn shadowed_type_does_not_narrow() {
    let guards = function_guards(
        r#"
def type(x):
    return 0

class C:
    pass

def probe(x):
    if type(x) is C:
        return x
    return None
"#,
        "probe",
    );
    assert_eq!(
        type_of_guards(&guards),
        Vec::<(String, String)>::new(),
        "a module-level `def type` shadows the builtin; `type(x) is C` must \
         not narrow when `type` is not the builtin"
    );
}

#[test]
fn qualified_type_still_narrows() {
    let guards = function_guards(
        r#"
import builtins

class C:
    pass

def probe(x):
    if builtins.type(x) is C:
        return x
    return None
"#,
        "probe",
    );
    assert_eq!(
        type_of_guards(&guards),
        vec![("x".to_owned(), "C".to_owned())],
        "`builtins.type` is `type`; qualification must not change recognition"
    );
}

#[test]
fn assert_hasattr_narrows_after_binding_resolution() {
    let guards = function_guards(
        r#"
def probe(x):
    assert hasattr(x, "field")
    return x.field
"#,
        "probe",
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
        vec![("x".to_owned(), "field".to_owned())],
        "`assert hasattr(x, ...)` narrows subsequent flow (§7.8); the call \
         guard must be produced through binding resolution"
    );
}
