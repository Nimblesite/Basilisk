//! E2E tests for error codes E0051 through E0069.

mod common;

use common::run;

// ---------------------------------------------------------------------------
// E0051 — Invalid Literal parameterization
// ---------------------------------------------------------------------------

#[test]
fn e0051_invalid_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0051_invalid_literal.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0051")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0051 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0052 — Frozen dataclass attribute assignment
// ---------------------------------------------------------------------------

#[test]
fn e0052_frozen_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0052_frozen_dataclass.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0052")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0052 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0053 — assert_type() type mismatch (may be disabled)
// ---------------------------------------------------------------------------

#[test]
fn e0053_assert_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // E0053 may be disabled pending full type inference; just verify the
    // fixture parses and runs through the pipeline without crashing.
    let _diags = run("errors/e0053_assert_type_mismatch.py")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// E0054 — Final reassignment
// ---------------------------------------------------------------------------

#[test]
fn e0054_final_reassignment() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0054_final_reassignment.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0054")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0054 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0055 — Invalid TypeVar keyword argument combination
// ---------------------------------------------------------------------------

#[test]
fn e0055_typevar_invalid_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0055_typevar_invalid_kwargs.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0055")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0055 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0056 — Mutation of ReadOnly TypedDict fields
// ---------------------------------------------------------------------------

#[test]
fn e0056_readonly_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0056_readonly_typeddict.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0056")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0056 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0057 — Invalid RHS in PEP 695 type alias
// ---------------------------------------------------------------------------

#[test]
fn e0057_pep695_type_alias_invalid() -> Result<(), Box<dyn std::error::Error>> {
    // E0057 depends on type_statements being populated by the resolver,
    // which is not yet implemented. Verify the fixture runs without crashing.
    let _diags = run("errors/e0057_pep695_type_alias_invalid.py")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// E0058 — Annotated requires at least two arguments
// ---------------------------------------------------------------------------

#[test]
fn e0058_annotated_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0058_annotated_too_few_args.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0058")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0058 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0059 — Access to __match_args__ on dataclass with match_args=False
// ---------------------------------------------------------------------------

#[test]
fn e0059_dataclass_match_args_false() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0059_dataclass_match_args_false.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0059")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0059 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0060 — Invalid ordering comparison of dataclass instances
// ---------------------------------------------------------------------------

#[test]
fn e0060_dataclass_ordering_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0060_dataclass_ordering_invalid.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0060")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0060 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0061 — assert_type with Literal[Enum.MEMBER] on enum-typed param
// ---------------------------------------------------------------------------

#[test]
fn e0061_assert_type_enum_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0061_assert_type_enum_literal.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0061 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0062 — NoReturn/Never function can fall through
// ---------------------------------------------------------------------------

#[test]
fn e0062_noreturn_fallthrough() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0062_noreturn_fallthrough.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0062")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0062 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0063 — Non-hashable dataclass assigned to Hashable
// ---------------------------------------------------------------------------

#[test]
fn e0063_non_hashable_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0063_non_hashable_dataclass.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0063")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0063 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0064 — Invalid argument in NamedTuple constructor
// ---------------------------------------------------------------------------

#[test]
fn e0064_namedtuple_invalid_arg() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0064_namedtuple_invalid_arg.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0064")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0064 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0065 — Access to int-only attribute on float-typed parameter
// ---------------------------------------------------------------------------

#[test]
fn e0065_float_param_int_attr() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0065_float_param_int_attr.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0065")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0065 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0066 — Enum member value incompatible with _value_ type
// ---------------------------------------------------------------------------

#[test]
fn e0066_enum_value_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0066_enum_value_type_mismatch.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0066")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0066 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0067 — Non-member referenced in Literal[EnumClass.X]
// ---------------------------------------------------------------------------

#[test]
fn e0067_enum_non_member_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0067_enum_non_member_literal.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0067")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0067 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0068 — Literal string used where enum member reference required
// ---------------------------------------------------------------------------

#[test]
fn e0068_literal_string_enum() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0068_literal_string_enum.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0068")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0068 diagnostic"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0069 — Dataclass keyword-only field violations
// ---------------------------------------------------------------------------

#[test]
fn e0069_dataclass_kwonly() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0069_dataclass_kwonly.py")?;
    let filtered: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0069")
        .collect();
    assert!(
        !filtered.is_empty(),
        "expected at least one BSK-E0069 diagnostic"
    );
    Ok(())
}
