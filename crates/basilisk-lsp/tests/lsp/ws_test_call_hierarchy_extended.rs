//! Tests for [LSPARCH-FEATURES-CALLHIER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CALLHIER
// Coverage-boost tests for the call hierarchy surface: exercises
// `callHierarchy/outgoingCalls`, `prepareCallHierarchy` on a class, and the
// no-symbol / no-match edge paths that the basic `ws_test_hierarchies` module
// does not reach. Each test drives MULTIPLE user interactions (prepare →
// incoming → outgoing) and asserts on every response.

use super::ws_test_common::*;

/// Open `code` under a freshly initialized fixture and return the fixture
/// ready for further requests. Drains the initial diagnostics publish.
async fn opened(code: &str, uri: &str) -> TestResult<WsTestFixture> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    fixture.did_open(uri, code).await?;
    let _ = fixture.wait_for_diagnostics().await;
    Ok(fixture)
}

/// `outgoingCalls` for a function that calls other functions AND a class
/// constructor must list every callee, grouped. This covers the
/// `outgoing_calls` / `class_item` / `function_item` paths in
/// `call_hierarchy.rs` that the basic test never reaches.
#[tokio::test]
async fn test_ws_call_hierarchy_outgoing_calls() -> TestResult<()> {
    let code = "\
class Logger:
    def emit(self) -> None:
        pass

def helper(x: int) -> int:
    return x + 1

def main() -> None:
    helper(2)
    Logger()
";
    let mut fixture = opened(code, "file:///ws_call_outgoing.py").await?;

    // Prepare on `main` (line 7, character 4 — inside the name).
    let prepare = fixture
        .request(
            210,
            "textDocument/prepareCallHierarchy",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_call_outgoing.py" },
                "position": { "line": 7, "character": 4 }
            }),
        )
        .await?
        .ok_or("no prepareCallHierarchy response")?;
    assert!(
        prepare.contains("\"name\":\"main\""),
        "prepare should return 'main': {prepare}"
    );
    // `kind: 12` is SymbolKind::Function — assert the prepared item is a function.
    assert!(
        prepare.contains("\"kind\":12"),
        "prepared 'main' should be a function (kind 12): {prepare}"
    );

    // outgoingCalls(main) → should include both `helper` and `Logger`.
    let outgoing = fixture
        .request(
            211,
            "callHierarchy/outgoingCalls",
            serde_json::json!({
                "item": {
                    "name": "main",
                    "kind": 12,
                    "uri": "file:///ws_call_outgoing.py",
                    "range": { "start": { "line": 7, "character": 0 }, "end": { "line": 7, "character": 8 } },
                    "selectionRange": { "start": { "line": 7, "character": 4 }, "end": { "line": 7, "character": 8 } }
                }
            }),
        )
        .await?
        .ok_or("no outgoingCalls response")?;

    assert!(
        outgoing.contains("\"name\":\"helper\""),
        "outgoingCalls(main) should list 'helper': {outgoing}"
    );
    assert!(
        outgoing.contains("\"name\":\"Logger\""),
        "outgoingCalls(main) should list the 'Logger' class constructor: {outgoing}"
    );
    // Logger is a class → kind 5, proving the class_item branch fired.
    assert!(
        outgoing.contains("\"kind\":5"),
        "Logger should be returned as a class (kind 5): {outgoing}"
    );

    // outgoingCalls for a function with NO callees returns an empty (null) list.
    let empty = fixture
        .request(
            212,
            "callHierarchy/outgoingCalls",
            serde_json::json!({
                "item": {
                    "name": "helper",
                    "kind": 12,
                    "uri": "file:///ws_call_outgoing.py",
                    "range": { "start": { "line": 4, "character": 0 }, "end": { "line": 4, "character": 6 } },
                    "selectionRange": { "start": { "line": 4, "character": 4 }, "end": { "line": 4, "character": 10 } }
                }
            }),
        )
        .await?
        .ok_or("no outgoingCalls response for helper")?;
    let parsed: serde_json::Value = serde_json::from_str(&empty)?;
    assert!(
        parsed["result"].is_null() || parsed["result"].as_array().is_some_and(Vec::is_empty),
        "outgoingCalls(helper) should be empty/null: {empty}"
    );

    Ok(())
}

/// `prepareCallHierarchy` on a class returns a class-kind item, and on a
/// blank position returns an empty list — covering the `SymbolHit::Class` and
/// the no-hit branches of `prepare`.
#[tokio::test]
async fn test_ws_call_hierarchy_prepare_class_and_blank() -> TestResult<()> {
    let code = "\
class Greeter:
    def greet(self) -> str:
        return \"hi\"
";
    let mut fixture = opened(code, "file:///ws_call_prepare.py").await?;

    // Cursor on `Greeter` class name (line 0, character 6).
    let class_prepare = fixture
        .request(
            220,
            "textDocument/prepareCallHierarchy",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_call_prepare.py" },
                "position": { "line": 0, "character": 6 }
            }),
        )
        .await?
        .ok_or("no prepareCallHierarchy response for class")?;
    assert!(
        class_prepare.contains("\"name\":\"Greeter\""),
        "prepare should return 'Greeter': {class_prepare}"
    );
    assert!(
        class_prepare.contains("\"kind\":5"),
        "prepare on a class should return kind 5 (Class): {class_prepare}"
    );

    // Cursor on a blank line (line 3, character 0) — no symbol → empty list.
    let blank_prepare = fixture
        .request(
            221,
            "textDocument/prepareCallHierarchy",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_call_prepare.py" },
                "position": { "line": 3, "character": 0 }
            }),
        )
        .await?
        .ok_or("no prepareCallHierarchy response for blank position")?;
    let parsed: serde_json::Value = serde_json::from_str(&blank_prepare)?;
    assert!(
        parsed["result"].is_null()
            || parsed["result"].as_array().is_some_and(Vec::is_empty),
        "prepare on a blank position should yield empty/null: {blank_prepare}"
    );

    Ok(())
}

/// `incomingCalls` for a symbol that nobody calls returns empty, and for a
/// callee that is called from multiple callers groups them correctly.
#[tokio::test]
async fn test_ws_call_hierarchy_incoming_groups_and_empty() -> TestResult<()> {
    let code = "\
def target() -> None:
    pass

def caller_a() -> None:
    target()

def caller_b() -> None:
    target()
";
    let mut fixture = opened(code, "file:///ws_call_incoming.py").await?;

    // incomingCalls(target) → both caller_a and caller_b.
    let incoming = fixture
        .request(
            230,
            "callHierarchy/incomingCalls",
            serde_json::json!({
                "item": {
                    "name": "target",
                    "kind": 12,
                    "uri": "file:///ws_call_incoming.py",
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 6 } },
                    "selectionRange": { "start": { "line": 0, "character": 4 }, "end": { "line": 0, "character": 10 } }
                }
            }),
        )
        .await?
        .ok_or("no incomingCalls response")?;
    assert!(
        incoming.contains("\"name\":\"caller_a\""),
        "incomingCalls(target) should list caller_a: {incoming}"
    );
    assert!(
        incoming.contains("\"name\":\"caller_b\""),
        "incomingCalls(target) should list caller_b: {incoming}"
    );

    // incomingCalls for a never-called function → empty.
    let no_callers = fixture
        .request(
            231,
            "callHierarchy/incomingCalls",
            serde_json::json!({
                "item": {
                    "name": "caller_b",
                    "kind": 12,
                    "uri": "file:///ws_call_incoming.py",
                    "range": { "start": { "line": 6, "character": 0 }, "end": { "line": 6, "character": 8 } },
                    "selectionRange": { "start": { "line": 6, "character": 4 }, "end": { "line": 6, "character": 12 } }
                }
            }),
        )
        .await?
        .ok_or("no incomingCalls response for caller_b")?;
    let parsed: serde_json::Value = serde_json::from_str(&no_callers)?;
    assert!(
        parsed["result"].is_null()
            || parsed["result"].as_array().is_some_and(Vec::is_empty),
        "incomingCalls(caller_b) should be empty/null: {no_callers}"
    );

    Ok(())
}
