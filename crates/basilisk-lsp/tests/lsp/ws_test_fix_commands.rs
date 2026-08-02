//! Tests for [LSPARCH-FEATURES-EXECCMD], [AUTOFIX-MASS-VSCODE].
//! See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Coverage-boost tests for the `basilisk.fix*` executeCommands and
// `basilisk.organizeImports` (with real edits) driven through the in-process
// WebSocket harness. The fixture auto-answers `workspace/applyEdit`, so the
// full apply → converge-index → report path in `command_fixes.rs` is reached.

use super::ws_test_common::*;

/// Open a document with a redundant annotation (`x: int = 42` → BSK-0050)
/// under the workspace root so fixWorkspace can see it.
async fn fixable_fixture() -> TestResult<(WsTestFixture, String)> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let file = fixture.workspace_root.join("ws_fix_target.py");
    std::fs::write(&file, "x: int = 42\n")?;
    let uri = format!("file://{}", file.display());
    fixture.did_open(&uri, "x: int = 42\n").await?;
    let _ = fixture.wait_for_diagnostics().await;
    Ok((fixture, uri))
}

/// `basilisk.fixFile` applies the Safe BSK-0050 fix and returns a `fixed`
/// count. Covers `fix_file` + `report_file_result` + the applied-true branch
/// of `apply_targets` + `converge_index`.
#[tokio::test]
async fn test_ws_fix_file_applies_safe_fix() -> TestResult<()> {
    let (mut fixture, uri) = fixable_fixture().await?;

    let resp = fixture
        .request(
            800,
            "workspace/executeCommand",
            serde_json::json!({ "command": "basilisk.fixFile", "arguments": [uri] }),
        )
        .await?
        .ok_or("no fixFile response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "fixFile must not error: {resp}"
    );
    assert_eq!(
        parsed["result"]["fixed"].as_u64(),
        Some(1),
        "fixFile should fix exactly 1 BSK-0050 issue: {resp}"
    );

    Ok(())
}

/// `basilisk.fixFileAll` widens to the Unsafe tier too; on a file with only a
/// Safe fix it still returns the Safe count. Covers the `FIX_FILE_ALL` arm.
#[tokio::test]
async fn test_ws_fix_file_all_dispatches() -> TestResult<()> {
    let (mut fixture, uri) = fixable_fixture().await?;

    let resp = fixture
        .request(
            810,
            "workspace/executeCommand",
            serde_json::json!({ "command": "basilisk.fixFileAll", "arguments": [uri] }),
        )
        .await?
        .ok_or("no fixFileAll response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "fixFileAll must not error: {resp}"
    );
    assert!(
        parsed["result"]["fixed"].as_u64().is_some(),
        "fixFileAll should return a fixed count: {resp}"
    );

    Ok(())
}

/// `basilisk.fixWorkspace` scans the workspace root and reports a per-file
/// + total count. Covers `fix_workspace` + `report_workspace_result`.
#[tokio::test]
async fn test_ws_fix_workspace_reports_counts() -> TestResult<()> {
    let (mut fixture, _uri) = fixable_fixture().await?;

    let resp = fixture
        .request(
            820,
            "workspace/executeCommand",
            serde_json::json!({ "command": "basilisk.fixWorkspace", "arguments": [] }),
        )
        .await?
        .ok_or("no fixWorkspace response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "fixWorkspace must not error: {resp}"
    );
    assert!(
        parsed["result"]["fixed"].as_u64().is_some(),
        "fixWorkspace should return a fixed count: {resp}"
    );
    assert!(
        parsed["result"]["files"].as_u64().is_some(),
        "fixWorkspace should return a files count: {resp}"
    );

    Ok(())
}

/// `basilisk.fixFile` on a URI with NO fixable diagnostics returns
/// `{ "fixed": 0 }` — covers the `no fixable diagnostics` early-return.
#[tokio::test]
async fn test_ws_fix_file_no_diagnostics_returns_zero() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    // Clean code — no diagnostics.
    let code = "x = 42\n";
    fixture.did_open("file:///ws_fix_clean.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            830,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.fixFile",
                "arguments": ["file:///ws_fix_clean.py"]
            }),
        )
        .await?
        .ok_or("no fixFile response for clean code")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert_eq!(
        parsed["result"]["fixed"].as_u64(),
        Some(0),
        "fixFile on clean code should fix 0: {resp}"
    );

    Ok(())
}

/// `basilisk.organizeImports` on a file with unsorted imports applies the
/// organize-imports edit via `workspace/applyEdit` (auto-answered) and returns
/// Ok(null). Covers the `execute_organize_imports` apply branch.
#[tokio::test]
async fn test_ws_organize_imports_applies_edit() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    // Reverse-sorted imports so organize has work to do.
    let code = "import sys\nimport os\n\nx: int = 1\n";
    fixture.did_open("file:///ws_org_imports.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            840,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.organizeImports",
                "arguments": ["file:///ws_org_imports.py"]
            }),
        )
        .await?
        .ok_or("no organizeImports response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "organizeImports must not error: {resp}"
    );
    // organizeImports returns Ok(None) → result: null.
    assert!(
        parsed["result"].is_null(),
        "organizeImports should return null result: {resp}"
    );

    Ok(())
}

/// An unknown `basilisk.*` command is logged as a warning and returns
/// Ok(null) — covers the `unknown =>` branch of `dispatch_execute_command`.
#[tokio::test]
async fn test_ws_unknown_command_returns_null() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let resp = fixture
        .request(
            850,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.doesNotExist",
                "arguments": []
            }),
        )
        .await?
        .ok_or("no unknown-command response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "unknown command must not error: {resp}"
    );
    assert!(
        parsed["result"].is_null(),
        "unknown command should return null: {resp}"
    );

    Ok(())
}
