//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_typeddict_keys_02`.

use super::common::resolve_src;

#[test]
fn types_match_bare_generic_vs_any() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import assert_type\ndef foo(x: list) -> None:\n    assert_type(x, list[Any])\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn body_last_stmt_terminates_with_raise_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    raise ValueError('bad')\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved
            .functions
            .first()
            .expect("expected at least one function")
            .body_last_stmt_terminates
    );
    Ok(())
}

#[test]
fn typeddict_non_literal_key_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class TD(TypedDict):\n",
        "    name: str\n",
        "key = 'name'\n",
        "td: TD = {key: 'value'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typeddict_key_violations.is_empty());
    Ok(())
}

#[test]
fn inner_tuple_unbounded_nested() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x: tuple[*tuple[str, *tuple[int, ...]], int]\n".to_owned();
    let resolved = resolve_src(&src)?;
    // Just exercise the code path
    assert!(!resolved.module_vars.is_empty());
    Ok(())
}

#[test]
fn is_enum_member_simple_form() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Literal\n",
        "from enum import Enum\n",
        "class Color(Enum):\n",
        "    RED = 1\n",
        "    BLUE = 2\n",
        "def check(c: Literal[Color.RED]) -> None:\n",
        "    result: Literal[\"Color.RED\"] = c\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.literal_string_enum_mismatches.is_empty());
    Ok(())
}

#[test]
fn readonly_kwargs_subscript_assign_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, ReadOnly, Unpack\n",
        "class TD(TypedDict):\n",
        "    name: ReadOnly[str]\n",
        "def foo(**kwargs: Unpack[TD]) -> None:\n",
        "    kwargs[\"name\"] = \"new\"\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.readonly_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_while_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypedDict\nclass TD(TypedDict):\n    name: str\nwhile True:\n    isinstance({}, TD)\n    break\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_detected_in_try_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypedDict\nclass TD(TypedDict):\n    name: str\ntry:\n    isinstance({}, TD)\nexcept Exception:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypedDict\nclass TD(TypedDict):\n    name: str\ndef check() -> None:\n    isinstance({}, TD)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_class_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypedDict\nclass TD(TypedDict):\n    name: str\nclass C:\n    isinstance({}, TD)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn protocol_rtc_in_class_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol\nclass P(Protocol):\n    def m(self) -> None: ...\nclass C:\n    isinstance(42, P)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_runtime_checkable_violations.is_empty());
    Ok(())
}

#[test]
fn protocol_rtc_in_for_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol\nclass P(Protocol):\n    def m(self) -> None: ...\nfor x in [1]:\n    isinstance(x, P)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_runtime_checkable_violations.is_empty());
    Ok(())
}

#[test]
fn file_final_module_reassign_in_function() -> Result<(), Box<dyn std::error::Error>> {
    // Module-level reassignment of Final is only detected when done inside
    // a function with `global X`.
    let src = concat!(
        "from typing import Final\n",
        "X: Final[int] = 42\n",
        "def change() -> None:\n",
        "    global X\n",
        "    X = 99\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.final_violations.is_empty());
    Ok(())
}

#[test]
fn enum_value_init_param_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from enum import Enum\nclass Color(Enum):\n    _value_: int\n    def __init__(self, v: str) -> None:\n        self._value_ = v\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.enum_value_type_violations.is_empty());
    Ok(())
}

#[test]
fn typeddict_subscript_assign_wrong_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "m: Movie = {'name': 'x', 'year': 2000}\n",
        "m['year'] = 'not_int'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typeddict_key_violations.is_empty());
    Ok(())
}

// `check_td_stmts` recurses into function bodies (added with the deep TypedDict
// checking): a TypedDict-typed *parameter* seeds the local var-type map, so
// `del param["required"]` inside the function — and inside a nested function —
// must still be flagged. The module-level del tests do not reach this path.
#[test]
fn typeddict_delete_required_key_inside_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "def outer(m: Movie) -> None:\n",
        "    del m['name']\n",
        "    def inner(n: Movie) -> None:\n",
        "        del n['year']\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "deleting required keys inside (nested) functions must be flagged"
    );
    Ok(())
}

// The required-field guard's negative arm: deleting a `NotRequired` field inside
// a function is allowed (PEP 655), so it must produce no violation even though
// the recursive function-body path runs.
#[test]
fn typeddict_delete_notrequired_key_inside_function_ok() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, NotRequired\n",
        "class Config(TypedDict):\n",
        "    host: str\n",
        "    port: NotRequired[int]\n",
        "def configure(c: Config) -> None:\n",
        "    del c['port']\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved
            .typeddict_key_violations
            .iter()
            .all(|v| v.class_name != "Config"),
        "deleting a NotRequired field must not be flagged"
    );
    Ok(())
}

// Drive every arm of the recursive `check_td_stmts` dispatch from inside a
// function body: an invalid-key read, a wrong-type subscript assignment, a
// disallowed mutator method (`clear`), a required-key delete, and an annotated
// re-binding. The module-level TypedDict tests reach the helpers directly but
// not through the function-body recursion added with the deep checker.
#[test]
fn typeddict_all_operations_inside_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "def process(m: Movie) -> None:\n",
        "    _ok = m['name']\n",
        "    _bad = m['missing']\n",
        "    m['year'] = 'not_int'\n",
        "    m.clear()\n",
        "    del m['name']\n",
        "    n: Movie = m\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.typeddict_key_violations.len() >= 2,
        "function-body TypedDict misuse must surface multiple violations, got {}",
        resolved.typeddict_key_violations.len()
    );
    Ok(())
}
