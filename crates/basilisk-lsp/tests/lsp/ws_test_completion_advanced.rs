//! Tests for [LSPARCH-FEATURES-COMPLETION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-COMPLETION
// Tests for LSP: `ws_test_completion_advanced`.

// WebSocket LSP E2E tests — Keyword argument completions, kind values, docstrings.

use super::ws_test_common::*;

// ── Keyword Argument Completions ────────────────────────────────────────────

#[tokio::test]
async fn test_ws_completion_kwarg_suggests_param_names() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
def greet(name: str, greeting: str) -> str:
    return f\"{greeting}, {name}!\"

result: str = greet()
";
    fixture.did_open("file:///ws_kwarg_comp.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor inside greet() — line 3, character 20 (after the opening paren)
    let resp = fixture
        .request(
            520,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_kwarg_comp.py" },
                "position": { "line": 3, "character": 20 }
            }),
        )
        .await?
        .ok_or("no completion response for kwarg")?;

    assert!(
        resp.contains("\"label\":\"name=\""),
        "should suggest 'name=' kwarg completion: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"greeting=\""),
        "should suggest 'greeting=' kwarg completion: {resp}"
    );
    // Kind should be KEYWORD (14 in LSP spec)
    assert!(
        resp.contains("\"kind\":14"),
        "kwarg completions should have kind KEYWORD (14): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_kwarg_skips_already_provided() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
def greet(name: str, greeting: str) -> str:
    return f\"{greeting}, {name}!\"

result: str = greet(name=\"world\", )
";
    fixture.did_open("file:///ws_kwarg_skip.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor after "name=\"world\", " — line 3, character 33
    let resp = fixture
        .request(
            521,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_kwarg_skip.py" },
                "position": { "line": 3, "character": 33 }
            }),
        )
        .await?
        .ok_or("no completion response for kwarg skip")?;

    // 'name=' was already provided, so only 'greeting=' should appear.
    assert!(
        !resp.contains("\"label\":\"name=\""),
        "should NOT suggest already-provided 'name=' kwarg: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"greeting=\""),
        "should suggest remaining 'greeting=' kwarg: {resp}"
    );
    Ok(())
}

// ── Completion Kind Values ──────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_completion_kind_values() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
class Widget:
    size: int

def render(w: Widget) -> str:
    return \"ok\"

count: int = 0
";
    fixture.did_open("file:///ws_comp_kinds.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            955,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_comp_kinds.py" },
                "position": { "line": 7, "character": 0 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let items = parsed["result"]
        .as_array()
        .or_else(|| parsed["result"]["items"].as_array())
        .ok_or("completion result should have items")?;

    // Find the Widget class completion — kind 7 (Class).
    let widget = items.iter().find(|i| i["label"].as_str() == Some("Widget"));
    assert!(
        widget.is_some(),
        "should have Widget in completions: {resp}"
    );
    assert_eq!(
        widget.map(|w| w["kind"].as_u64()),
        Some(Some(7)),
        "Widget should have kind CLASS (7): {resp}"
    );

    // Find the render function completion — kind 3 (Function).
    let render = items.iter().find(|i| i["label"].as_str() == Some("render"));
    assert!(
        render.is_some(),
        "should have render in completions: {resp}"
    );
    assert_eq!(
        render.map(|r| r["kind"].as_u64()),
        Some(Some(3)),
        "render should have kind FUNCTION (3): {resp}"
    );

    // Find the count variable completion — kind 6 (Variable).
    let count = items.iter().find(|i| i["label"].as_str() == Some("count"));
    assert!(count.is_some(), "should have count in completions: {resp}");
    assert_eq!(
        count.map(|c| c["kind"].as_u64()),
        Some(Some(6)),
        "count should have kind VARIABLE (6): {resp}"
    );
    Ok(())
}

// ── Completion Docstring / Resolve ──────────────────────────────────────────

#[tokio::test]
async fn test_ws_completion_includes_docstring() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
def helper(x: int) -> int:
    \"\"\"Return x plus one.\"\"\"
    return x + 1

hel
";
    fixture.did_open("file:///compdoc.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            312,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///compdoc.py" },
                "position": { "line": 4, "character": 3 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("helper"),
        "completions should include 'helper': {resp}"
    );
    // Docstrings are now lazy-loaded via completionItem/resolve, so the initial
    // completion list includes `data` for resolve but not inline documentation.
    assert!(
        resp.contains("\"data\""),
        "completion should include resolve data: {resp}"
    );
    Ok(())
}
