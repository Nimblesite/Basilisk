// Tests for BSK-E0089: Invalid PEP 695 type parameter bound.
//
// PEP 695 introduced a compact syntax for declaring generic classes:
// ```python
// class MyClass[T: int | str]: ...
// ```
//
// This rule detects invalid bounds in PEP 695 type parameter declarations.

use super::common::*;

#[test]
fn test_e0089_list_literal_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
class MyClass[T: [int, str]]:
    pass
";
    let diags = run(src)?;
    let e0089: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0089")
        .collect();
    assert!(!e0089.is_empty(), "list literal bound should fire E0089");
    Ok(())
}

#[test]
fn test_e0089_empty_tuple_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
class MyClass[T: ()]:
    pass
";
    let diags = run(src)?;
    let e0089: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0089")
        .collect();
    assert!(!e0089.is_empty(), "empty tuple bound should fire E0089");
    Ok(())
}

#[test]
fn test_e0089_single_element_tuple_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
class MyClass[T: (int,)]:
    pass
";
    let diags = run(src)?;
    let e0089: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0089")
        .collect();
    assert!(
        !e0089.is_empty(),
        "single-element tuple bound should fire E0089"
    );
    Ok(())
}

#[test]
fn test_e0089_valid_union_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
class MyClass[T: int | str]:
    pass
";
    let diags = run(src)?;
    let e0089: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0089")
        .collect();
    assert!(e0089.is_empty(), "valid union bound should not fire E0089");
    Ok(())
}

#[test]
fn test_e0089_valid_single_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
class MyClass[T: int]:
    pass
";
    let diags = run(src)?;
    let e0089: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0089")
        .collect();
    assert!(e0089.is_empty(), "valid single bound should not fire E0089");
    Ok(())
}

#[test]
fn test_e0089_nested_invalid_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
class MyClass[T: (int, 42)]:
    pass
";
    let diags = run(src)?;
    let e0089: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0089")
        .collect();
    assert!(
        !e0089.is_empty(),
        "invalid constraint element should fire E0089"
    );
    Ok(())
}

#[test]
fn test_e0089_outer_scope_typevar_reference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
class Outer[V: int]:
    class Inner[T: dict[str, V]]:
        pass
";
    let diags = run(src)?;
    let e0089: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0089")
        .collect();
    assert!(
        !e0089.is_empty(),
        "outer scope TypeVar reference should fire E0089"
    );
    Ok(())
}
