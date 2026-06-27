//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! E2E tests for error codes E0010 through E0025.
//!
//! Includes both exact-diagnostic tests and presence-check tests for
//! rules that are partially implemented.

mod common;

use basilisk_test_utils::{assert_diagnostics, Expected};
use common::{fixture, run};

// ---------------------------------------------------------------------------
// E0010 — import from untyped module
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
// W0014 — explicit Any without justification (split from E0011)
// ---------------------------------------------------------------------------

#[test]
fn explicit_any_in_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0011_explicit_any.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-W0014"),
        "should emit W0014 for explicit Any annotations, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// W0014 — Any on vararg, kwarg, and return annotation (split from E0011)
// ---------------------------------------------------------------------------

#[test]
fn any_on_vararg_kwarg_and_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0011_vararg_kwarg_any.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0011_vararg_kwarg_any.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::warning("BSK-W0014", "return annotation", 4, 5),
            Expected::warning("BSK-W0014", "`args`", 4, 14),
            Expected::warning("BSK-W0014", "`kwargs`", 4, 27),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0013 — return type mismatch (-> None returning value)
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
// E0014 — assignment type incompatibility (literal mismatches)
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
// E0014 — bytes literal, float literal, and int-to-bytes mismatches
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
// E0015 — invalid type argument count
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
// E0015 — set, frozenset, and dict with wrong type argument counts
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
// E0020 — @overload without implementation
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
// E0020 — exact diagnostic: two @overload variants with no implementation
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
// E0021 — overlapping @overload signatures
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
// E0021 — exact diagnostics: overlapping overloads also trigger E0001
// ---------------------------------------------------------------------------

#[test]
fn exact_diagnostics_for_overlapping_overloads() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0021_overlapping_overloads.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0021_overlapping_overloads.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`x`", 5, 13),
            Expected::error("overloads_consistency", "`process`", 9, 5),
            // The second overload returns `str`, not assignable to the impl's `int`.
            Expected::error("overloads_consistency_3", "`process`", 9, 5),
            Expected::error("BSK-E0001", "`x`", 9, 13),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0023 — non-exhaustive match (no wildcard case)
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
// E0024 — invalid type form in annotation
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
// E0024 — numeric literal on vararg, kwarg, and return annotation
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
// E0025 — method override without @override decorator
// ---------------------------------------------------------------------------

#[test]
fn override_without_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0025_missing_override.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0025"),
        "should emit E0025 for override without @override, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// FAILING TESTS — rules not yet implemented (Phase 1 limitations)
// These tests document desired behavior and fail to mark missing functionality.
// ---------------------------------------------------------------------------

/// E0012: Argument type mismatch.
/// Requires a type inference engine — not implemented in Phase 1.
#[test]
fn argument_type_mismatch_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0012_wrong_arg_type.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "calls_argument_type"),
        "E0012 (argument type mismatch) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0016: Incompatible method override (type-level).
/// Requires class hierarchy + type inference — not implemented in Phase 1.
#[test]
fn incompatible_method_override_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>>
{
    let diags = run("errors/e0016_incompatible_override.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "classes_override"),
        "E0016 (incompatible override) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0017: Incompatible variable override.
/// Requires type inference for variable types — not implemented in Phase 1.
#[test]
fn incompatible_variable_override_not_yet_implemented(
) -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0017_variable_override.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "classes_override_2"),
        "E0017 (incompatible variable override) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0018: Undefined variable.
/// Requires full scope analysis of expressions — not implemented in Phase 1.
#[test]
fn undefined_variable_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0018_undefined_variable.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "names_undefined"),
        "E0018 (undefined variable) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0019: Unbound variable on some code paths.
/// Requires full flow analysis — not implemented in Phase 1.
#[test]
fn unbound_variable_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0019_unbound_variable.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "names_unbound"),
        "E0019 (unbound variable) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0022: Unhashable type in hash-requiring context.
/// Requires type inference — not implemented in Phase 1.
#[test]
fn unhashable_type_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0022_unhashable_type.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "dict_key_hashable"),
        "E0022 (unhashable type) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}
