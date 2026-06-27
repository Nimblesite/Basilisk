//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! E2E tests for error codes E0070 through E0086.

mod common;

use common::run;

// ---------------------------------------------------------------------------
// E0070 — Never type compatibility violations
// ---------------------------------------------------------------------------

#[test]
fn never_type_compat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0070_never_type_compat.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "specialtypes_never_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one specialtypes_never_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0071 — Historical positional-only parameter violations
// ---------------------------------------------------------------------------

#[test]
fn historical_positional() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0071_historical_positional.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "historical_positional")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one historical_positional diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0072 — No matching overload for subscript indexing
// ---------------------------------------------------------------------------

#[test]
fn no_matching_overload() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0072_no_matching_overload.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_basic")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one overloads_basic diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0073 — NamedTuple-to-tuple type incompatibility
// ---------------------------------------------------------------------------

#[test]
fn namedtuple_tuple_compat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0073_namedtuple_tuple_compat.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "namedtuples_type_compat")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one namedtuples_type_compat diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0074 — Constructor call type mismatch with specialized generic
// ---------------------------------------------------------------------------

#[test]
fn constructor_new_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0074_constructor_new_mismatch.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "constructors_call_new")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one constructors_call_new diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0075 — Incompatible type for Self-typed attribute
// ---------------------------------------------------------------------------

#[test]
fn self_type_attr_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0075_self_type_attr_incompat.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_self_attributes")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_self_attributes diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0076 — Overload union expansion failure
// ---------------------------------------------------------------------------

#[test]
fn overload_union_expansion() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0076_overload_union_expansion.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_evaluation")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one overloads_evaluation diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0077 — Protocol Self-return conformance violation
// ---------------------------------------------------------------------------

#[test]
fn protocol_self_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0077_protocol_self_return.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_self_protocols")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_self_protocols diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0078 — Self type violations in generics
// ---------------------------------------------------------------------------

#[test]
fn self_type_violation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0078_self_type_violation.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_self_basic")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_self_basic diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0079 — Module assigned to incompatible protocol type
// ---------------------------------------------------------------------------

#[test]
fn module_protocol_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0079_module_protocol_incompat.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "protocols_modules")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one protocols_modules diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0080 — TypeVar upper bound violation at call site
// ---------------------------------------------------------------------------

#[test]
fn typevar_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0080_typevar_bound_violation.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_upper_bound diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0081 — TypeVarTuple unpack minimum type argument violation
// ---------------------------------------------------------------------------

#[test]
fn typevartuple_unpack_min() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0081_typevartuple_unpack_min.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_typevartuple_unpack")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_typevartuple_unpack diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0082 — TypeVarTuple callable/tuple argument mismatch
// ---------------------------------------------------------------------------

#[test]
fn typevartuple_callable_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0082_typevartuple_callable_mismatch.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_typevartuple_callable")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_typevartuple_callable diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0083 — TypeVarTuple must be unpacked with * operator
// ---------------------------------------------------------------------------

#[test]
fn typevartuple_unpack_required() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0083_typevartuple_unpack_required.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_typevartuple_basic_2")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_typevartuple_basic_2 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0084 — TypeVarTuple variance/bounds/constraints violation
// ---------------------------------------------------------------------------

#[test]
fn typevartuple_invalid_params() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0084_typevartuple_invalid_params.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_typevartuple_basic_3")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_typevartuple_basic_3 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0085 — TypeVarTuple argument count mismatch
// ---------------------------------------------------------------------------

#[test]
fn typevartuple_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0085_typevartuple_arg_count.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_typevartuple_args")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_typevartuple_args diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0086 — Multiple TypeVarTuple declarations in generic
// ---------------------------------------------------------------------------

#[test]
fn multiple_typevartuple() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0086_multiple_typevartuple.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_typevartuple_specialization")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one generics_typevartuple_specialization diagnostic"
    );
    Ok(())
}
