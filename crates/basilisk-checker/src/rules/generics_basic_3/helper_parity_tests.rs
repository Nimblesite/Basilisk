//! Parity pins for [`super::helpers::is_subtype_of`]
//! ([NARROWPLAN-SUBTYPING]): the accepted/rejected cases below are the
//! rule's CURRENT behavior and must not drift until the helper merges into
//! `crate::subtyping::SubtypingContext` behind these same expectations
//! ([NARROWPLAN-INTEGRATION]).

use std::collections::HashMap;

use super::helpers::is_subtype_of;

fn no_classes() -> HashMap<String, Vec<String>> {
    HashMap::new()
}

/// The built-in table is DELIBERATELY narrower than the shared numeric
/// tower: only `bool <: int`. Widening it would accept constraint matches
/// this rule must flag.
#[test]
fn builtin_table_is_bool_int_only() {
    let classes = no_classes();
    assert!(is_subtype_of("bool", "int", &classes));
    assert!(!is_subtype_of("int", "float", &classes));
    assert!(!is_subtype_of("bool", "float", &classes));
    assert!(!is_subtype_of("float", "complex", &classes));
    assert!(!is_subtype_of("int", "bool", &classes));
    assert!(!is_subtype_of("str", "int", &classes));
}

/// Identity holds only through the class table or the built-in pair — a
/// name with no registered bases is NOT its own subtype here (callers test
/// equality separately before calling).
#[test]
fn identity_is_the_callers_job() {
    let classes = no_classes();
    assert!(!is_subtype_of("int", "int", &classes));
}

/// The nominal walk follows registered bases transitively.
#[test]
fn nominal_walk_is_transitive() {
    let mut classes = HashMap::new();
    let _ = classes.insert("Dog".to_owned(), vec!["Animal".to_owned()]);
    let _ = classes.insert("Puppy".to_owned(), vec!["Dog".to_owned()]);
    assert!(is_subtype_of("Dog", "Animal", &classes));
    assert!(is_subtype_of("Puppy", "Animal", &classes));
    assert!(!is_subtype_of("Animal", "Dog", &classes));
    assert!(!is_subtype_of("Cat", "Animal", &classes));
}
