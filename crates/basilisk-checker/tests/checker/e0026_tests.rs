// Integration tests for BSK-E0026: `TypeVar` with single constraint.

use super::common::*;

#[test]
fn e0026_single_constraint_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", int)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0026"),
        "TypeVar with single constraint should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0026_two_constraints_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", int, str)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0026"),
        "TypeVar with two constraints should not fire E0026"
    );
    Ok(())
}

#[test]
fn e0026_unconstrained_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0026"),
        "unconstrained TypeVar should not fire E0026"
    );
    Ok(())
}

#[test]
fn e0026_name_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
MyT = TypeVar("T")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0026"),
        "TypeVar name mismatch should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0026_constraints_and_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", int, str, bound=object)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0026"),
        "TypeVar with constraints and bound should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0026_parameterized_constraint_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T")
U = TypeVar("U", list[T], dict[str, T])
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0026"),
        "TypeVar with parameterized constraint should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0026_parameterized_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T")
U = TypeVar("U", bound=list[T])
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0026"),
        "TypeVar with parameterized bound should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0026_typevartuple_name_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple
WrongName = TypeVarTuple("Ts")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0026"),
        "TypeVarTuple name mismatch should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0026_paramspec_name_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec
WrongName = ParamSpec("P")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0026"),
        "ParamSpec name mismatch should fire E0026, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
