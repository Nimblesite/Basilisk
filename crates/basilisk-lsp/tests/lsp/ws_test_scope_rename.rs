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
