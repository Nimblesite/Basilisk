//! Tests for [BSK-W0050] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// E2E tests for BSK-W0050: Redundant annotation warning

use super::common::*;

#[test]
fn test_w0050_int_literal_redundant() {
    let diags = run_with_optin_rules("x: int = 42\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_str_literal_redundant() {
    let diags = run_with_optin_rules("x: str = \"hello\"\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_float_literal_redundant() {
    let diags = run_with_optin_rules("x: float = 3.14\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_bool_literal_redundant() {
    let diags = run_with_optin_rules("x: bool = True\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_bytes_literal_redundant() {
    let diags = run_with_optin_rules("x: bytes = b\"data\"\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_none_literal_redundant() {
    let diags = run_with_optin_rules("x: None = None\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_int_widening_no_warning() {
    let diags = run_with_optin_rules("x: float = 42\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_call_expression_no_warning() {
    let diags = run_with_optin_rules("x: int = some_function()\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_unknown_inference_no_warning() {
    let diags = run_with_optin_rules("x: int = unknown_var\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_function_param_exempt() {
    let diags = run_with_optin_rules("def f(x: int): pass\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_return_annotation_exempt() {
    let diags = run_with_optin_rules("def f() -> int: return 42\n").unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_class_attribute_redundant() {
    let diags = run_with_optin_rules("class C:\n    x: int = 42\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_module_level_redundant() {
    let diags = run_with_optin_rules("count: int = 0\nlabel: str = \"test\"\nratio: float = 1.5\n").unwrap();
    let w0050_count = diags.iter().filter(|d| d.code.code == "BSK-W0050").count();
    assert_eq!(w0050_count, 3);
}

#[test]
fn test_w0050_list_literal_no_warning() {
    let diags = run_with_optin_rules("items: list[int] = [1, 2, 3]\n").unwrap();
    // Collection types rarely match exactly due to inference differences
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_dict_literal_no_warning() {
    let diags = run_with_optin_rules("pairs: dict[str, int] = {\"a\": 1, \"b\": 2}\n").unwrap();
    // Collection types rarely match exactly due to inference differences
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_set_literal_no_warning() {
    let diags = run_with_optin_rules("numbers: set[int] = {1, 2, 3}\n").unwrap();
    // Collection types rarely match exactly due to inference differences
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_tuple_literal_no_warning() {
    let diags = run_with_optin_rules("coords: tuple[int, int] = (1, 2)\n").unwrap();
    // Collection types rarely match exactly due to inference differences
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

// Issue #110: the annotation on a Pydantic/dataclass/attrs field is load-bearing —
// removing it deletes the field. W0050 must never fire there.

#[test]
fn test_w0050_pydantic_basemodel_field_not_flagged() {
    let diags = run_with_optin_rules(concat!(
        "from pydantic import BaseModel\n",
        "class ToolResultIn(BaseModel):\n",
        "    ok: bool = True\n",
    ))
    .unwrap();
    assert!(
        !diags.iter().any(|d| d.code.code == "BSK-W0050"),
        "W0050 on a BaseModel field deletes the field when autofixed"
    );
}

#[test]
fn test_w0050_pydantic_basemodel_transitive_subclass_field_not_flagged() {
    let diags = run_with_optin_rules(concat!(
        "from pydantic import BaseModel\n",
        "class Base(BaseModel):\n",
        "    name: str = \"x\"\n",
        "class Child(Base):\n",
        "    count: int = 0\n",
    ))
    .unwrap();
    assert!(
        !diags.iter().any(|d| d.code.code == "BSK-W0050"),
        "W0050 on a transitive BaseModel subclass field deletes the field when autofixed"
    );
}

#[test]
fn test_w0050_dataclass_field_not_flagged() {
    let diags = run_with_optin_rules(concat!(
        "from dataclasses import dataclass\n",
        "@dataclass(frozen=True, kw_only=True)\n",
        "class GitOutcome:\n",
        "    committed: bool = False\n",
    ))
    .unwrap();
    assert!(
        !diags.iter().any(|d| d.code.code == "BSK-W0050"),
        "W0050 on a dataclass field turns an init param into an inert class attribute"
    );
}

#[test]
fn test_w0050_dataclasses_dotted_decorator_field_not_flagged() {
    let diags = run_with_optin_rules(concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass\n",
        "class Manifest:\n",
        "    method: str = \"POST\"\n",
    ))
    .unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_attrs_define_field_not_flagged() {
    let diags = run_with_optin_rules(concat!(
        "from attrs import define\n",
        "@define\n",
        "class AuthContext:\n",
        "    is_platform_admin: bool = False\n",
    ))
    .unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_attr_s_decorator_field_not_flagged() {
    let diags = run_with_optin_rules(concat!(
        "import attr\n",
        "@attr.s(auto_attribs=True)\n",
        "class AgentConfig:\n",
        "    retries: int = 3\n",
    ))
    .unwrap();
    assert!(!diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_plain_class_attribute_still_flagged() {
    // Regression guard: the exemption must not swallow plain classes.
    let diags = run_with_optin_rules("class Plain:\n    x: int = 42\n").unwrap();
    assert!(diags.iter().any(|d| d.code.code == "BSK-W0050"));
}

#[test]
fn test_w0050_pydantic_dataclasses_dotted_decorator_field_not_flagged() {
    // Regression for issue #39: `@pydantic.dataclasses.dataclass` fields need
    // their annotation to be fields at all — never redundant.
    let diags = run_with_optin_rules(concat!(
        "import pydantic\n",
        "@pydantic.dataclasses.dataclass\n",
        "class ChatTurnInput:\n",
        "    tool_results_already_persisted: bool = False\n",
    ))
    .unwrap();
    assert!(
        !diags.iter().any(|d| d.code.code == "BSK-W0050"),
        "W0050 must not fire on pydantic dataclass fields (issue #39)"
    );
}
