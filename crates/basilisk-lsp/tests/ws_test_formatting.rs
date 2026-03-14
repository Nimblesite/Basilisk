//! Tests for LSP: `ws_test_formatting`.

#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

// ── Document Formatting ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_format_document() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Badly formatted Python: inconsistent spacing, missing trailing newline.
    let code =
        "x:int=1\ny:str=\"hello\"\ndef   greet( name:str )->str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ws_format.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            600,
            "textDocument/formatting",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_format.py" },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        )
        .await?
        .ok_or("no formatting response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    // If ruff is available, we should get text edits back.
    // If ruff is not installed, result may be null — that's acceptable.
    if !result.is_null() {
        let edits = result
            .as_array()
            .ok_or("formatting result should be an array of TextEdits")?;
        assert!(
            !edits.is_empty(),
            "formatting should produce at least one TextEdit for badly formatted code: {resp}"
        );

        // Verify the edit has a range and newText.
        let first_edit = &edits[0];
        assert!(
            first_edit.get("range").is_some(),
            "TextEdit should have a range: {resp}"
        );
        assert!(
            first_edit.get("newText").is_some(),
            "TextEdit should have newText: {resp}"
        );

        // The formatted text should differ from the original.
        let new_text = first_edit["newText"]
            .as_str()
            .ok_or("newText should be a string")?;
        assert_ne!(new_text, code, "formatted text should differ from original");
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_format_document_already_formatted() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Well-formatted Python code (PEP 8 compliant, trailing newline).
    let code = "x: int = 1\ny: str = \"hello\"\n\n\ndef greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///ws_format_clean.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            710,
            "textDocument/formatting",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_format_clean.py" },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        )
        .await?
        .ok_or("no formatting response for already-formatted code")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    // For already-formatted code, result should be null (no changes)
    // or an empty array of edits — both are valid LSP responses.
    if !result.is_null() {
        let edits = result
            .as_array()
            .ok_or("formatting result should be null or an array")?;
        // If edits are returned, verify the new text is the same as original
        // (ruff may return a whole-file replacement that is identical).
        if !edits.is_empty() {
            let new_text = edits[0]["newText"].as_str().unwrap_or("");
            // The resulting text should be equivalent to the input.
            assert!(
                new_text == code || edits.is_empty(),
                "already-formatted code should produce no meaningful changes: {resp}"
            );
        }
    }
    // result == null is also fine — means no edits needed.

    Ok(())
}

#[tokio::test]
async fn test_ws_format_document_empty_file() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Empty file — formatting should not crash.
    let code = "";
    fixture.did_open("file:///ws_format_empty.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            711,
            "textDocument/formatting",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_format_empty.py" },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        )
        .await?
        .ok_or("no formatting response for empty file")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;

    // Response must have a result field — it can be null or an empty array.
    assert!(
        parsed.get("result").is_some(),
        "formatting empty file should return a valid result: {resp}"
    );

    let result = &parsed["result"];
    if !result.is_null() {
        let edits = result
            .as_array()
            .ok_or("formatting result for empty file should be null or an array")?;
        // Empty file should not produce meaningful edits.
        // If there are edits, the newText should be empty or whitespace-only.
        for edit in edits {
            let new_text = edit["newText"].as_str().unwrap_or("");
            assert!(
                new_text.trim().is_empty(),
                "empty file formatting should not produce non-empty content: {resp}"
            );
        }
    }

    Ok(())
}
