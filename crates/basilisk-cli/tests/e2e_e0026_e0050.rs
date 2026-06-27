//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! E2E tests for error codes E0026 through E0050.

mod common;

use common::run;

// ---------------------------------------------------------------------------
// E0026 — TypeVar with single constraint
// ---------------------------------------------------------------------------

#[test]
fn e0026_typevar_single_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0026_typevar_single_constraint.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_basic")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_basic diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0027 — Duplicate TypeVar in Generic[...]
// ---------------------------------------------------------------------------

#[test]
fn e0027_duplicate_typevar_generic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0027_duplicate_typevar_generic.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_base_class")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_base_class diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0029 — Method defined inside a TypedDict
// ---------------------------------------------------------------------------

#[test]
fn e0029_typeddict_method() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0029_typeddict_method.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_class_syntax")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one typeddicts_class_syntax diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0030 — Non-default TypeVar follows default TypeVar in Generic[...]
// ---------------------------------------------------------------------------

#[test]
fn e0030_non_default_after_default() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0030_non_default_after_default.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_defaults")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_defaults diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0031 — Invalid cast() call
// ---------------------------------------------------------------------------

#[test]
fn e0031_invalid_cast() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0031_invalid_cast.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "directives_cast")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one directives_cast diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0032 — Invalid keyword argument in TypedDict class
// ---------------------------------------------------------------------------

#[test]
fn e0032_typeddict_invalid_keyword() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0032_typeddict_invalid_keyword.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_class_syntax_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one typeddicts_class_syntax_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0033 — Invalid reveal_type() call
// ---------------------------------------------------------------------------

#[test]
fn e0033_invalid_reveal_type() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0033_invalid_reveal_type.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "directives_reveal_type")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one directives_reveal_type diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0034 — @final decorator violations
// ---------------------------------------------------------------------------

#[test]
fn e0034_final_class_inherit() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0034_final_class_inherit.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "qualifiers_final_decorator")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one qualifiers_final_decorator diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0035 — Required/NotRequired used outside TypedDict
// ---------------------------------------------------------------------------

#[test]
fn e0035_required_outside_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0035_required_outside_typeddict.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_required")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one typeddicts_required diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0036 — ClassVar used in invalid context
// ---------------------------------------------------------------------------

#[test]
fn e0036_classvar_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0036_classvar_invalid.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_classvar")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one classes_classvar diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0037 — Invalid TypedDict functional syntax
// ---------------------------------------------------------------------------

#[test]
fn e0037_typeddict_functional_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0037_typeddict_functional_invalid.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_alt_syntax")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one typeddicts_alt_syntax diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0038 — Invalid TypedDict inheritance
// ---------------------------------------------------------------------------

#[test]
fn e0038_typeddict_inheritance_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0038_typeddict_inheritance_invalid.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_inheritance")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one typeddicts_inheritance diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0039 — Invalid assert_type() call
// ---------------------------------------------------------------------------

#[test]
fn e0039_invalid_assert_type() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0039_invalid_assert_type.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "directives_assert_type")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one directives_assert_type diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0040 — Invalid Enum subclassing
// ---------------------------------------------------------------------------

#[test]
fn e0040_enum_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0040_enum_subclass.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_behaviors")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one enums_behaviors diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0041 — Too few arguments in function call
// ---------------------------------------------------------------------------

#[test]
fn e0041_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0041_too_few_args.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_count")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one calls_argument_count diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0042 — PEP 695 type parameter mixed with traditional TypeVars
// ---------------------------------------------------------------------------

#[test]
fn e0042_pep695_mixed_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0042_pep695_mixed_typevar.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_syntax_compatibility")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_syntax_compatibility diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0043 — Non-TypeVar argument in Generic[...]
// ---------------------------------------------------------------------------

#[test]
fn e0043_non_typevar_in_generic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0043_non_typevar_in_generic.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_basic_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_basic_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0044 — Final used in invalid position
// ---------------------------------------------------------------------------

#[test]
fn e0044_final_invalid_position() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0044_final_invalid_position.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "qualifiers_final_annotation")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one qualifiers_final_annotation diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0045 — Invalid first argument to Annotated[...]
// ---------------------------------------------------------------------------

#[test]
fn e0045_annotated_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0045_annotated_invalid.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "qualifiers_annotated")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one qualifiers_annotated diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0046 — Enum member annotated with explicit type
// ---------------------------------------------------------------------------

#[test]
fn e0046_enum_member_annotated() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0046_enum_member_annotated.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "enums_members")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one enums_members diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0047 — Invalid type expression in annotation
// ---------------------------------------------------------------------------

#[test]
fn e0047_invalid_type_expr() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0047_invalid_type_expr.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "annotations_forward_refs")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one annotations_forward_refs diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0048 — Invalid RHS for TypeAlias
// ---------------------------------------------------------------------------

#[test]
fn e0048_typealias_invalid_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0048_typealias_invalid_rhs.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "aliases_implicit")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one aliases_implicit diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0049 — Multiple unbounded tuple components
// ---------------------------------------------------------------------------

#[test]
fn e0049_multiple_unbounded_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0049_multiple_unbounded_tuple.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "tuples_type_form")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one tuples_type_form diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0050 — Invalid NewType call
// ---------------------------------------------------------------------------

#[test]
fn e0050_invalid_newtype() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0050_invalid_newtype.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "aliases_newtype")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one aliases_newtype diagnostic"
    );
    Ok(())
}
