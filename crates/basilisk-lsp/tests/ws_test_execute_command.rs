#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for LSP: `ws_test_execute_command`.

#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

#[tokio::test]
async fn test_ws_execute_command_organize_imports_returns_success() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Open a document with unsorted imports so the command has something to work with.
    let code = "import os\nimport sys\n\nx: int = 42\n";
    fixture.did_open("file:///ws_exec_cmd_org.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Send workspace/executeCommand with basilisk.organizeImports.
    let resp = fixture
        .request(
            500,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.organizeImports",
                "arguments": ["file:///ws_exec_cmd_org.py"]
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    // The command returns Ok(None), which serializes as {"result": null}.
    assert!(
        parsed.get("result").is_some(),
        "executeCommand should return a result (even if null): {resp}"
    );
    // Must not have an error field.
    assert!(
        parsed.get("error").is_none(),
        "executeCommand should not return an error: {resp}"
    );
    Ok(())
}
