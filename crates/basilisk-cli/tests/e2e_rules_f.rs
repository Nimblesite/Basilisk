//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! E2E tests for error codes E0088 through E0100.

mod common;

use common::run;

// ---------------------------------------------------------------------------
// TypedDict runtime violation (isinstance)
// ---------------------------------------------------------------------------

#[test]
fn typeddict_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0088_typeddict_isinstance.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_usage")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one typeddicts_usage diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Invalid PEP 695 type parameter bound or constraint
// ---------------------------------------------------------------------------

#[test]
fn pep695_invalid_bound() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0089_pep695_invalid_bound.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_syntax_declarations")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_syntax_declarations diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Invalid tuple type syntax
// ---------------------------------------------------------------------------

#[test]
fn invalid_tuple_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0090_invalid_tuple_syntax.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "tuples_type_form_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one tuples_type_form_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Incompatible TypeVar bound/constraint with default
// ---------------------------------------------------------------------------

#[test]
fn typevar_default_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0091_typevar_default_incompat.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_defaults_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_defaults_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Too few type arguments to generic class
// ---------------------------------------------------------------------------

#[test]
fn too_few_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0092_too_few_type_args.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_defaults_specialization")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_defaults_specialization diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Invalid key or value type in TypedDict assignment
// ---------------------------------------------------------------------------

#[test]
fn typeddict_key_validation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0093_typeddict_key_validation.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_operations")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one typeddicts_operations diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Self type used in an invalid location
// ---------------------------------------------------------------------------

#[test]
fn self_type_invalid_location() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0094_self_type_invalid_location.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_self_usage")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_self_usage diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// InitVar field validation in dataclasses
// ---------------------------------------------------------------------------

#[test]
fn initvar_field() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0095_initvar_field.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "dataclasses_postinit")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one dataclasses_postinit diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Dataclass field default_factory type mismatch
// ---------------------------------------------------------------------------

#[test]
fn dataclass_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0096_dataclass_default_factory.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "dataclasses_usage")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one dataclasses_usage diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Protocol __new__/__init__ sets undeclared self-attributes
// ---------------------------------------------------------------------------

#[test]
fn protocol_self_attr() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0097_protocol_self_attr.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "protocols_definition")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one protocols_definition diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Non-Protocol base class in Protocol definition
// ---------------------------------------------------------------------------

#[test]
fn non_protocol_base() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0098_non_protocol_base.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "protocols_merging")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one protocols_merging diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Direct instantiation of a Protocol class
// ---------------------------------------------------------------------------

#[test]
fn protocol_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0099_protocol_instantiation.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "protocols_explicit")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one protocols_explicit diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Augmented assignment widens Literal type
// ---------------------------------------------------------------------------

#[test]
fn literal_augmented_assign() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0100_literal_augmented_assign.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "literals_semantics")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one literals_semantics diagnostic"
    );
    Ok(())
}
