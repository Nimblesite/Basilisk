//! Tests for [BSK-W0050] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// E2E tests for BSK-W0050: Redundant annotation warning

use super::common::*;

#[test]
fn test_w0050_int_literal_redundant() {
    let diags = run("x: int = 42\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_str_literal_redundant() {
    let diags = run("x: str = \"hello\"\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_float_literal_redundant() {
    let diags = run("x: float = 3.14\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_bool_literal_redundant() {
    let diags = run("x: bool = True\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_bytes_literal_redundant() {
    let diags = run("x: bytes = b\"data\"\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_none_literal_redundant() {
    let diags = run("x: None = None\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_int_widening_no_warning() {
    let diags = run("x: float = 42\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_call_expression_no_warning() {
    let diags = run("x: int = some_function()\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_unknown_inference_no_warning() {
    let diags = run("x: int = unknown_var\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_function_param_exempt() {
    let diags = run("def f(x: int): pass\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_return_annotation_exempt() {
    let diags = run("def f() -> int: return 42\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_class_attribute_redundant() {
    let diags = run("class C:\n    x: int = 42\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_module_level_redundant() {
    let diags = run("count: int = 0\nlabel: str = \"test\"\nratio: float = 1.5\n").unwrap();
    let w0050_count = diags.iter().filter(|d| d.code.code == "BSK-W0050").count();
    assert_eq!(w0050_count, 3);
}

#[test]
fn test_w0050_list_literal_no_warning() {
    let diags = run("items: list[int] = [1, 2, 3]\n").unwrap();
    // Collection types rarely match exactly due to inference differences
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_dict_literal_no_warning() {
    let diags = run("pairs: dict[str, int] = {\"a\": 1, \"b\": 2}\n").unwrap();
    // Collection types rarely match exactly due to inference differences
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_set_literal_no_warning() {
    let diags = run("numbers: set[int] = {1, 2, 3}\n").unwrap();
    // Collection types rarely match exactly due to inference differences
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_tuple_literal_no_warning() {
    let diags = run("coords: tuple[int, int] = (1, 2)\n").unwrap();
    // Collection types rarely match exactly due to inference differences
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}
