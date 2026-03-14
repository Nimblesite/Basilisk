//! Tests for LSP: `ws_test_document_highlight`.

#![allow(dead_code, unused_imports)]

mod ws_test_common;
use ws_test_common::*;

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
