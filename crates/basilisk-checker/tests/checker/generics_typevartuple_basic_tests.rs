//! Tests for [generics_typevartuple_basic] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// Integration tests for generics_typevartuple_basic: Invalid `TypeVar`/`TypeVarTuple`/`ParamSpec` kwargs.

use super::common::*;

#[test]
fn covariant_and_contravariant_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", covariant=True, contravariant=True)
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_typevartuple_basic");
    assert!(
        msgs.iter()
            .any(|m| m.contains("both covariant and contravariant")),
        "covariant+contravariant should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn infer_variance_with_covariant_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", covariant=True, infer_variance=True)
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_typevartuple_basic");
    assert!(
        msgs.iter().any(|m| m.contains("infer_variance")),
        "infer_variance + covariant should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn constraints_with_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", str, int, bound=float)
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_typevartuple_basic");
    assert!(
        msgs.iter()
            .any(|m| m.contains("constraints") && m.contains("bound")),
        "constraints + bound should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn valid_typevar_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", covariant=True)
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_typevartuple_basic");
    assert!(msgs.is_empty(), "valid TypeVar should not fire E0055");
    Ok(())
}

#[test]
fn typevartuple_with_covariant_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple
Ts = TypeVarTuple("Ts", covariant=True)
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_typevartuple_basic");
    assert!(
        !msgs.is_empty(),
        "TypeVarTuple with covariant should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn typevartuple_with_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple
Ts = TypeVarTuple("Ts", bound=int)
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_typevartuple_basic");
    assert!(
        !msgs.is_empty(),
        "TypeVarTuple with bound should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn paramspec_with_covariant_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec
P = ParamSpec("P", covariant=True)
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_typevartuple_basic");
    assert!(
        !msgs.is_empty(),
        "ParamSpec with covariant should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn paramspec_with_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec
P = ParamSpec("P", bound=int)
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_typevartuple_basic");
    assert!(
        !msgs.is_empty(),
        "ParamSpec with bound should fire E0055, got: {msgs:?}"
    );
    Ok(())
}
