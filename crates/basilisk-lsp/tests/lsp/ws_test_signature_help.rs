//! Tests for [LSPARCH-FEATURES-SIGHELP]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-SIGHELP
// Tests for LSP: `ws_test_signature_help`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_signature_help() -> TestResult<()> {
    let code = "\
def greet(name: str, greeting: str) -> str:
    return f\"{greeting}, {name}!\"

result: str = greet(\"world\", \"Hi\")
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_sighelp.py",
        code,
        330,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_sighelp.py" },
            "position": { "line": 3, "character": 21 }
        }),
    )
    .await?;

    assert!(
        resp.contains("greet"),
        "signature should show function name: {resp}"
    );
    assert!(
        resp.contains("name"),
        "signature should show parameter 'name': {resp}"
    );
    assert!(
        resp.contains("greeting"),
        "signature should show parameter 'greeting': {resp}"
    );

    // Hardened: parse and verify signature help structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    assert!(
        !result.is_null(),
        "signature help result must not be null: {resp}"
    );

    // Hardened: verify activeParameter is 0 (cursor at first param position)
    let active_param = result["activeParameter"].as_u64();
    assert_eq!(
        active_param,
        Some(0),
        "activeParameter should be 0 (first parameter): {resp}"
    );

    // Hardened: verify signatures array exists and is non-empty
    let signatures = result["signatures"]
        .as_array()
        .ok_or("signature help should have signatures array")?;
    assert!(
        !signatures.is_empty(),
        "signatures array must be non-empty: {resp}"
    );

    // Hardened: verify parameters array length matches expected count (2: name, greeting)
    let first_sig = &signatures[0];
    let parameters = first_sig["parameters"]
        .as_array()
        .ok_or("first signature should have parameters array")?;
    assert_eq!(
        parameters.len(),
        2,
        "should have exactly 2 parameters (name, greeting), got {}: {resp}",
        parameters.len()
    );

    // Hardened: verify each parameter has a label
    for param in parameters {
        assert!(
            param.get("label").is_some() && !param["label"].is_null(),
            "each parameter must have a label: {resp}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_signature_help_outside_call_returns_null() -> TestResult<()> {
    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nx: int = 42\n";
    let (_fixture, resp) = open_and_request(
        "file:///ws_edge_sighelp.py",
        code,
        403,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_edge_sighelp.py" },
            "position": { "line": 3, "character": 0 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "signature help outside a call should return null: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_signature_help_active_parameter_index() -> TestResult<()> {
    let code = "\
def add(a: int, b: int) -> int:
    return a + b

result: int = add(1, 2)
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_edge_sighelp_idx.py",
        code,
        408,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_edge_sighelp_idx.py" },
            "position": { "line": 3, "character": 21 }
        }),
    )
    .await?;

    assert!(
        resp.contains("activeParameter"),
        "signature help should include activeParameter: {resp}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let active_param = &parsed["result"]["activeParameter"];
    assert!(
        !active_param.is_null(),
        "activeParameter should not be null: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_signature_help_method_skips_self() -> TestResult<()> {
    let code = "\
class Greeter:
    prefix: str
    def greet(self, name: str, loud: bool) -> str:
        return f\"{self.prefix} {name}\"

g: Greeter = Greeter()
result: str = g.greet(\"world\", True)
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_sighelp_self.py",
        code,
        1102,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_sighelp_self.py" },
            "position": { "line": 6, "character": 23 }
        }),
    )
    .await?;

    // Should show greet signature with name and loud but NOT self.
    assert!(
        resp.contains("name"),
        "signature should show parameter 'name': {resp}"
    );
    assert!(
        resp.contains("loud"),
        "signature should show parameter 'loud': {resp}"
    );

    // Parse and verify self is not in the parameter list.
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let signatures = parsed["result"]["signatures"]
        .as_array()
        .ok_or("expected signatures array")?;
    if let Some(sig) = signatures.first() {
        let params = sig["parameters"]
            .as_array()
            .ok_or("expected parameters array")?;
        let param_labels: Vec<&str> = params.iter().filter_map(|p| p["label"].as_str()).collect();
        assert!(
            !param_labels.contains(&"self"),
            "signature help should NOT include 'self' as a parameter: {param_labels:?}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_signature_help_class_constructor() -> TestResult<()> {
    let code = "\
class Point:
    x: int
    y: int
    def __init__(self, x: int, y: int) -> None:
        self.x = x
        self.y = y

p: Point = Point(1, 2)
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_sighelp_ctor.py",
        code,
        1103,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_sighelp_ctor.py" },
            "position": { "line": 7, "character": 18 }
        }),
    )
    .await?;

    // Should show __init__ signature parameters (x and y, not self).
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    assert!(
        !result.is_null(),
        "signature help for constructor should not be null: {resp}"
    );

    // If we get a valid signature, verify it shows the parameters.
    if let Some(signatures) = result["signatures"].as_array() {
        if let Some(sig) = signatures.first() {
            let label = sig["label"].as_str().unwrap_or("");
            assert!(
                label.contains('x') && label.contains('y'),
                "constructor signature should show parameters x and y: {label}"
            );
            // self should not appear in the label.
            let params = sig["parameters"].as_array();
            if let Some(params) = params {
                let param_labels: Vec<&str> =
                    params.iter().filter_map(|p| p["label"].as_str()).collect();
                assert!(
                    !param_labels.contains(&"self"),
                    "constructor signature should NOT include 'self': {param_labels:?}"
                );
            }
        }
    }

    Ok(())
}
