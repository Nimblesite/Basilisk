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
    assert!(
        parsed.get("error").is_none(),
        "rangeFormatting must not error: {resp}"
    );
    // `x={1,2,3}` is NOT already formatted, so the engine must return an edit.
    // Assert the exact replacement text — the embedded engine is a byte-exact
    // passthrough of `ruff format` ([LSPFMT-HONESTY]), and `ruff format 0.16.1`
    // turns this line into `x = {1, 2, 3}`. Asserting only "no error" would
    // pass even if the formatter silently stopped emitting edits.
    let edits = parsed["result"]
        .as_array()
        .ok_or_else(|| format!("rangeFormatting must return TextEdits, not null: {resp}"))?;
    let new_text = edits
        .first()
        .and_then(|edit| edit["newText"].as_str())
        .ok_or("first TextEdit must carry newText")?;
    assert_eq!(
        new_text, "x = {1, 2, 3}\n",
        "range formatting must match `ruff format` byte-for-byte: {resp}"
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
    // `document_color` returns a plain Vec, so `is_array()` is trivially true
    // even when the scanner finds nothing. Assert the COLOR instead: in
    // `BG = "#ff8800"` the `#` is byte 6, so the swatch spans characters 6..13
    // (`#` + six hex digits) and decodes to rgba(1.0, 0x88/255, 0.0, 1.0).
    let found = parsed["result"]
        .as_array()
        .ok_or_else(|| format!("documentColor must return an array: {colors}"))?;
    assert_eq!(
        found.len(),
        1,
        "exactly one hex literal in `BG = \"#ff8800\"`: {colors}"
    );
    let swatch = found.first().ok_or("documentColor returned no entry")?;
    let approx = |actual: Option<f64>, want: f64| -> bool {
        actual.is_some_and(|value| (value - want).abs() < 0.001_f64)
    };
    assert!(
        approx(swatch["color"]["red"].as_f64(), 1.0)
            && approx(swatch["color"]["green"].as_f64(), f64::from(0x88) / 255.0)
            && approx(swatch["color"]["blue"].as_f64(), 0.0)
            && approx(swatch["color"]["alpha"].as_f64(), 1.0),
        "#ff8800 must decode to rgba(1, 0x88/255, 0, 1): {swatch}"
    );
    assert_eq!(
        swatch["range"]["start"]["character"].as_u64(),
        Some(6),
        "swatch must start at the `#`: {swatch}"
    );
    assert_eq!(
        swatch["range"]["end"]["character"].as_u64(),
        Some(13),
        "swatch must end after the last hex digit: {swatch}"
    );

    // colorPresentation renders a chosen color back to source text. An opaque
    // color yields BOTH the 6- and 8-digit forms; 0.5 * 255 truncates to 0x7f.
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
    let presentations = pres_parsed["result"]
        .as_array()
        .ok_or_else(|| format!("colorPresentation must return an array: {pres}"))?;
    let labels: Vec<&str> = presentations
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect();
    assert_eq!(
        labels,
        vec!["#ff7f00", "#ff7f00ff"],
        "an opaque color must offer the 6- and 8-digit forms: {pres}"
    );
    assert_eq!(
        presentations
            .first()
            .and_then(|item| item["textEdit"]["newText"].as_str()),
        Some("#ff7f00"),
        "each presentation must carry the edit that writes it back: {pres}"
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
    std::fs::write(dir.join("lib.py"), "class database_helper:\n    pass\n")?;
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

    // Cursor at end-of-line, right after `database_helper`. Line 1 is
    // `    database_helper` = 4 spaces + 15 chars, so the last valid column is
    // 19. `extract_completion_prefix` walks back from the cursor, and
    // `suggest_imports` does an EXACT-name lookup — at any other column the
    // prefix is not `database_helper` and auto-import cannot fire.
    let resp = fixture
        .request(
            540,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 1, "character": 19 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "completion must not error: {resp}"
    );
    // Assert UNCONDITIONALLY. Guarding these behind `if let Some(item) = …find()`
    // makes the test pass when auto-import silently stops firing — which is the
    // only regression it exists to catch.
    let items = parsed["result"]
        .as_array()
        .ok_or_else(|| format!("completion must return items, not null: {resp}"))?;
    let suggestion = items
        .iter()
        .find(|item| item["label"].as_str() == Some("database_helper"))
        .ok_or_else(|| {
            format!("completion must offer an auto-import for `database_helper`: {resp}")
        })?;
    assert!(
        suggestion["labelDetails"]["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("auto-import from"),
        "the item must be labelled as an auto-import: {suggestion}"
    );
    // The whole point of the feature: accepting the item also inserts the
    // import statement. `generate_import_text` emits `from <mod> import <name>`.
    let import_edit = suggestion["additionalTextEdits"]
        .as_array()
        .and_then(|edits| edits.first())
        .ok_or_else(|| format!("auto-import must carry additionalTextEdits: {suggestion}"))?;
    assert_eq!(
        import_edit["newText"].as_str(),
        Some("from lib import database_helper\n"),
        "auto-import must insert the import for the defining module: {suggestion}"
    );
    assert_eq!(
        import_edit["range"]["start"]["line"].as_u64(),
        Some(0),
        "`main.py` has no imports, so the statement goes at the top: {suggestion}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
