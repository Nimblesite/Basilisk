//! Tests for [LSPARCH-FEATURES-INLAYHINTS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-INLAYHINTS
// Tests for LSP: `ws_test_inlay_hints`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_inlay_hints_variable_types() -> TestResult<()> {
    let uri = "file:///inlay_vars.py";
    let code = "x = 42\ny = \"hello\"\nz = True\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        100,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 3, "character": 0 }
            }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let hints = parsed["result"]
        .as_array()
        .ok_or("result should be an array")?;

    assert_eq!(hints.len(), 3, "expected exactly 3 hints: {resp}");

    for hint in hints {
        let pos = &hint["position"];
        assert!(
            pos["line"].as_u64().is_some(),
            "hint position must have a line: {hint}"
        );
        assert!(
            pos["character"].as_u64().is_some(),
            "hint position must have a character: {hint}"
        );

        let label = hint["label"].as_str().unwrap_or("");
        assert!(!label.is_empty(), "hint label must not be empty: {hint}");

        assert_eq!(
            hint["kind"].as_u64(),
            Some(1),
            "hint kind must be 1 (Type): {hint}"
        );
    }

    let all_labels: String = hints
        .iter()
        .filter_map(|h| h["label"].as_str())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        all_labels.contains("int"),
        "should contain int hint: {all_labels}"
    );
    assert!(
        all_labels.contains("LiteralString"),
        "should contain LiteralString hint: {all_labels}"
    );
    assert!(
        all_labels.contains("bool"),
        "should contain bool hint: {all_labels}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hints_fully_annotated_returns_empty() -> TestResult<()> {
    let uri = "file:///inlay_annotated.py";
    let code = "x: int = 42\ny: str = \"hello\"\nz: bool = True\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        101,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 3, "character": 0 }
            }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    assert!(
        result.is_null() || result.as_array().is_some_and(Vec::is_empty),
        "fully annotated code should return null or empty hints: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_return_type_inferred() -> TestResult<()> {
    let uri = "file:///inlay_return.py";
    let code = "def add(a: int, b: int):\n    return 42\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        102,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 2, "character": 0 }
            }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let hints = parsed["result"]
        .as_array()
        .ok_or("result should be an array")?;

    let has_return_hint = hints
        .iter()
        .filter_map(|h| h["label"].as_str())
        .any(|label| label.contains("-> int"));

    assert!(
        has_return_hint,
        "should have a return type hint containing '-> int': {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_return_type_not_shown_when_annotated() -> TestResult<()> {
    let uri = "file:///inlay_return_annotated.py";
    let code = "def greet(name: str) -> str:\n    return f'Hello, {name}!'\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        103,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 2, "character": 0 }
            }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    if let Some(hints) = result.as_array() {
        let has_return_hint = hints
            .iter()
            .filter_map(|h| h["label"].as_str())
            .any(|label| label.starts_with(" -> "));

        assert!(
            !has_return_hint,
            "annotated return type should not produce a ' -> ' hint: {resp}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_parameter_names_at_call_site() -> TestResult<()> {
    let uri = "file:///inlay_param_names.py";
    let code = "def greet(name: str, greeting: str) -> str:\n    return f'{greeting}, {name}!'\n\nresult = greet(\"world\", \"Hi\")\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        104,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 4, "character": 0 }
            }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let hints = parsed["result"]
        .as_array()
        .ok_or("result should be an array")?;

    let all_labels: String = hints
        .iter()
        .filter_map(|h| h["label"].as_str())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        all_labels.contains("name="),
        "should contain 'name=' parameter hint: {all_labels}"
    );
    assert!(
        all_labels.contains("greeting="),
        "should contain 'greeting=' parameter hint: {all_labels}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_no_hints_for_annotated_vars() -> TestResult<()> {
    let uri = "file:///inlay_mixed.py";
    let code = "x: int = 42\ny = \"hello\"\nz: bool = True\nw = 3.14\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        105,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 4, "character": 0 }
            }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let hints = parsed["result"]
        .as_array()
        .ok_or("result should be an array")?;

    let all_labels: String = hints
        .iter()
        .filter_map(|h| h["label"].as_str())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        all_labels.contains("LiteralString"),
        "should contain 'LiteralString' hint for unannotated y: {all_labels}"
    );
    assert!(
        all_labels.contains("float"),
        "should contain 'float' hint for unannotated w: {all_labels}"
    );
    assert!(
        !all_labels.contains(": int"),
        "should not contain ': int' hint for annotated x: {all_labels}"
    );
    assert!(
        !all_labels.contains(": bool"),
        "should not contain ': bool' hint for annotated z: {all_labels}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_function_local_unannotated_vars() -> TestResult<()> {
    // Regression for #68: un-annotated *function-local* variables must produce
    // inferred `: <type>` hints, just like module-level ones do.  Previously the
    // local-variable branch was dead code because `FunctionInfo::local_vars` only
    // held annotated locals, so every entry was skipped.
    let uri = "file:///inlay_local_vars.py";
    let code = "def f():\n    n = 0\n    s = \"hi\"\n    return n\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        108,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 4, "character": 0 }
            }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let hints = parsed["result"]
        .as_array()
        .ok_or("result should be an array")?;

    let all_labels: String = hints
        .iter()
        .filter_map(|h| h["label"].as_str())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        all_labels.contains(": int"),
        "local `n = 0` should produce a ': int' hint: {all_labels}"
    );
    assert!(
        all_labels.contains(": LiteralString"),
        "local `s = \"hi\"` should produce a ': LiteralString' hint: {all_labels}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_return_type_multiple_returns() -> TestResult<()> {
    let uri = "file:///inlay_multi_return.py";
    let code = "def choose(flag: bool):\n    if flag:\n        return 1\n    return 2\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        106,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 4, "character": 0 }
            }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let hints = parsed["result"]
        .as_array()
        .ok_or("result should be an array")?;

    let has_return_hint = hints
        .iter()
        .filter_map(|h| h["label"].as_str())
        .any(|label| label.contains("-> int"));

    assert!(
        has_return_hint,
        "function with multiple returns should have '-> int' hint: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_method_return_type() -> TestResult<()> {
    let uri = "file:///inlay_method.py";
    let code = "class Calculator:\n    def add(self, a: int, b: int):\n        return 42\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        107,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 3, "character": 0 }
            }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let hints = parsed["result"]
        .as_array()
        .ok_or("result should be an array")?;

    let has_return_hint = hints
        .iter()
        .filter_map(|h| h["label"].as_str())
        .any(|label| label.contains("-> int"));

    assert!(
        has_return_hint,
        "method should have '-> int' return type hint: {resp}"
    );

    Ok(())
}
