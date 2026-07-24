//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Clean fixture tests — zero diagnostics expected.
//!
//! These tests verify that fully-typed Python files produce no diagnostics.

mod common;

use common::run;

// ---------------------------------------------------------------------------
// Clean fixtures — zero diagnostics expected
// ---------------------------------------------------------------------------

#[test]
fn clean_fully_typed_module_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/fully_typed_module.py")?;
    assert!(
        diags.is_empty(),
        "fully_typed_module.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_with_varargs_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_with_varargs.py")?;
    assert!(
        diags.is_empty(),
        "typed_with_varargs.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_nested_functions_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/nested_functions.py")?;
    assert!(
        diags.is_empty(),
        "nested_functions.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Clean fixtures — additional patterns, zero diagnostics expected
// ---------------------------------------------------------------------------

#[test]
fn clean_typed_generics_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_generics.py")?;
    assert!(
        diags.is_empty(),
        "typed_generics.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_optional_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_optional.py")?;
    assert!(
        diags.is_empty(),
        "typed_optional.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_inheritance_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_inheritance.py")?;
    assert!(
        diags.is_empty(),
        "typed_inheritance.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_dataclass_style_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_dataclass_style.py")?;
    assert!(
        diags.is_empty(),
        "typed_dataclass_style.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_control_flow_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_control_flow.py")?;
    assert!(
        diags.is_empty(),
        "typed_control_flow.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Clean fixtures — control flow and exception handling
// ---------------------------------------------------------------------------

#[test]
fn clean_typed_try_except_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_try_except.py")?;
    assert!(
        diags.is_empty(),
        "typed_try_except.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_while_for_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_while_for.py")?;
    assert!(
        diags.is_empty(),
        "typed_while_for.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_with_statement_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_with_statement.py")?;
    assert!(
        diags.is_empty(),
        "typed_with_statement.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Clean — overloads with different arities must not trigger E0020 or E0021
// ---------------------------------------------------------------------------

#[test]
fn clean_overloads_different_arity_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_overloads_multi_arity.py")?;
    assert!(
        diags.is_empty(),
        "overloads with different arities must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Clean fixtures for new rules — must produce zero diagnostics
// ---------------------------------------------------------------------------

#[test]
fn clean_typed_module_vars_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_module_vars.py")?;
    assert!(
        diags.is_empty(),
        "typed_module_vars.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_class_attrs_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_class_attrs.py")?;
    assert!(
        diags.is_empty(),
        "typed_class_attrs.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_overloads_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_overloads.py")?;
    assert!(
        diags.is_empty(),
        "typed_overloads.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_override_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_override.py")?;
    assert!(
        diags.is_empty(),
        "typed_override.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_match_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_match.py")?;
    assert!(
        diags.is_empty(),
        "typed_match.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// stdlib imports must NOT trigger E0010
// ---------------------------------------------------------------------------

#[test]
fn clean_stdlib_imports_are_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_stdlib_imports.py")?;
    let e0010: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "imports_unresolved")
        .collect();
    assert!(
        e0010.is_empty(),
        "stdlib imports must not produce E0010, got:\n{e0010:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// concrete annotations must NOT trigger the explicit-Any warning
// ---------------------------------------------------------------------------

#[test]
fn clean_concrete_annotations_no_any_warning() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_any_justified.py")?;
    let any_warnings: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0014").collect();
    assert!(
        any_warnings.is_empty(),
        "concrete annotations must not produce the BSK-0014 explicit-Any warning, got:\n{any_warnings:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// match with wildcard must NOT trigger E0023
// ---------------------------------------------------------------------------

#[test]
fn clean_match_with_wildcard_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_match.py")?;
    let e0023: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "match_exhaustiveness")
        .collect();
    assert!(
        e0023.is_empty(),
        "match with wildcard must not produce E0023, got:\n{e0023:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// override WITH @override must NOT trigger BSK-0025
// ---------------------------------------------------------------------------

#[test]
fn clean_override_with_decorator_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_override.py")?;
    let e0025: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        e0025.is_empty(),
        "override with @override must not produce BSK-0025, got:\n{e0025:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// proper @overload with implementation must NOT trigger E0020
// ---------------------------------------------------------------------------

#[test]
fn clean_overloads_with_implementation_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_overloads.py")?;
    let e0020: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_definitions")
        .collect();
    assert!(
        e0020.is_empty(),
        "properly implemented overloads must not produce E0020, got:\n{e0020:#?}"
    );
    Ok(())
}
