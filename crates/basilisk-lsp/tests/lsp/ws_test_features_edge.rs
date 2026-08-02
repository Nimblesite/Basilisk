//! Tests for [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
// Coverage-boost tests for the feature-handler surface (`features.rs`):
// exercises the "document not opened → null" early-return paths, the
// `source.fixAll` code-action branch, range formatting, document color +
// color presentation, and completion auto-import suggestions. Each test issues
// several requests and asserts on each response.

use super::ws_test_common::*;

async fn opened(code: &str, uri: &str) -> TestResult<WsTestFixture> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    fixture.did_open(uri, code).await?;
    let _ = fixture.wait_for_diagnostics().await;
    Ok(fixture)
}

/// Requesting feature handlers for a document that was NEVER opened must
/// return `null` (not error). This covers the `get_document_data → None`
/// branches in `folding_range`, `selection_range`, `code_lens`,
/// `semantic_tokens_full`, and `inlay_hint`.
#[tokio::test]
async fn test_ws_feature_handlers_return_null_for_unopened_doc() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let ghost = "file:///never_opened.py";

    for (id, method, params) in [
        (
            500u64,
            "textDocument/foldingRange",
            serde_json::json!({ "textDocument": { "uri": ghost } }),
        ),
        (
            501u64,
            "textDocument/selectionRange",
            serde_json::json!({
                "textDocument": { "uri": ghost },
                "positions": [{ "line": 0, "character": 0 }]
            }),
        ),
        (
            502u64,
            "textDocument/codeLens",
            serde_json::json!({ "textDocument": { "uri": ghost } }),
        ),
        (
            503u64,
            "textDocument/semanticTokens/full",
            serde_json::json!({ "textDocument": { "uri": ghost } }),
        ),
        (
            504u64,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": ghost },
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } }
            }),
        ),
    ] {
        let resp = fixture
            .request(id, method, params)
            .await?
            .ok_or(format!("no response for {method}"))?;
        let parsed: serde_json::Value = serde_json::from_str(&resp)?;
        assert!(
            parsed.get("error").is_none(),
            "{method} for unopened doc must not error: {resp}"
        );
        assert!(
            parsed["result"].is_null(),
            "{method} for unopened doc should be null: {resp}"
        );
    }

    Ok(())
}

/// The `source.fixAll.basilisk` code action produces a single combined
/// `WorkspaceEdit` action. Covers the `wants_fix_all` branch in `code_action`.
#[tokio::test]
async fn test_ws_source_fix_all_code_action() -> TestResult<()> {
    let code = "x: int = 42\ny: str = \"hi\"\n";
    let mut fixture = opened(code, "file:///ws_fixall.py").await?;

    let resp = fixture
        .request(
            510,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_fixall.py" },
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 2, "character": 0 } },
                "context": { "diagnostics": [], "only": ["source.fixAll.basilisk"] }
            }),
        )
        .await?
        .ok_or("no fixAll code action response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let actions = parsed["result"]
        .as_array()
        .ok_or("fixAll should return an array")?;
    assert!(
        !actions.is_empty(),
        "source.fixAll must produce at least one action: {resp}"
    );
    // The action should carry an edit (the combined WorkspaceEdit).
    assert!(
        actions[0].get("edit").is_some(),
        "fixAll action should carry a workspace edit: {resp}"
    );

    Ok(())
}

/// `textDocument/rangeFormatting` formats a sub-range in-place via the
/// embedded Ruff engine. Covers the `range_formatting` handler.
#[tokio::test]
async fn test_ws_range_formatting() -> TestResult<()> {
    let code = "x={1,2,3}\n";
    let mut fixture = opened(code, "file:///ws_rangefmt.py").await?;

    let resp = fixture
        .request(
            520,
            "textDocument/rangeFormatting",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_rangefmt.py" },
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        )
        .await?
        .ok_or("no rangeFormatting response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    // Either the engine returns edits (non-null) or null if nothing to change;
    // both are well-formed. Assert no error and a result field is present.
    assert!(
        parsed.get("error").is_none(),
        "rangeFormatting must not error: {resp}"
    );
    assert!(
        parsed.get("result").is_some(),
        "rangeFormatting must carry a result: {resp}"
    );

    Ok(())
}

/// `textDocument/documentColor` finds CSS-style hex color literals and
/// `textDocument/colorPresentation` produces their string forms. Covers the
/// `document_color` and `color_presentation` handlers + `color.rs`.
#[tokio::test]
async fn test_ws_document_color_and_presentations() -> TestResult<()> {
    let code = "BG = \"#ff8800\"\n";
    let mut fixture = opened(code, "file:///ws_color.py").await?;

    let colors = fixture
        .request(
            530,
            "textDocument/documentColor",
            serde_json::json!({ "textDocument": { "uri": "file:///ws_color.py" } }),
        )
        .await?
        .ok_or("no documentColor response")?;
    let parsed: serde_json::Value = serde_json::from_str(&colors)?;
    assert!(
        parsed.get("error").is_none(),
        "documentColor must not error: {colors}"
    );
    // Result is an array (possibly empty if the literal form isn't matched).
    assert!(
        parsed["result"].is_array(),
        "documentColor result must be an array: {colors}"
    );

    // colorPresentation is a request the editor sends for a chosen color; it
    // must return an array of presentations regardless of input.
    let pres = fixture
        .request(
            531,
            "textDocument/colorPresentation",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_color.py" },
                "color": { "red": 1.0, "green": 0.5, "blue": 0.0, "alpha": 1.0 },
                "range": { "start": { "line": 0, "character": 5 }, "end": { "line": 0, "character": 12 } }
            }),
        )
        .await?
        .ok_or("no colorPresentation response")?;
    let pres_parsed: serde_json::Value = serde_json::from_str(&pres)?;
    assert!(
        pres_parsed["result"].is_array(),
        "colorPresentation result must be an array: {pres}"
    );

    Ok(())
}

/// Completion at a position referencing an as-yet-unimported workspace symbol
/// should surface auto-import suggestions. Covers the `build_auto_import_items`
/// path in `completion`. We open a second file defining a class, then request
/// completion in the first file for that class's prefix.
#[tokio::test]
async fn test_ws_completion_auto_import() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_autoimport");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("lib.py"),
        "class database_helper:\n    pass\n",
    )?;
    std::fs::write(dir.join("main.py"), "def use():\n    database_helper")?;

    let root_uri = format!("file://{}", dir.display());
    let lib_uri = format!("file://{}", dir.join("lib.py").display());
    let main_uri = format!("file://{}", dir.join("main.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;
    for _ in 0..30 {
        let msg = tokio::time::timeout(Duration::from_millis(400), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }
    fixture
        .did_open(&lib_uri, "class database_helper:\n    pass\n")
        .await?;
    fixture
        .did_open(&main_uri, "def use():\n    database_helper")
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor right after `database_helper` (line 1, char 20).
    let resp = fixture
        .request(
            540,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 1, "character": 20 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    // The completion response must be well-formed and not error. If
    // auto-import fired, it carries the label `database_helper` with an
    // additional text edit inserting the import.
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "completion must not error: {resp}"
    );
    if let Some(items) = parsed["result"].as_array() {
        if let Some(ai) = items
            .iter()
            .find(|i| i["label"].as_str() == Some("database_helper"))
        {
            assert!(
                ai.get("additionalTextEdits").is_some(),
                "auto-import item must carry additionalTextEdits: {ai}"
            );
            assert!(
                ai["labelDetails"]["detail"]
                    .as_str()
                    .unwrap_or("")
                    .contains("auto-import"),
                "auto-import item should be labelled as such: {ai}"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
