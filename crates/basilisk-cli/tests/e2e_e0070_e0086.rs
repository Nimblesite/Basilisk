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
fn e0070_never_type_compat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0070_never_type_compat.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0070")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0070 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0071 — Historical positional-only parameter violations
// ---------------------------------------------------------------------------

#[test]
fn e0071_historical_positional() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0071_historical_positional.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0071")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0071 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0072 — No matching overload for subscript indexing
// ---------------------------------------------------------------------------

#[test]
fn e0072_no_matching_overload() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0072_no_matching_overload.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0072")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0072 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0073 — NamedTuple-to-tuple type incompatibility
// ---------------------------------------------------------------------------

#[test]
fn e0073_namedtuple_tuple_compat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0073_namedtuple_tuple_compat.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0073")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0073 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0074 — Constructor call type mismatch with specialized generic
// ---------------------------------------------------------------------------

#[test]
fn e0074_constructor_new_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0074_constructor_new_mismatch.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0074")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0074 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0075 — Incompatible type for Self-typed attribute
// ---------------------------------------------------------------------------

#[test]
fn e0075_self_type_attr_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0075_self_type_attr_incompat.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0075")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0075 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0076 — Overload union expansion failure
// ---------------------------------------------------------------------------

#[test]
fn e0076_overload_union_expansion() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0076_overload_union_expansion.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0076")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0076 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0077 — Protocol Self-return conformance violation
// ---------------------------------------------------------------------------

#[test]
fn e0077_protocol_self_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0077_protocol_self_return.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0077")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0077 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0078 — Self type violations in generics
// ---------------------------------------------------------------------------

#[test]
fn e0078_self_type_violation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0078_self_type_violation.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0078")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0078 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0079 — Module assigned to incompatible protocol type
// ---------------------------------------------------------------------------

#[test]
fn e0079_module_protocol_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0079_module_protocol_incompat.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0079")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0079 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0080 — TypeVar upper bound violation at call site
// ---------------------------------------------------------------------------

#[test]
fn e0080_typevar_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0080_typevar_bound_violation.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0080")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0080 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0081 — TypeVarTuple unpack minimum type argument violation
// ---------------------------------------------------------------------------

#[test]
fn e0081_typevartuple_unpack_min() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0081_typevartuple_unpack_min.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0081")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0081 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0082 — TypeVarTuple callable/tuple argument mismatch
// ---------------------------------------------------------------------------

#[test]
fn e0082_typevartuple_callable_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0082_typevartuple_callable_mismatch.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0082")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0082 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0083 — TypeVarTuple must be unpacked with * operator
// ---------------------------------------------------------------------------

#[test]
fn e0083_typevartuple_unpack_required() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0083_typevartuple_unpack_required.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0083")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0083 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0084 — TypeVarTuple variance/bounds/constraints violation
// ---------------------------------------------------------------------------

#[test]
fn e0084_typevartuple_invalid_params() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0084_typevartuple_invalid_params.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0084")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0084 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0085 — TypeVarTuple argument count mismatch
// ---------------------------------------------------------------------------

#[test]
fn e0085_typevartuple_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0085_typevartuple_arg_count.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0085")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0085 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0086 — Multiple TypeVarTuple declarations in generic
// ---------------------------------------------------------------------------

#[test]
fn e0086_multiple_typevartuple() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0086_multiple_typevartuple.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0086")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0086 diagnostic"
    );
    Ok(())
}
