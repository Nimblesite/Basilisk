//! Tests for [callables_annotation] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Tests for callables_annotation: Invalid type argument count for generic type.
//
// This rule detects when a generic type is subscripted with the wrong number
// of type arguments. For example:
// - `List[int, str]` (should be `List[int]`)
// - `Dict[str]` (should be `Dict[str, int]`)

use super::common::*;

#[test]
fn test_e0015_list_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: list[int, str]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        !e0015.is_empty(),
        "list with too many args should fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_dict_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: dict[str]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        !e0015.is_empty(),
        "dict with too few args should fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_dict_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: dict[str, int, float]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        !e0015.is_empty(),
        "dict with too many args should fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_tuple_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: tuple[int, str, float]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        e0015.is_empty(),
        "tuple with correct args should not fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_set_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: set[int]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        e0015.is_empty(),
        "set with correct args should not fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_set_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: set[int, str]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        !e0015.is_empty(),
        "set with too many args should fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_optional_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: optional[int]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        e0015.is_empty(),
        "optional with correct args should not fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_optional_too_many_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: optional[int, str]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        !e0015.is_empty(),
        "optional with too many args should fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_union_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: union[int, str, float]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        e0015.is_empty(),
        "union with correct args should not fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_callable_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: callable[[int, str], bool]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        e0015.is_empty(),
        "callable with correct args should not fire E0015"
    );
    Ok(())
}

#[test]
fn test_e0015_callable_malformed_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
x: callable[int, str, bool]
";
    let diags = run(src)?;
    let e0015: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        !e0015.is_empty(),
        "callable with malformed args should fire E0015"
    );
    Ok(())
}
