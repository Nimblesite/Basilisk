//! E2E tests for collection type inference.
//!
//! All tests are coarse E2E tests that run real .py fixtures through the full pipeline.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(src: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

/// Test empty list inference: `[]` → E0003 fires (cannot infer type without annotation).
///
/// An unannotated empty list cannot have its element type inferred, so E0003 must fire.
/// To suppress the diagnostic, use an explicit annotation: `x: list[int] = []`.
#[test]
fn test_empty_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = []\n")?;
    assert_eq!(diagnostics.len(), 1, "Unannotated empty list must produce E0003");
    assert_eq!(diagnostics[0].code.code, "BSK-E0003");
    Ok(())
}

/// Test homogeneous list inference: `[1, 2, 3]` → `list[int]`
#[test]
fn test_homogeneous_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = [1, 2, 3]\n")?;
    assert_eq!(diagnostics.len(), 0, "Homogeneous list should not produce diagnostics");
    Ok(())
}

/// Test heterogeneous list inference: `[1, "hi"]` → `list[int | str]`
#[test]
fn test_heterogeneous_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = [1, \"hi\"]\n")?;
    assert_eq!(diagnostics.len(), 0, "Heterogeneous list should not produce diagnostics");
    Ok(())
}

/// Test empty dict inference: `{}` → E0003 fires (cannot infer type without annotation).
///
/// An unannotated empty dict cannot have its key/value types inferred, so E0003 must fire.
/// To suppress the diagnostic, use an explicit annotation: `x: dict[str, int] = {}`.
#[test]
fn test_empty_dict_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = {}\n")?;
    assert_eq!(diagnostics.len(), 1, "Unannotated empty dict must produce E0003");
    assert_eq!(diagnostics[0].code.code, "BSK-E0003");
    Ok(())
}

/// Test homogeneous dict inference: `{"a": 1, "b": 2}` → `dict[str, int]`
#[test]
fn test_homogeneous_dict_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = {\"a\": 1, \"b\": 2}\n")?;
    assert_eq!(diagnostics.len(), 0, "Homogeneous dict should not produce diagnostics");
    Ok(())
}

/// Test heterogeneous dict inference: `{1: "a", "b": 2}` → `dict[int | str, str | int]`
#[test]
fn test_heterogeneous_dict_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = {1: \"a\", \"b\": 2}\n")?;
    assert_eq!(diagnostics.len(), 0, "Heterogeneous dict should not produce diagnostics");
    Ok(())
}

/// Test set inference: `{1, 2, 3}` → `set[int]`
#[test]
fn test_set_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = {1, 2, 3}\n")?;
    assert_eq!(diagnostics.len(), 0, "Homogeneous set should not produce diagnostics");
    Ok(())
}

/// Test tuple inference: `(1, "hi", 3.0)` → `tuple[int, str, float]`
#[test]
fn test_tuple_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = (1, \"hi\", 3.0)\n")?;
    assert_eq!(diagnostics.len(), 0, "Tuple should not produce diagnostics");
    Ok(())
}

/// Test mixed type list: `[1, "hi", 3.0]` → `list[int | str | float]`
#[test]
fn test_mixed_type_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = [1, \"hi\", 3.0]\n")?;
    assert_eq!(diagnostics.len(), 0, "Mixed type list should not produce diagnostics");
    Ok(())
}

/// Test nested list: `[[1, 2], ["a", "b"]]` → `list[list[int | str]]`
#[test]
fn test_nested_list_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = [[1, 2], [\"a\", \"b\"]]\n")?;
    assert_eq!(diagnostics.len(), 0, "Nested list should not produce diagnostics");
    Ok(())
}

/// Test dict with mixed key types: `{1: "a", "b": 2}` → `dict[int | str, str | int]`
#[test]
fn test_mixed_key_dict_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = {1: \"a\", \"b\": 2}\n")?;
    assert_eq!(diagnostics.len(), 0, "Dict with mixed key types should not produce diagnostics");
    Ok(())
}

/// Test list with None: `[1, None, 3]` → `list[int | None]`
#[test]
fn test_list_with_none_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = [1, None, 3]\n")?;
    assert_eq!(diagnostics.len(), 0, "List with None should not produce diagnostics");
    Ok(())
}

/// Test flow union in if-else: variable assigned in both branches
#[test]
fn test_flow_union_if_else_inference() -> Result<(), Box<dyn std::error::Error>> {
    let source = "cond = True\nif cond:\n    x = 1\nelse:\n    x = \"hi\"\n";
    let diagnostics = run(source)?;
    assert_eq!(diagnostics.len(), 0, "Flow union should not produce diagnostics");
    Ok(())
}

/// Test walrus operator inference: `(n := len(a))` → `n` is `int`
#[test]
fn test_walrus_operator_inference() -> Result<(), Box<dyn std::error::Error>> {
    let source = "a = [1, 2]\nif (n := len(a)) > 10:\n    pass\n";
    let diagnostics = run(source)?;
    assert_eq!(diagnostics.len(), 0, "Walrus operator should not produce diagnostics");
    Ok(())
}

/// Test augmented assignment: `x = 1; x += 2` → `x` remains `int`
#[test]
fn test_augmented_assign_inference() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("x = 1\nx += 2\n")?;
    assert_eq!(diagnostics.len(), 0, "Augmented assignment should not produce diagnostics");
    Ok(())
}

/// Test literal inference at module scope: `STATUS = "active"` — no diagnostics
#[test]
fn test_literal_inference_module_scope() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run("STATUS = \"active\"\n")?;
    assert_eq!(diagnostics.len(), 0, "Literal at module scope should not produce diagnostics");
    Ok(())
}

/// Test literal inference in function scope: widened to base type
#[test]
fn test_literal_inference_function_scope() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f() -> None:\n    x = \"active\"\n";
    let diagnostics = run(source)?;
    assert_eq!(diagnostics.len(), 0, "Literal in function scope should be widened");
    Ok(())
}

/// Test isinstance narrowing: `if isinstance(x, int):` — no diagnostics
#[test]
fn test_isinstance_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def get_value() -> int:\n    return 1\nx = get_value()\nif isinstance(x, int):\n    pass\n";
    let diagnostics = run(source)?;
    assert_eq!(diagnostics.len(), 0, "isinstance narrowing should not produce diagnostics");
    Ok(())
}

/// Test is None / is not None narrowing
#[test]
fn test_is_none_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def get_value() -> int:\n    return 1\nx = get_value()\nif x is None:\n    pass\n";
    let diagnostics = run(source)?;
    assert_eq!(diagnostics.len(), 0, "is None narrowing should not produce diagnostics");
    Ok(())
}

/// Test assignment narrowing
#[test]
fn test_assignment_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def get_value() -> int:\n    return 1\nx = get_value()\nx = 42\n";
    let diagnostics = run(source)?;
    assert_eq!(diagnostics.len(), 0, "Assignment narrowing should not produce diagnostics");
    Ok(())
}
