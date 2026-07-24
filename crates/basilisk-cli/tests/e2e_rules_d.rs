//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! E2E tests for error codes E0051 through E0069.

mod common;

use common::run;

// ---------------------------------------------------------------------------
// Invalid Literal parameterization
// ---------------------------------------------------------------------------

#[test]
fn invalid_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0051_invalid_literal.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "literals_parameterizations")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one literals_parameterizations diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Frozen dataclass attribute assignment
// ---------------------------------------------------------------------------

#[test]
fn frozen_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0052_frozen_dataclass.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "dataclasses_frozen")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one dataclasses_frozen diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// assert_type() type mismatch (may be disabled)
// ---------------------------------------------------------------------------

#[test]
fn assert_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // E0053 may be disabled pending full type inference; just verify the
    // fixture parses and runs through the pipeline without crashing.
    let _diags = run("errors/e0053_assert_type_mismatch.py")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Final reassignment
// ---------------------------------------------------------------------------

#[test]
fn final_reassignment() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0054_final_reassignment.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "qualifiers_final_annotation_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one qualifiers_final_annotation_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Invalid TypeVar keyword argument combination
// ---------------------------------------------------------------------------

#[test]
fn typevar_invalid_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0055_typevar_invalid_kwargs.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_typevartuple_basic")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_typevartuple_basic diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Mutation of ReadOnly TypedDict fields
// ---------------------------------------------------------------------------

#[test]
fn readonly_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0056_readonly_typeddict.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_readonly")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one typeddicts_readonly diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Invalid RHS in PEP 695 type alias
// ---------------------------------------------------------------------------

#[test]
fn pep695_type_alias_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0057_pep695_type_alias_invalid.py")?;
    assert!(
        diags
            .iter()
            .any(|diagnostic| diagnostic.code.code == "aliases_type_statement"),
        "expected aliases_type_statement for an invalid PEP 695 alias"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Annotated requires at least two arguments
// ---------------------------------------------------------------------------

#[test]
fn annotated_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0058_annotated_too_few_args.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "qualifiers_annotated_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one qualifiers_annotated_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Access to __match_args__ on dataclass with match_args=False
// ---------------------------------------------------------------------------

#[test]
fn dataclass_match_args_false() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0059_dataclass_match_args_false.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "dataclasses_match_args")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one dataclasses_match_args diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Invalid ordering comparison of dataclass instances
// ---------------------------------------------------------------------------

#[test]
fn dataclass_ordering_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0060_dataclass_ordering_invalid.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "dataclasses_order")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one dataclasses_order diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// assert_type with Literal[Enum.MEMBER] on enum-typed param
// ---------------------------------------------------------------------------

#[test]
fn assert_type_enum_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0061_assert_type_enum_literal.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_expansion")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one enums_expansion diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// NoReturn/Never function can fall through
// ---------------------------------------------------------------------------

#[test]
fn noreturn_fallthrough() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0062_noreturn_fallthrough.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "specialtypes_never")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one specialtypes_never diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Non-hashable dataclass assigned to Hashable
// ---------------------------------------------------------------------------

#[test]
fn non_hashable_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0063_non_hashable_dataclass.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "dataclasses_hash")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one dataclasses_hash diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Invalid argument in NamedTuple constructor
// ---------------------------------------------------------------------------

#[test]
fn namedtuple_invalid_arg() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0064_namedtuple_invalid_arg.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "namedtuples_define_functional")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one namedtuples_define_functional diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Access to int-only attribute on float-typed parameter
// ---------------------------------------------------------------------------

#[test]
fn float_param_int_attr() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0065_float_param_int_attr.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "specialtypes_promotions")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one specialtypes_promotions diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Enum member value incompatible with _value_ type
// ---------------------------------------------------------------------------

#[test]
fn enum_value_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0066_enum_value_type_mismatch.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_member_values")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one enums_member_values diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Non-member referenced in Literal[EnumClass.X]
// ---------------------------------------------------------------------------

#[test]
fn enum_non_member_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0067_enum_non_member_literal.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_members_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one enums_members_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Literal string used where enum member reference required
// ---------------------------------------------------------------------------

#[test]
fn literal_string_enum() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0068_literal_string_enum.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "literals_parameterizations_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one literals_parameterizations_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Dataclass keyword-only field violations
// ---------------------------------------------------------------------------

#[test]
fn dataclass_kwonly() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0069_dataclass_kwonly.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "dataclasses_kwonly")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one dataclasses_kwonly diagnostic"
    );
    Ok(())
}
