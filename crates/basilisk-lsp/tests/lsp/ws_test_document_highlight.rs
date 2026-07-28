//! Tests for [LSPARCH-FEATURES-HIGHLIGHT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HIGHLIGHT
// Tests for LSP: `ws_test_document_highlight`.

use super::ws_test_common::*;

// ── Document Highlight ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_document_highlight() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
def greet(name: str) -> str:
    return name
greet(\"hi\")
";
    fixture.did_open("file:///ws_highlight.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Request documentHighlight at the position of `greet` on line 0 (character 4).
    let resp = fixture
        .request(
            630,
            "textDocument/documentHighlight",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_highlight.py" },
                "position": { "line": 0, "character": 4 }
            }),
        )
        .await?
        .ok_or("no documentHighlight response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let highlights = parsed["result"]
        .as_array()
        .ok_or("documentHighlight result should be an array")?;

    // Should find at least 2 highlights: definition of greet + call of greet.
    assert!(
        highlights.len() >= 2,
        "should find at least 2 highlights for 'greet' (found {}): {resp}",
        highlights.len()
    );

    // Each highlight should have a range and a kind.
    for hl in highlights {
        assert!(
            hl.get("range").is_some(),
            "highlight should have a range: {hl}"
        );
        assert!(
            hl.get("kind").is_some(),
            "highlight should have a kind: {hl}"
        );
    }

    Ok(())
}

/// Document highlight must not mark occurrences of the name inside string
/// literals — they are data, not references.
#[tokio::test]
async fn test_ws_document_highlight_skips_string_content() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
def greet(name: str) -> str:
    return name
note: str = \"greet me\"
greet(\"hi\")
";
    fixture
        .did_open("file:///ws_highlight_strings.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            631,
            "textDocument/documentHighlight",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_highlight_strings.py" },
                "position": { "line": 0, "character": 4 }
            }),
        )
        .await?
        .ok_or("no documentHighlight response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let highlights = parsed["result"]
        .as_array()
        .ok_or("documentHighlight result should be an array")?;

    // Definition (line 0) + call (line 3); the string on line 2 must not be lit.
    for hl in highlights {
        let line = hl["range"]["start"]["line"].as_u64().unwrap_or(99);
        assert!(
            line == 0 || line == 3,
            "highlight must not mark string content on line {line}: {resp}"
        );
    }
    assert_eq!(
        highlights.len(),
        2,
        "expected exactly 2 highlights (definition + call): {resp}"
    );

    Ok(())
}
