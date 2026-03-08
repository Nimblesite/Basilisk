//! Integration tests for BSK-E0055: Invalid TypeVar/TypeVarTuple/ParamSpec kwargs.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0055_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0055")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn e0055_covariant_and_contravariant_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", covariant=True, contravariant=True)
"#;
    let msgs = e0055_messages(&run(source)?);
    assert!(
        msgs.iter()
            .any(|m| m.contains("both covariant and contravariant")),
        "covariant+contravariant should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0055_infer_variance_with_covariant_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", covariant=True, infer_variance=True)
"#;
    let msgs = e0055_messages(&run(source)?);
    assert!(
        msgs.iter().any(|m| m.contains("infer_variance")),
        "infer_variance + covariant should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0055_constraints_with_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", str, int, bound=float)
"#;
    let msgs = e0055_messages(&run(source)?);
    assert!(
        msgs.iter()
            .any(|m| m.contains("constraints") && m.contains("bound")),
        "constraints + bound should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0055_valid_typevar_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", covariant=True)
"#;
    let msgs = e0055_messages(&run(source)?);
    assert!(msgs.is_empty(), "valid TypeVar should not fire E0055");
    Ok(())
}

#[test]
fn e0055_typevartuple_with_covariant_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple
Ts = TypeVarTuple("Ts", covariant=True)
"#;
    let msgs = e0055_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "TypeVarTuple with covariant should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0055_typevartuple_with_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple
Ts = TypeVarTuple("Ts", bound=int)
"#;
    let msgs = e0055_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "TypeVarTuple with bound should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0055_paramspec_with_covariant_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec
P = ParamSpec("P", covariant=True)
"#;
    let msgs = e0055_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "ParamSpec with covariant should fire E0055, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0055_paramspec_with_bound_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ParamSpec
P = ParamSpec("P", bound=int)
"#;
    let msgs = e0055_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "ParamSpec with bound should fire E0055, got: {msgs:?}"
    );
    Ok(())
}
