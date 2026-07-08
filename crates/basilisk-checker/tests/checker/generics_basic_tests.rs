//! Tests for [`generics_basic`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for generics_basic: `TypeVar` with single constraint.

use super::common::*;

#[test]
fn single_constraint_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", int)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic"),
        "TypeVar with single constraint should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn two_constraints_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", int, str)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_basic"),
        "TypeVar with two constraints should not fire E0026"
    );
    Ok(())
}

#[test]
fn unconstrained_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_basic"),
        "unconstrained TypeVar should not fire E0026"
    );
    Ok(())
}

#[test]
fn concrete_generic_bound_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Regression test for issue #283: `Callable[..., Any]` contains no type
    // variables, so it is a valid PEP 484 bound — no diagnostic.
    let source = r#"
from typing import Any, Callable, TypeVar
CallableT = TypeVar("CallableT", bound=Callable[..., Any])
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_basic"),
        "bound not parameterized by a type variable should not fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn typevar_parameterized_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    // PEP 484: a bound parameterized by a type variable (`list[T]`) is invalid.
    let source = r#"
from typing import TypeVar
T = TypeVar("T")
U = TypeVar("U", bound=list[T])
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic"),
        "bound parameterized by a type variable should fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn name_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
MyT = TypeVar("T")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic"),
        "TypeVar name mismatch should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn constraints_and_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", int, str, bound=object)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic"),
        "TypeVar with constraints and bound should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn parameterized_constraint_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T")
U = TypeVar("U", list[T], dict[str, T])
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic"),
        "TypeVar with parameterized constraint should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn parameterized_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T")
U = TypeVar("U", bound=list[T])
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic"),
        "TypeVar with parameterized bound should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn typevartuple_name_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple
WrongName = TypeVarTuple("Ts")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic"),
        "TypeVarTuple name mismatch should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn paramspec_name_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec
WrongName = ParamSpec("P")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic"),
        "ParamSpec name mismatch should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
