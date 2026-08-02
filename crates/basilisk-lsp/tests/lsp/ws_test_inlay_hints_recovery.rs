//! Tests for [ANALYSIS-INDEX-LASTGOOD]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INDEX-LASTGOOD
//! Also covers [LSPARCH-FEATURES-INLAYHINTS], the surface that reads through it.
// Tests for LSP: `ws_test_inlay_hints_recovery` — a transient parse error must
// not blank a file's inlay hints. Typing `.` to reach an attribute makes the
// buffer stop parsing for one keystroke; hints on unrelated lines above are
// still correct and must keep rendering. GitHub #386.
//
// Exercises `WorkspaceIndex::{store_entry, get_for_display}` in
// `crates/basilisk-lsp/src/workspace.rs` through the real LSP message loop.

use super::ws_test_common::*;

/// Request full-file inlay hints and return the type hint labels (`": <type>"`).
async fn type_hints_after_change(
    fixture: &mut WsTestFixture,
    uri: &str,
    version: i32,
    text: &str,
    request_id: u64,
) -> TestResult<Vec<String>> {
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }
        }))
        .await?;
    let _ = fixture.wait_for_diagnostics().await?;

    let resp = fixture
        .request(
            request_id,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 40, "character": 0 }
                }
            }),
        )
        .await?
        .ok_or("no response to textDocument/inlayHint")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    Ok(parsed["result"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|hint| hint["label"].as_str())
        .filter(|label| label.starts_with(": "))
        .map(str::to_owned)
        .collect())
}

/// GitHub #386: appending a trailing `.` — the buffer state during every
/// attribute access — dropped EVERY hint in the file, including hints on lines
/// far above the edit whose types had not changed.
#[tokio::test]
async fn test_ws_inlay_hints_survive_transient_parse_error() -> TestResult<()> {
    let uri = "file:///inlay_recovery.py";
    let parses = "x = \"abc\"\ny = [1, 2]\n\nn = 5\nn\n";
    // Mid-token: exactly what the editor holds after typing `.` on line 4.
    let mid_token = "x = \"abc\"\ny = [1, 2]\n\nn = 5\nn.\n";

    let (mut fixture, _diag) = open_and_diagnose(uri, parses).await?;

    let before = type_hints_after_change(&mut fixture, uri, 2, parses, 150).await?;
    assert_eq!(
        before,
        vec![": str", ": list[int]", ": int"],
        "baseline: the parsing buffer hints all three unannotated variables"
    );

    let during = type_hints_after_change(&mut fixture, uri, 3, mid_token, 151).await?;
    assert_eq!(
        during, before,
        "a trailing `.` on line 4 must not blank the hints on lines 0, 1 and 3 — \
         they were correct one keystroke ago and nothing above the dot changed"
    );

    Ok(())
}

/// The stale-view fallback must not outlive its usefulness: once the buffer
/// parses again, hints come from the CURRENT text, not the retained snapshot.
#[tokio::test]
async fn test_ws_inlay_hints_recover_current_text_after_parse_error() -> TestResult<()> {
    let uri = "file:///inlay_recovery_back.py";
    let parses = "x = \"abc\"\n";
    let mid_token = "x = \"abc\"\nx.\n";
    let repaired = "x = \"abc\"\nlength = len(x)\n";

    let (mut fixture, _diag) = open_and_diagnose(uri, parses).await?;

    let _ = type_hints_after_change(&mut fixture, uri, 2, mid_token, 152).await?;
    let after = type_hints_after_change(&mut fixture, uri, 3, repaired, 153).await?;

    assert_eq!(
        after,
        vec![": str", ": int"],
        "once the buffer parses again the hints must describe the CURRENT text, \
         including the newly added `length` binding"
    );

    Ok(())
}
