//! Tests for [LSPARCH-FEATURES-RENAME]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-RENAME
// Tests for scope-aware rename.
//
// These tests verify that rename operations respect Python's lexical scoping
// rules: shadowed names in inner scopes are NOT renamed, and names in the
// correct scope ARE renamed.

use super::ws_test_common::*;

/// Renaming a local variable inside a function must NOT rename the same name
/// at module level.
#[tokio::test]
async fn test_ws_rename_respects_function_scope() -> TestResult<()> {
    let uri = "file:///scope_rename_func.py";
    // `x` at module level and `x` inside `foo` are different bindings.
    let code = "x: int = 1\n\ndef foo() -> int:\n    x: int = 2\n    return x\n\ny: int = x\n";

    // Rename `x` inside the function (line 3, char 4 = the `x` in `x: int = 2`).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        500,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 3, "character": 4 },
            "newName": "local_x"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Should rename the local `x` occurrences (the assignment + the return),
    // but NOT the module-level `x` on line 0 or line 6.
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            (3..=4).contains(&line),
            "edit should only touch lines 3-4 (function body), but found edit on line {line}: {edit}"
        );
        assert_eq!(edit["newText"].as_str(), Some("local_x"));
    }

    Ok(())
}

/// Renaming a module-level variable must NOT rename a shadowed local variable
/// with the same name inside a function.
#[tokio::test]
async fn test_ws_rename_module_scope_skips_shadowed() -> TestResult<()> {
    let uri = "file:///scope_rename_module.py";
    let code = "x: int = 1\n\ndef foo() -> int:\n    x: int = 2\n    return x\n\ny: int = x\n";

    // Rename `x` at module level (line 0, char 0).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        501,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
            "newName": "global_x"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Should rename module-level `x` (line 0) and `y: int = x` (line 6),
    // but NOT the `x` inside `foo` (lines 3-4).
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 0 || line == 6,
            "edit should only touch lines 0 and 6, but found edit on line {line}: {edit}"
        );
        assert_eq!(edit["newText"].as_str(), Some("global_x"));
    }

    Ok(())
}

/// Renaming a function parameter must only affect usages within that function.
#[tokio::test]
async fn test_ws_rename_parameter_scope() -> TestResult<()> {
    let uri = "file:///scope_rename_param.py";
    let code = "name: str = \"global\"\n\ndef greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = name\n";

    // Rename `name` parameter (line 2, char 10 = the `n` in `greet(name: str)`).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        502,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": 10 },
            "newName": "person"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Should only rename within the function (lines 2-3), not at module level.
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            (2..=3).contains(&line),
            "edit should only touch lines 2-3, but found edit on line {line}: {edit}"
        );
    }

    Ok(())
}

/// Renaming a function name should rename at the definition and all call sites,
/// but not a local variable with the same name.
#[tokio::test]
async fn test_ws_rename_function_name_not_local_shadow() -> TestResult<()> {
    let uri = "file:///scope_rename_func_name.py";
    let code = "def compute(x: int) -> int:\n    return x * 2\n\ncompute: int = 42\n";

    // Rename the function `compute` (line 0, char 4).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        503,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 4 },
            "newName": "calculate"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    Ok(())
}

/// Renaming a variable to a Python keyword must be rejected.
#[tokio::test]
async fn test_ws_rename_rejects_keyword() -> TestResult<()> {
    let uri = "file:///scope_rename_keyword.py";
    let code = "x: int = 1\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        504,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
            "newName": "class"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    // Should return null result (rename rejected).
    assert!(
        parsed["result"].is_null(),
        "rename to keyword 'class' should return null: {resp}"
    );

    Ok(())
}

/// Renaming a variable to an invalid identifier (starts with digit) must be rejected.
#[tokio::test]
async fn test_ws_rename_rejects_invalid_identifier() -> TestResult<()> {
    let uri = "file:///scope_rename_invalid.py";
    let code = "x: int = 1\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        505,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
            "newName": "123abc"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "rename to '123abc' should return null: {resp}"
    );

    Ok(())
}

/// Renaming a symbol must never rewrite matching text inside string literals
/// or docstring prose — those are data, not references.
///
/// Regression test for `is_in_string_or_comment` (references.rs) only
/// implementing the `#`-comment half of its contract: occurrences of the
/// name inside plain strings and docstrings were emitted as rename edits,
/// silently corrupting user data.
#[tokio::test]
async fn test_ws_rename_skips_string_literals_and_docstring_prose() -> TestResult<()> {
    let uri = "file:///scope_rename_strings.py";
    // `total` appears as: the parameter (line 0), prose inside the docstring
    // (line 1), text inside a string literal (line 2), and the real usage
    // (line 3). Only lines 0 and 3 are genuine references.
    let code = "def process(total: int) -> int:\n    \"\"\"Compute the total for a report.\"\"\"\n    label: str = \"total is big\"\n    return total\n";

    // Rename the `total` parameter (line 0, char 12 = the `t` in `total`).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        507,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 12 },
            "newName": "amount"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // The docstring (line 1) and the string literal (line 2) must be untouched.
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 0 || line == 3,
            "rename must not edit string content on line {line}: {edit}"
        );
    }
    // Exactly the definition + the real usage, nothing more.
    assert_eq!(
        edits.len(),
        2,
        "expected exactly 2 edits (definition + usage): {resp}"
    );

    Ok(())
}

/// Renaming a parameter used in an f-string interpolation field must rename
/// the field: `{name}` is code, not string data. Companion pin for the
/// string-literal mask — only *literal* f-string chunks are masked.
#[tokio::test]
async fn test_ws_rename_includes_fstring_interpolation_field() -> TestResult<()> {
    let uri = "file:///scope_rename_fstring.py";
    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";

    // Rename the `name` parameter (line 0, char 10).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        508,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 10 },
            "newName": "person"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // The interpolation field on line 1 MUST be renamed. `{` sits at char 20,
    // so the `name` identifier spans chars 21-25.
    let interpolation_edit = edits.iter().find(|edit| {
        edit["range"]["start"]["line"].as_u64() == Some(1)
            && edit["range"]["start"]["character"].as_u64() == Some(21)
    });
    assert!(
        interpolation_edit.is_some(),
        "f-string interpolation field must be renamed: {resp}"
    );
    // Definition + interpolation field, nothing else (the literal chunks
    // `Hello, ` and `!` contain no match, and no other usage exists).
    assert_eq!(edits.len(), 2, "expected exactly 2 edits: {resp}");

    Ok(())
}

/// Renaming a class referenced through PEP 563 string annotations must rename
/// the name inside the annotation strings — they are forward references, not
/// data. Companion pin for the string-literal mask's annotation exemption.
#[tokio::test]
async fn test_ws_rename_includes_string_annotation_references() -> TestResult<()> {
    let uri = "file:///scope_rename_str_ann.py";
    let code =
        "class MyClass:\n    pass\n\ndef make(x: \"MyClass\") -> \"MyClass\":\n    return x\n";

    // Rename `MyClass` at its definition (line 0, char 6).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        509,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 6 },
            "newName": "Renamed"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Both string-annotation occurrences on line 3 must be renamed: the
    // parameter annotation (char 13) and the return annotation (char 27).
    for expected_char in [13_u64, 27_u64] {
        let annotation_edit = edits.iter().find(|edit| {
            edit["range"]["start"]["line"].as_u64() == Some(3)
                && edit["range"]["start"]["character"].as_u64() == Some(expected_char)
        });
        assert!(
            annotation_edit.is_some(),
            "string annotation at line 3 char {expected_char} must be renamed: {resp}"
        );
    }
    // Definition + the two annotation strings.
    assert_eq!(edits.len(), 3, "expected exactly 3 edits: {resp}");

    Ok(())
}

/// Nested function scoping: renaming `x` in the outer function should not
/// affect `x` in the inner function when the inner function redefines it.
#[tokio::test]
async fn test_ws_rename_nested_function_shadow() -> TestResult<()> {
    let uri = "file:///scope_rename_nested.py";
    let code = "def outer() -> int:\n    x: int = 1\n    def inner() -> int:\n        x: int = 2\n        return x\n    return x\n";

    // Rename `x` in outer (line 1, char 4).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        506,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 1, "character": 4 },
            "newName": "outer_x"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Should rename `x` on lines 1 and 5 (outer scope), but NOT lines 3-4 (inner).
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 1 || line == 5,
            "edit should only touch lines 1 and 5 (outer function), but found edit on line {line}: {edit}"
        );
        assert_eq!(edit["newText"].as_str(), Some("outer_x"));
    }

    Ok(())
}
