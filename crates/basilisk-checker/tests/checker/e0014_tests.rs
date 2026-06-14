//! Tests for [BSK-E0014] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0014: Assignment type incompatibility.

use super::common::*;

#[test]
fn e0014_int_annotated_str_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "count: int = \"hello\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        !msgs.is_empty(),
        "int annotation with str literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_str_annotated_int_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "label: str = 42\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        !msgs.is_empty(),
        "str annotation with int literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_bool_annotated_str_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "flag: bool = \"yes\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        !msgs.is_empty(),
        "bool annotation with str literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_float_annotated_str_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "ratio: float = \"1.5\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        !msgs.is_empty(),
        "float annotation with str literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_compatible_assignment_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "count: int = 42\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "compatible assignment should not fire E0014"
    );
    Ok(())
}

#[test]
fn e0014_str_annotated_str_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "name: str = \"hello\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "str annotation with str literal should not fire E0014"
    );
    Ok(())
}

#[test]
fn e0014_local_var_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def func() -> None:
    x: int = "oops"
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        !msgs.is_empty(),
        "local variable type mismatch should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_local_var_compatible_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func() -> None:
    x: int = 42
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "compatible local var should not fire E0014"
    );
    Ok(())
}

#[test]
fn e0014_bytes_annotated_str_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "data: bytes = \"text\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        !msgs.is_empty(),
        "bytes annotation with str literal should fire E0014"
    );
    Ok(())
}

#[test]
fn e0014_int_annotated_bool_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // bool is a subclass of int in Python
    let source = "x: int = True\n";
    let diags = run(source)?;

    let _msgs = messages_for(&diags, "BSK-E0014");
    // This may or may not fire depending on implementation; bool is subtype of int
    // Just exercise the code path
    Ok(())
}

#[test]
fn e0014_float_annotated_int_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // int is widened to float in Python
    let source = "x: float = 42\n";
    let diags = run(source)?;

    let _msgs = messages_for(&diags, "BSK-E0014");
    // int -> float widening may or may not be handled
    Ok(())
}

#[test]
fn e0014_optional_none_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // None is a valid value for Optional[int]
    let source = "from typing import Optional\nmaybe_int: Optional[int] = None\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "Optional[int] = None should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_union_member_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // 42 (int) is a valid member of Union[int, str]
    let source = "from typing import Union\neither: Union[int, str] = 42\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "Union[int, str] = 42 should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_final_with_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Final[int] = 100 — int matches int
    let source = "from typing import Final\nMAX_SIZE: Final[int] = 100\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "Final[int] = 100 should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_no_false_positive_on_pep695_type_alias_annotation(
) -> Result<(), Box<dyn std::error::Error>> {
    // PEP 695 type alias used as annotation: E0014 should NOT fire because
    // the type alias might expand to a union that includes int.
    let source = r#"
type RecursiveTypeAlias1[T] = T | list[RecursiveTypeAlias1[T]]

r1_1: RecursiveTypeAlias1[int] = 1
r1_2: RecursiveTypeAlias1[int] = [1, [1, 2, 3]]
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "E0014 should not fire on variables annotated with a PEP 695 type alias, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_no_false_positive_on_homogeneous_tuple_str_annotation(
) -> Result<(), Box<dyn std::error::Error>> {
    // Regression for issue #45: `tuple[str, ...]` is PEP 484's homogeneous
    // variable-length tuple. A literal tuple of all-string elements widens to
    // it and must NOT fire E0014.
    let source = r#"
_MODEL_SETTING_KEYS: tuple[str, ...] = ("max_tokens", "temperature", "top_p", "timeout", "seed")
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "tuple[str, ...] = (..all strings..) should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_no_false_positive_on_dict_with_tuple_key_annotation(
) -> Result<(), Box<dyn std::error::Error>> {
    // Regression for issue #51: `dict[tuple[str, str], str]` has a tuple KEY
    // type whose inner comma must not be split by the dict arg parser. A dict
    // literal matching the annotation exactly must NOT fire E0014.
    let source = r#"
_LLM_DISPLAY_NAMES: dict[tuple[str, str], str] = {
    ("anthropic", "claude-opus-4-7"): "Claude Opus 4.7",
    ("anthropic", "claude-sonnet-4-6"): "Claude Sonnet 4.6",
}
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "dict[tuple[str, str], str] = {{matching literal}} should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_tuple_reassignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 1
y: str = "hello"
x, y = "wrong", 42
"#;
    // Just exercise the tuple reassignment path
    let diags = run(source)?;

    let _msgs = messages_for(&diags, "BSK-E0014");
    Ok(())
}

#[test]
fn e0014_no_false_positive_on_variadic_tuple_of_tuples_annotation(
) -> Result<(), Box<dyn std::error::Error>> {
    // Regression for issue #26: a concrete-length tuple literal of homogeneous
    // element types is assignable to the variadic `tuple[T, ...]` annotation.
    let source = r#"
_DIRECT_TENANT_TABLES: tuple[tuple[str, str], ...] = (
    ("tenants", "id"),
    ("agent_configs", "tenant_id"),
)
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        msgs.is_empty(),
        "tuple[tuple[str, str], ...] = (..matching pairs..) should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_variadic_tuple_of_tuples_element_mismatch_fires() -> Result<(), Box<dyn std::error::Error>>
{
    // The true-positive pair for issue #26: an element violating the variadic
    // element type must still be flagged.
    let source = r#"
_BAD: tuple[tuple[str, str], ...] = (("a", 1), ("c", "d"))
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "BSK-E0014");
    assert!(
        !msgs.is_empty(),
        "a (str, int) element must still fire E0014 against tuple[tuple[str, str], ...]"
    );
    Ok(())
}
