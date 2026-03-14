//! Tests for resolver: `test_mutant_typevar`.

mod common;

use common::resolve_src;

#[test]
fn collect_typevar_calls_returns_entries() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar\n",
        "T = TypeVar('T', int, str)\n",
        "S = TypeVar('S')\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.typevar_calls.len(),
        2,
        "both TypeVars must be collected"
    );
    Ok(())
}

#[test]
fn collect_typevar_calls_plain_assign_arm() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', int, str)\n".to_owned();
    let resolved = resolve_src(&src)?;
    let tv = resolved
        .typevar_calls
        .iter()
        .find(|t| t.name == "T")
        .ok_or("T not found")?;
    assert_eq!(tv.constraint_count, 2);
    Ok(())
}

#[test]
fn collect_typevar_calls_ann_assign_arm() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar\n",
        "T: TypeVar = TypeVar('T', int, str)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typevar_calls.is_empty(),
        "annotated TypeVar assignment must be collected"
    );
    Ok(())
}

#[test]
fn collect_typevar_calls_qualified_typing_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = typing.TypeVar('T', int, str)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typevar_calls.is_empty(),
        "typing.TypeVar must be collected"
    );
    let tv = &resolved.typevar_calls[0];
    assert_eq!(tv.name, "T");
    assert_eq!(tv.constraint_count, 2);
    Ok(())
}

#[test]
fn collect_typevar_calls_ignores_non_typevar_calls() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = int('T')\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.typevar_calls.is_empty(),
        "non-TypeVar call must not be collected"
    );
    Ok(())
}

#[test]
fn collect_typevar_calls_ann_assign_ignores_non_typevar() -> Result<(), Box<dyn std::error::Error>>
{
    let src = "T: int = int('T')\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.typevar_calls.is_empty(),
        "non-TypeVar ann-assign must not be collected"
    );
    Ok(())
}

#[test]
fn collect_typevar_calls_has_default_true() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', default=int)\n".to_owned();
    let resolved = resolve_src(&src)?;
    let tv = resolved
        .typevar_calls
        .iter()
        .find(|t| t.name == "T")
        .ok_or("T not found")?;
    assert!(
        tv.has_default,
        "TypeVar with default= must have has_default=true"
    );
    Ok(())
}

#[test]
fn collect_typevar_calls_has_default_false_when_absent() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', int, str)\n".to_owned();
    let resolved = resolve_src(&src)?;
    let tv = resolved
        .typevar_calls
        .iter()
        .find(|t| t.name == "T")
        .ok_or("T not found")?;
    assert!(
        !tv.has_default,
        "TypeVar without default= must have has_default=false"
    );
    Ok(())
}

#[test]
fn collect_typevar_calls_ann_assign_has_default() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar\n",
        "T: TypeVar = TypeVar('T', default=int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let tv = resolved
        .typevar_calls
        .iter()
        .find(|t| t.name == "T")
        .ok_or("T not found")?;
    assert!(
        tv.has_default,
        "annotated TypeVar with default= must have has_default=true"
    );
    Ok(())
}
