//! Tests for [TYPEINF-COLLECTIONS]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-COLLECTIONS
// End-to-end tests for Basilisk's collection type inference.

// ---------------------------------------------------------------------------
// Collection Inference E2E Tests
// ---------------------------------------------------------------------------

use super::common::*;

#[test]
fn test_empty_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = []
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "empty list should be clean");
    Ok(())
}

#[test]
fn test_empty_dict_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = {}
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "empty dict should be clean");
    Ok(())
}

#[test]
fn test_homogeneous_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = [1, 2, 3]
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "homogeneous list should be clean");
    Ok(())
}

#[test]
fn test_heterogeneous_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = [1, 'hello']
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "heterogeneous list should be clean");
    Ok(())
}

#[test]
fn test_mixed_type_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = [1, 2.0, 'hello']
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "mixed type list should be clean");
    Ok(())
}

#[test]
fn test_list_with_none_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = [1, None, 'hello']
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "list with None should be clean");
    Ok(())
}

#[test]
fn test_nested_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = [[1, 2], [3, 4]]
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "nested list should be clean");
    Ok(())
}

#[test]
fn test_homogeneous_dict_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = {'a': 1, 'b': 2}
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "homogeneous dict should be clean");
    Ok(())
}

#[test]
fn test_heterogeneous_dict_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = {'a': 1, 'b': 'hello'}
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "heterogeneous dict should be clean");
    Ok(())
}

#[test]
fn test_mixed_key_dict_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = {1: 'a', 'b': 2}
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "mixed key dict should be clean");
    Ok(())
}

#[test]
fn test_set_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = {1, 2, 3}
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "set should be clean");
    Ok(())
}

#[test]
fn test_tuple_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = (1, 'hello', 3.0)
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "tuple should be clean");
    Ok(())
}

#[test]
fn test_assignment_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x: int | str = get_value()
    x = 42
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "assignment narrowing should be clean");
    Ok(())
}

#[test]
fn test_isinstance_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f(x: int | str) -> None:
    if isinstance(x, int):
        reveal_type(x)
    else:
        reveal_type(x)
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "isinstance narrowing should be clean");
    Ok(())
}

#[test]
fn test_is_none_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f(x: int | None) -> None:
    if x is None:
        reveal_type(x)
    else:
        reveal_type(x)
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "is None narrowing should be clean");
    Ok(())
}

#[test]
fn test_flow_union_if_else_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f(cond: bool) -> None:
    if cond:
        x = 1
    else:
        x = 'hi'
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "flow union if-else should be clean");
    Ok(())
}

#[test]
fn test_augmented_assign_inference() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
def f() -> None:
    x = 1
    x += 2
";
    let diags = run(src)?;
    assert!(diags.is_empty(), "augmented assignment should be clean");
    Ok(())
}
