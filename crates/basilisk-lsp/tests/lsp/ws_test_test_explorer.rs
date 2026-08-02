//! Tests for [LSPTEST-LSP-PROTOCOL-COMMANDS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md
// Coverage-boost tests for the `basilisk.discoverTests` executeCommand: the
// file-scoped path (with a `{ uri }` arg) and the workspace-scoped path (no
// arg, emits the `basilisk/testDiscoveryResult` notification). Covers the
// `execute_discover_tests` + `send_test_discovery_notification` handlers in
// `test_handlers.rs` that the unit tests don't reach.

use super::ws_test_common::*;

/// `basilisk.discoverTests` with a `{ uri }` argument discovers the `test_*`
/// functions in that file and returns them as items.
#[tokio::test]
async fn test_ws_discover_tests_in_file() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
def test_addition() -> None:
    assert 1 + 1 == 2

def test_subtraction() -> None:
    assert 2 - 1 == 1

def helper() -> None:
    pass
";
    fixture.did_open("file:///ws_discover.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            700,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.discoverTests",
                "arguments": [{ "uri": "file:///ws_discover.py" }]
            }),
        )
        .await?
        .ok_or("no discoverTests response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "discoverTests must not error: {resp}"
    );
    let items = parsed["result"]["items"]
        .as_array()
        .ok_or("discoverTests should return an items array")?;
    let labels: Vec<&str> = items
        .iter()
        .filter_map(|i| i["name"].as_str().or_else(|| i["label"].as_str()))
        .collect();
    assert!(
        labels.iter().any(|l| l.contains("test_addition")),
        "items should include test_addition: {resp}"
    );
    assert!(
        labels.iter().any(|l| l.contains("test_subtraction")),
        "items should include test_subtraction: {resp}"
    );
    assert!(
        !labels.iter().any(|l| l.contains("helper")),
        "non-test helper must not be discovered: {resp}"
    );

    Ok(())
}

/// `basilisk.discoverTests` with no URI does a full workspace scan and emits
/// the `basilisk/testDiscoveryResult` notification. The workspace fixture root
/// has a `pyproject.toml` but no test files, so the notification's items may
/// be empty — we assert the notification is delivered (not that items exist).
#[tokio::test]
async fn test_ws_discover_tests_workspace_emits_notification() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Write a test file (must match test_*.py) under the workspace root so
    // workspace discovery finds it.
    let test_file = fixture.workspace_root.join("test_ws_discover.py");
    std::fs::write(
        &test_file,
        "def test_workspace() -> None:\n    assert True\n",
    )?;
    let uri = format!("file://{}", test_file.display());
    fixture
        .did_open(&uri, "def test_workspace() -> None:\n    assert True\n")
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            710,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.discoverTests",
                "arguments": []
            }),
        )
        .await?
        .ok_or("no discoverTests workspace response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "workspace discoverTests must not error: {resp}"
    );
    assert!(
        parsed["result"]["items"].is_array(),
        "workspace discoverTests should return an items array: {resp}"
    );

    // The handler ALSO sends a `basilisk/testDiscoveryResult` notification,
    // but it is emitted right before the response and is consumed by the
    // request loop (it carries no id). The response itself carries the items,
    // so we assert the workspace scan found the test file instead.
    assert!(
        resp.contains("test_workspace"),
        "workspace discoverTests should find test_workspace in the response: {resp}"
    );
    assert!(
        resp.contains("test_ws_discover.py"),
        "workspace discoverTests should list the test file: {resp}"
    );

    let _ = std::fs::remove_file(&test_file);
    Ok(())
}

/// `basilisk.discoverTests` with a `{ uri }` pointing at an UNOPENED file
/// (empty source in the index) returns an empty items array — covers the
/// `source.is_empty()` early-return.
#[tokio::test]
async fn test_ws_discover_tests_unopened_file_returns_empty() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let resp = fixture
        .request(
            720,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.discoverTests",
                "arguments": [{ "uri": "file:///never_opened_tests.py" }]
            }),
        )
        .await?
        .ok_or("no discoverTests response for unopened file")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let items = parsed["result"]["items"]
        .as_array()
        .ok_or("should return an items array")?;
    assert!(
        items.is_empty(),
        "discoverTests for an unopened/empty file must return empty items: {resp}"
    );

    Ok(())
}
