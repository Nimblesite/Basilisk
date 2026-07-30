//! Parity pins for [`super::helpers::types_compatible`]
//! ([NARROWPLAN-SUBTYPING]): the accepted/rejected cases below are the
//! rule's CURRENT behavior and must not drift until the helper merges into
//! `crate::subtyping::SubtypingContext` behind these same expectations
//! ([NARROWPLAN-INTEGRATION]).

use super::helpers::types_compatible;

/// Identity, unknown (empty) sides, and `Any` all accept.
#[test]
fn identity_unknown_and_any_accept() {
    assert!(types_compatible("int", "int"));
    assert!(types_compatible("Self", "Self"));
    assert!(types_compatible("", "int"));
    assert!(types_compatible("int", ""));
    assert!(types_compatible("Any", "str"));
    assert!(types_compatible("str", "Any"));
}

/// The numeric acceptances are DELIBERATELY narrower than the shared tower:
/// `int` where `float` is expected and `bool` where `int` is expected — no
/// `complex`, no transitive `bool` → `float`.
#[test]
fn numeric_acceptances_are_the_two_listed_pairs() {
    assert!(types_compatible("float", "int"));
    assert!(types_compatible("int", "bool"));
    assert!(!types_compatible("complex", "float"));
    assert!(!types_compatible("float", "bool"));
    assert!(!types_compatible("int", "float"));
    assert!(!types_compatible("bool", "int"));
}

/// A capitalised, unparameterised expected type reads as a possibly
/// unsubstituted `TypeVar` and accepts conservatively; parameterised or
/// lowercase expectations do not.
#[test]
fn unresolved_typevar_heuristic_is_conservative() {
    assert!(types_compatible("T", "int"));
    assert!(types_compatible("MyClass", "int"));
    assert!(!types_compatible("List[int]", "int"));
    assert!(!types_compatible("str", "int"));
}
