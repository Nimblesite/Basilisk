//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! E2E tests for error codes E0010 through BSK-0025.
//!
//! Includes both exact-diagnostic tests and presence-check tests for
//! rules that are partially implemented.

mod common;

use basilisk_test_utils::{assert_diagnostics, Expected};
use common::{
    annotation_rules_config, annotation_rules_config_for_python, fixture, run, run_with_config,
};

// ---------------------------------------------------------------------------
// import from untyped module
// ---------------------------------------------------------------------------

#[test]
fn import_from_untyped_module() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0010_untyped_import.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"imports_unresolved"),
        "should emit E0010 for untyped imports, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// explicit Any without justification (split from E0011)
// ---------------------------------------------------------------------------

#[test]
fn explicit_any_in_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("errors/e0011_explicit_any.py", &annotation_rules_config())?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-0014"),
        "should emit BSK-0014 for explicit Any annotations, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Any on vararg, kwarg, and return annotation (split from E0011)
// ---------------------------------------------------------------------------

#[test]
fn any_on_vararg_kwarg_and_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config(
        "errors/e0011_vararg_kwarg_any.py",
        &annotation_rules_config(),
    )?;
    let src = std::fs::read_to_string(fixture("errors/e0011_vararg_kwarg_any.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::warning("BSK-0014", "return annotation", 4, 5),
            Expected::warning("BSK-0014", "`args`", 4, 14),
            Expected::warning("BSK-0014", "`kwargs`", 4, 27),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// return type mismatch (-> None returning value)
// ---------------------------------------------------------------------------

#[test]
fn none_annotated_returning_value() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0013_return_mismatch.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"returns_compatibility_2"),
        "should emit E0013 when -> None function returns a value, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// assignment type incompatibility (literal mismatches)
// ---------------------------------------------------------------------------

#[test]
fn literal_assigned_to_incompatible_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0014_assignment_incompatible.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"assignment_compatibility"),
        "should emit E0014 for literal type mismatches, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// bytes literal, float literal, and int-to-bytes mismatches
// ---------------------------------------------------------------------------

#[test]
fn bytes_and_float_mismatches() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0014_bytes_float_mismatches.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0014_bytes_float_mismatches.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("assignment_compatibility", "`ratio`", 1, 1),
            Expected::error("assignment_compatibility", "`name`", 2, 1),
            Expected::error("assignment_compatibility", "`raw`", 3, 1),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// invalid type argument count
// ---------------------------------------------------------------------------

#[test]
fn invalid_type_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0015_invalid_type_arg.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"callables_annotation"),
        "should emit E0015 for invalid generic arg count, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// set, frozenset, and dict with wrong type argument counts
// ---------------------------------------------------------------------------

#[test]
fn set_frozenset_and_dict_wrong_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0015_more_generics.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0015_more_generics.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("callables_annotation", "`set[", 1, 11),
            Expected::error("callables_annotation", "`frozenset[", 5, 17),
            Expected::error("callables_annotation", "`data`", 9, 18),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// @overload without implementation
// ---------------------------------------------------------------------------

#[test]
fn overload_missing_implementation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0020_missing_overload_impl.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"overloads_definitions"),
        "should emit E0020 when @overload has no implementation, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// exact diagnostic: two @overload variants with no implementation
// ---------------------------------------------------------------------------

#[test]
fn exact_diagnostic_for_double() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0020_missing_overload_impl.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0020_missing_overload_impl.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("overloads_definitions", "`double`", 5, 5)],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// overlapping @overload signatures
// ---------------------------------------------------------------------------

#[test]
fn overlapping_overload_signatures() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0021_overlapping_overloads.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"overloads_consistency"),
        "should emit E0021 for overlapping overload signatures, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// exact diagnostics: overlapping overloads also trigger BSK-0001
// ---------------------------------------------------------------------------

#[test]
fn exact_diagnostics_for_overlapping_overloads() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config(
        "errors/e0021_overlapping_overloads.py",
        &annotation_rules_config(),
    )?;
    let src = std::fs::read_to_string(fixture("errors/e0021_overlapping_overloads.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-0001", "`x`", 5, 13),
            Expected::error("overloads_consistency", "`process`", 9, 5),
            // The second overload returns `str`, not assignable to the impl's `int`.
            Expected::error("overloads_consistency_3", "`process`", 9, 5),
            Expected::error("BSK-0001", "`x`", 9, 13),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// non-exhaustive match (no wildcard case)
// ---------------------------------------------------------------------------

#[test]
fn match_without_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0023_nonexhaustive_match.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"match_exhaustiveness"),
        "should emit E0023 for match without wildcard, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// invalid type form in annotation
// ---------------------------------------------------------------------------

#[test]
fn numeric_literal_as_type_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0024_invalid_type_form.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"annotations_typeexpr"),
        "should emit E0024 for numeric literal used as type, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// numeric literal on vararg, kwarg, and return annotation
// ---------------------------------------------------------------------------

#[test]
fn numeric_literal_on_vararg_kwarg_and_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0024_vararg_kwarg_return_literal.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0024_vararg_kwarg_return_literal.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("annotations_typeexpr", "return type", 1, 5),
            Expected::error("annotations_typeexpr", "`args`", 1, 14),
            Expected::error("annotations_typeexpr", "`kwargs`", 1, 26),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// method override without @override decorator
// ---------------------------------------------------------------------------

#[test]
fn override_without_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config(
        "errors/e0025_missing_override.py",
        &annotation_rules_config_for_python("3.12"),
    )?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-0025"),
        "should emit BSK-0025 for override without @override, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Type-safety and flow diagnostics.
// ---------------------------------------------------------------------------

/// Argument type mismatch.
#[test]
fn argument_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0012_wrong_arg_type.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "calls_argument_type"),
        "expected calls_argument_type for an incompatible argument"
    );
    Ok(())
}

/// Incompatible method override (type-level).
#[test]
fn incompatible_method_override() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0016_incompatible_override.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "classes_override"),
        "expected classes_override for an incompatible method override"
    );
    Ok(())
}

/// Incompatible variable override.
#[test]
fn incompatible_variable_override() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0017_variable_override.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "classes_override_2"),
        "expected classes_override_2 for an incompatible variable override"
    );
    Ok(())
}

/// Undefined variable.
#[test]
fn undefined_variable() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0018_undefined_variable.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "names_undefined"),
        "expected names_undefined for an undefined variable"
    );
    Ok(())
}

/// Unbound variable on some code paths.
#[test]
fn unbound_variable() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0019_unbound_variable.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "names_unbound"),
        "expected names_unbound for a conditionally unbound variable"
    );
    Ok(())
}

/// Unhashable type in hash-requiring context.
#[test]
fn unhashable_type() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0022_unhashable_type.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "dict_key_hashable"),
        "expected dict_key_hashable for an unhashable dictionary key"
    );
    Ok(())
}
