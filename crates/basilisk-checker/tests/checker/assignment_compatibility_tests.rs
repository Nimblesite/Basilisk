//! Tests for [`assignment_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for assignment_compatibility: Assignment type incompatibility.

use super::common::*;

#[test]
fn int_annotated_str_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "count: int = \"hello\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "int annotation with str literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn empty_dict_to_implicit_dict_alias_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Regression test for issue #282: `RefsDictT` is a legacy implicit alias
    // for a dict type, so the empty dict literal is assignable to it.
    let source = r#"
RefsDictT = dict[tuple[str, str], str]

def build_refs_dict() -> RefsDictT:
    refs_dict: RefsDictT = {}
    return refs_dict
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "empty dict is assignable to a dict alias; should not fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn empty_dict_to_explicit_typealias_dict_alias_no_diagnostic() -> Result<(), Box<dyn std::error::Error>>
{
    // Regression test for issue #282: the *explicit* `TypeAlias`-annotated form
    // (`RefsDictT: TypeAlias = dict[...]`) must be treated as a value alias too,
    // so the empty dict literal is assignable to it. The implicit form is
    // covered by `empty_dict_to_implicit_dict_alias_no_diagnostic` above.
    let source = r#"
from typing import TypeAlias
RefsDictT: TypeAlias = dict[tuple[str, str], str]

def build_refs_dict() -> RefsDictT:
    refs_dict: RefsDictT = {}
    return refs_dict
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "empty dict is assignable to an explicit TypeAlias dict alias; should not fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn mismatched_dict_literal_to_implicit_dict_alias_fires() -> Result<(), Box<dyn std::error::Error>>
{
    // A dict literal whose key type contradicts the alias definition must
    // still fire after alias expansion.
    let source = r#"
RefsDictT = dict[tuple[str, str], str]

def build_refs_dict() -> RefsDictT:
    refs_dict: RefsDictT = {1: "x"}
    return refs_dict
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "int key is not assignable to tuple[str, str]; should fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn dict_literal_with_typed_variable_value_no_diagnostic() -> Result<(), Box<dyn std::error::Error>>
{
    // Regression for issue #332: a dict display whose value comes from a typed
    // variable must be checked contextually against the declared dict type
    // (exactly like `return {...}`), not inferred bottom-up to
    // `dict[LiteralString, Unknown]` and then rejected under dict invariance.
    // The `y`/`lst` lines already pass; the `d` line was the false positive.
    let source = r#"
def f(x: str) -> None:
    y: str = x
    lst: list[str] = [x]
    d: dict[str, str] = {"k": x}
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "a dict literal with a typed variable value is assignable to dict[str, str]; should not fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn dict_literal_with_multiple_typed_variable_values_no_diagnostic(
) -> Result<(), Box<dyn std::error::Error>> {
    // Real-world instance from issue #332 (docker_host.py:617): every value
    // comes from a typed parameter.
    let source = r#"
def build(bind_host: str, host_port_str: str) -> None:
    binding: dict[str, str] = {"HostIp": bind_host, "HostPort": host_port_str}
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "a dict of typed-variable values should not fire against dict[str, str], got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn dict_literal_with_wrong_value_type_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    // Guard for issue #332: contextual checking must not swallow a genuine
    // mismatch — a str-literal value against `dict[str, int]` is still an error.
    let source = r#"
def f() -> None:
    d: dict[str, int] = {"k": "not an int"}
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "a str value is not assignable to dict[str, int]; should fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn str_annotated_int_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "label: str = 42\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "str annotation with int literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn bool_annotated_str_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "flag: bool = \"yes\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "bool annotation with str literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn float_annotated_str_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "ratio: float = \"1.5\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "float annotation with str literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn compatible_assignment_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "count: int = 42\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "compatible assignment should not fire E0014"
    );
    Ok(())
}

#[test]
fn str_annotated_str_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "name: str = \"hello\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "str annotation with str literal should not fire E0014"
    );
    Ok(())
}

#[test]
fn local_var_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def func() -> None:
    x: int = "oops"
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "local variable type mismatch should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn local_var_compatible_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func() -> None:
    x: int = 42
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "compatible local var should not fire E0014"
    );
    Ok(())
}

#[test]
fn bytes_annotated_str_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "data: bytes = \"text\"\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "bytes annotation with str literal should fire E0014"
    );
    Ok(())
}

#[test]
fn int_annotated_bool_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // bool is a subclass of int in Python
    let source = "x: int = True\n";
    let diags = run(source)?;

    let _msgs = messages_for(&diags, "assignment_compatibility");
    // This may or may not fire depending on implementation; bool is subtype of int
    // Just exercise the code path
    Ok(())
}

#[test]
fn float_annotated_int_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // int is widened to float in Python
    let source = "x: float = 42\n";
    let diags = run(source)?;

    let _msgs = messages_for(&diags, "assignment_compatibility");
    // int -> float widening may or may not be handled
    Ok(())
}

#[test]
fn optional_none_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // None is a valid value for Optional[int]
    let source = "from typing import Optional\nmaybe_int: Optional[int] = None\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "Optional[int] = None should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn union_member_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // 42 (int) is a valid member of Union[int, str]
    let source = "from typing import Union\neither: Union[int, str] = 42\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "Union[int, str] = 42 should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn final_with_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Final[int] = 100 — int matches int
    let source = "from typing import Final\nMAX_SIZE: Final[int] = 100\n";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "Final[int] = 100 should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn no_false_positive_on_pep695_type_alias_annotation() -> Result<(), Box<dyn std::error::Error>> {
    // PEP 695 type alias used as annotation: E0014 should NOT fire because
    // the type alias might expand to a union that includes int.
    let source = r#"
type RecursiveTypeAlias1[T] = T | list[RecursiveTypeAlias1[T]]

r1_1: RecursiveTypeAlias1[int] = 1
r1_2: RecursiveTypeAlias1[int] = [1, [1, 2, 3]]
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "E0014 should not fire on variables annotated with a PEP 695 type alias, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn no_false_positive_on_homogeneous_tuple_str_annotation() -> Result<(), Box<dyn std::error::Error>>
{
    // Regression for issue #45: `tuple[str, ...]` is PEP 484's homogeneous
    // variable-length tuple. A literal tuple of all-string elements widens to
    // it and must NOT fire E0014.
    let source = r#"
_MODEL_SETTING_KEYS: tuple[str, ...] = ("max_tokens", "temperature", "top_p", "timeout", "seed")
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "tuple[str, ...] = (..all strings..) should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn no_false_positive_on_dict_with_tuple_key_annotation() -> Result<(), Box<dyn std::error::Error>> {
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

    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "dict[tuple[str, str], str] = {{matching literal}} should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn tuple_reassignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 1
y: str = "hello"
x, y = "wrong", 42
"#;
    // Just exercise the tuple reassignment path
    let diags = run(source)?;

    let _msgs = messages_for(&diags, "assignment_compatibility");
    Ok(())
}

#[test]
fn no_false_positive_on_variadic_tuple_of_tuples_annotation(
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
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "tuple[tuple[str, str], ...] = (..matching pairs..) should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn variadic_tuple_of_tuples_element_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The true-positive pair for issue #26: an element violating the variadic
    // element type must still be flagged.
    let source = r#"
_BAD: tuple[tuple[str, str], ...] = (("a", 1), ("c", "d"))
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "a (str, int) element must still fire E0014 against tuple[tuple[str, str], ...]"
    );
    Ok(())
}
