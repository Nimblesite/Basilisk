//! Tests for [LSPARCH-FEATURES-FORMAT] / [LSPFMT-ENGINE].
//! See docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-ENGINE
// Tests for LSP: `ws_test_formatting`.
//
// Formatting is served by the EMBEDDED Ruff formatter — always available, no
// external binary ([LSPFMT-DECISION], #254) — so these tests assert
// affirmatively; "ruff might not be installed" is no longer a reason for null.

use super::ws_test_common::*;

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

    let edits = result
        .as_array()
        .ok_or_else(|| format!("formatting must return TextEdits, not null: {resp}"))?;
    let new_text = edits
        .first()
        .and_then(|e| e["newText"].as_str())
        .ok_or("first TextEdit must carry newText")?;

    // Pure passthrough of the embedded Ruff formatter ([LSPFMT-HONESTY]):
    // byte-exact output, verified against `ruff format` 0.15.17.
    assert_eq!(
        new_text,
        "x: int = 1\ny: str = \"hello\"\n\n\ndef greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n",
        "embedded formatter output must match ruff format: {resp}"
    );
    assert!(
        edits.first().is_some_and(|e| e.get("range").is_some()),
        "TextEdit should have a range: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_format_document_matches_real_ruff_binary_output() -> TestResult<()> {
    // Live parity check ([LSPFMT-ENGINE] acceptance): the embedded engine's
    // output must be byte-identical to `ruff format` at the pinned release.
    // Runs wherever a `ruff` binary exists (CI pins ruff==0.15.17); the
    // functionality itself is exercised unconditionally by the tests above.
    let probe = std::process::Command::new("ruff").arg("--version").output();
    if probe.is_err() {
        return Ok(());
    }

    let code = "import functools\nclass  Point :\n    def __init__(self,x:int=0,*,y:int=0)->None:\n        self.x=x ; self.y=y\n    @functools.cached_property\n    def norm(self)->float:\n        return (self.x**2+self.y**2)**0.5\n";

    // Ground truth from the real binary.
    let dir = unique_temp_dir("bsk_fmt_parity");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("parity.py");
    std::fs::write(&path, code)?;
    let status = std::process::Command::new("ruff")
        .arg("format")
        .arg(&path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    assert!(status.success(), "ruff format must succeed on the fixture");
    let expected = std::fs::read_to_string(&path)?;
    let _ = std::fs::remove_dir_all(&dir);

    // Same source through the LSP's embedded engine.
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    fixture
        .did_open("file:///ws_format_parity.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            601,
            "textDocument/formatting",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_format_parity.py" },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        )
        .await?
        .ok_or("no formatting response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let new_text = parsed["result"]
        .as_array()
        .and_then(|edits| edits.first())
        .and_then(|e| e["newText"].as_str())
        .ok_or_else(|| format!("formatting must return TextEdits: {resp}"))?;

    assert_eq!(
        new_text, expected,
        "embedded formatter must be byte-identical to `ruff format`"
    );
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

    // Already-formatted code produces no edit: strictly null.
    assert!(
        parsed["result"].is_null(),
        "already-formatted code must produce no edits: {resp}"
    );
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

    // An empty file is already formatted — strictly no edits.
    assert!(
        parsed.get("result").is_some(),
        "formatting empty file should return a valid result: {resp}"
    );
    assert!(
        parsed["result"].is_null(),
        "empty file must produce no edits: {resp}"
    );
    Ok(())
}
