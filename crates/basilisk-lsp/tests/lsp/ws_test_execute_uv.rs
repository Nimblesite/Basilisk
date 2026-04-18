// E2E tests for uv command execution via `workspace/executeCommand`.
//
// Tests the full pipeline:
//   1. diagnostic fires for unresolved import
//   2. `workspace/executeCommand` dispatches the uv command
//   3. uv subprocess runs and succeeds
//   4. registry rebuilds and imports re-resolve
//   5. diagnostics clear automatically
//
// These tests require `uv` to be installed. They are skipped if `uv` is not
// available on PATH.

use super::ws_test_common::*;

use std::time::Duration;

use futures_util::StreamExt;

/// Check if `uv` is available on PATH. Tests skip if it is not.
fn uv_available() -> bool {
    std::process::Command::new("uv")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Create a minimal uv project in a temp directory and return its path.
fn create_uv_project(prefix: &str) -> std::path::PathBuf {
    let dir = unique_temp_dir(prefix);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"test-project\"\nversion = \"0.1.0\"\n\
         requires-python = \">=3.12\"\ndependencies = []\n",
    )
    .expect("write pyproject.toml");

    // Run `uv sync` to create venv and lock file so the project is valid.
    let status = std::process::Command::new("uv")
        .args(["sync"])
        .current_dir(&dir)
        .output()
        .expect("uv sync");
    assert!(
        status.status.success(),
        "uv sync failed during setup: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    dir
}

/// Drain all messages from the fixture for up to `duration`, collecting them.
async fn drain_messages(fixture: &mut WsTestFixture, duration: Duration) -> Vec<String> {
    let mut messages = Vec::new();
    loop {
        let msg = tokio::time::timeout(duration, fixture.ws_read.next()).await;
        match msg {
            Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
                messages.push(text.clone());
            }
            _ => break,
        }
    }
    messages
}

/// Wait for a publishDiagnostics notification matching a file name.
/// Returns the raw message if found within `attempts` reads.
async fn wait_for_file_diagnostics(
    fixture: &mut WsTestFixture,
    file_name: &str,
    attempts: usize,
) -> Option<String> {
    for _ in 0..attempts {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") && msg.contains(file_name)
        {
            return Some(msg);
        }
    }
    None
}

// ── Tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_uv_add_dispatches_and_returns_success() -> TestResult<()> {
    if !uv_available() {
        return Ok(());
    }

    let dir = create_uv_project("bsk_uv_add_dispatch");
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // Drain startup messages.
    let _ = drain_messages(&mut fixture, Duration::from_millis(500)).await;

    // Execute `basilisk.uv.add` with a real package.
    let resp = fixture
        .request(
            100,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.uv.add",
                "arguments": ["six"]
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;

    // Must have a result, no error.
    assert!(
        parsed.get("error").is_none(),
        "uv add should not return an error: {resp}"
    );

    let result = &parsed["result"];
    assert_eq!(result["success"], true, "uv add should succeed: {resp}");

    // Verify the package was actually added to pyproject.toml.
    let toml = std::fs::read_to_string(dir.join("pyproject.toml"))?;
    assert!(
        toml.contains("six"),
        "pyproject.toml should list 'six' after uv add: {toml}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_uv_add_dev_dispatches_and_returns_success() -> TestResult<()> {
    if !uv_available() {
        return Ok(());
    }

    let dir = create_uv_project("bsk_uv_add_dev_dispatch");
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;
    let _ = drain_messages(&mut fixture, Duration::from_millis(500)).await;

    let resp = fixture
        .request(
            101,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.uv.addDev",
                "arguments": ["types-six"]
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "uv addDev should not return an error: {resp}"
    );
    assert_eq!(
        parsed["result"]["success"], true,
        "uv addDev should succeed: {resp}"
    );

    // Verify the package was added as a dev dependency.
    let toml = std::fs::read_to_string(dir.join("pyproject.toml"))?;
    assert!(
        toml.contains("types-six"),
        "pyproject.toml should list 'types-six' as dev dep: {toml}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_uv_sync_dispatches_and_returns_success() -> TestResult<()> {
    if !uv_available() {
        return Ok(());
    }

    let dir = create_uv_project("bsk_uv_sync_dispatch");
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;
    let _ = drain_messages(&mut fixture, Duration::from_millis(500)).await;

    let resp = fixture
        .request(
            102,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.uv.sync",
                "arguments": []
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "uv sync should not return an error: {resp}"
    );
    assert_eq!(
        parsed["result"]["success"], true,
        "uv sync should succeed: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_uv_add_triggers_registry_rebuild_and_clears_diagnostics() -> TestResult<()> {
    if !uv_available() {
        return Ok(());
    }

    let dir = create_uv_project("bsk_uv_add_clears_diags");

    // Write a Python file that imports a package we haven't installed yet.
    let py_path = dir.join("app.py");
    std::fs::write(&py_path, "import six\n\nx: int = six.moves\n")?;

    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // Wait for initial diagnostics — should contain BSK-E0010 for `six`.
    let initial_diag = wait_for_file_diagnostics(&mut fixture, "app.py", 15).await;
    assert!(
        initial_diag.is_some(),
        "should receive initial diagnostics for app.py"
    );
    let initial_msg = initial_diag.ok_or("expected initial diagnostics")?;
    assert!(
        initial_msg.contains("BSK-E0010"),
        "initial diagnostics should contain BSK-E0010 for unresolved import: {initial_msg}"
    );

    // Drain any remaining startup messages.
    let _ = drain_messages(&mut fixture, Duration::from_millis(300)).await;

    // Send the `uv add six` command. We manually send and then collect all
    // messages (both the command response and the republished diagnostics),
    // because `rebuild_registry_and_resolve` publishes diagnostics
    // asynchronously after the command completes.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 200,
            "method": "workspace/executeCommand",
            "params": {
                "command": "basilisk.uv.add",
                "arguments": ["six"]
            }
        }))
        .await?;

    // Collect all messages: command response + diagnostics notifications.
    let mut command_succeeded = false;
    let mut final_diag_for_app: Option<String> = None;

    for _ in 0..30 {
        let Some(msg) = fixture.recv().await else {
            break;
        };

        // Check for command response.
        if msg.contains("\"id\":200") {
            let parsed: serde_json::Value = serde_json::from_str(&msg)?;
            assert!(
                parsed.get("error").is_none(),
                "uv add should not return an error: {msg}"
            );
            command_succeeded = parsed["result"]["success"] == true;
        }

        // Capture the latest diagnostics for app.py (may arrive multiple times).
        if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") && msg.contains("app.py")
        {
            final_diag_for_app = Some(msg);
        }

        // Once we have both, stop waiting.
        if command_succeeded && final_diag_for_app.is_some() {
            break;
        }
    }

    assert!(command_succeeded, "uv add six should succeed");

    let updated_msg =
        final_diag_for_app.ok_or("should receive updated diagnostics after uv add")?;

    // Parse the diagnostics array to check E0010 is gone for `six`.
    let updated_json: serde_json::Value = serde_json::from_str(&updated_msg)?;
    let diags = updated_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let has_e0010_for_six = diags.iter().any(|d| {
        d["code"].as_str() == Some("BSK-E0010")
            && d["message"].as_str().is_some_and(|m| m.contains("six"))
    });

    assert!(
        !has_e0010_for_six,
        "BSK-E0010 for 'six' should be cleared after uv add: {updated_msg}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_uv_remove_dispatches_and_returns_success() -> TestResult<()> {
    if !uv_available() {
        return Ok(());
    }

    let dir = create_uv_project("bsk_uv_remove_dispatch");
    let root_uri = format!("file://{}", dir.display());

    // First add a package so we can remove it.
    let add_output = std::process::Command::new("uv")
        .args(["add", "six"])
        .current_dir(&dir)
        .output()?;
    assert!(add_output.status.success(), "setup: uv add six failed");

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;
    let _ = drain_messages(&mut fixture, Duration::from_millis(500)).await;

    let resp = fixture
        .request(
            103,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.uv.remove",
                "arguments": ["six"]
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "uv remove should not return an error: {resp}"
    );
    assert_eq!(
        parsed["result"]["success"], true,
        "uv remove should succeed: {resp}"
    );

    // Verify the package was actually removed from pyproject.toml.
    let toml = std::fs::read_to_string(dir.join("pyproject.toml"))?;
    assert!(
        !toml.contains("six"),
        "pyproject.toml should not list 'six' after uv remove: {toml}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_uv_add_missing_package_arg_returns_null() -> TestResult<()> {
    if !uv_available() {
        return Ok(());
    }

    let dir = create_uv_project("bsk_uv_add_no_arg");
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;
    let _ = drain_messages(&mut fixture, Duration::from_millis(500)).await;

    // Send uv add with no arguments — handler should return Ok(None).
    let resp = fixture
        .request(
            104,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.uv.add",
                "arguments": []
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "missing arg should not be an error, just null result: {resp}"
    );
    assert!(
        parsed["result"].is_null(),
        "missing arg should return null result: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_uv_add_with_object_arg_format() -> TestResult<()> {
    if !uv_available() {
        return Ok(());
    }

    let dir = create_uv_project("bsk_uv_add_obj_arg");
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;
    let _ = drain_messages(&mut fixture, Duration::from_millis(500)).await;

    // Send uv add with object-style argument (VS Code middleware format).
    let resp = fixture
        .request(
            105,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.uv.add",
                "arguments": [{"package": "six"}]
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "uv add with object arg should not error: {resp}"
    );
    assert_eq!(
        parsed["result"]["success"], true,
        "uv add with object arg should succeed: {resp}"
    );

    let toml = std::fs::read_to_string(dir.join("pyproject.toml"))?;
    assert!(
        toml.contains("six"),
        "pyproject.toml should contain 'six' after uv add: {toml}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_uv_lock_dispatches_and_returns_success() -> TestResult<()> {
    if !uv_available() {
        return Ok(());
    }

    let dir = create_uv_project("bsk_uv_lock_dispatch");
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;
    let _ = drain_messages(&mut fixture, Duration::from_millis(500)).await;

    let resp = fixture
        .request(
            106,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.uv.lock",
                "arguments": []
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "uv lock should not return an error: {resp}"
    );
    assert_eq!(
        parsed["result"]["success"], true,
        "uv lock should succeed: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

/// Execute a command and wait for both its response and updated diagnostics.
async fn execute_and_wait_for_diags(
    fixture: &mut WsTestFixture,
    request_id: i64,
    command: &str,
    args: &[&str],
) -> TestResult<(bool, Option<String>)> {
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "workspace/executeCommand",
            "params": {
                "command": command,
                "arguments": args
            }
        }))
        .await?;

    let mut command_succeeded = false;
    let mut final_diag: Option<String> = None;
    let id_str = format!("\"id\":{request_id}");

    for _ in 0..30 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains(&id_str) {
            let parsed: serde_json::Value = serde_json::from_str(&msg)?;
            assert!(
                parsed.get("error").is_none(),
                "command should not error: {msg}"
            );
            command_succeeded = parsed["result"]["success"] == true;
        }
        if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") && msg.contains("main.py")
        {
            final_diag = Some(msg);
        }
        if command_succeeded && final_diag.is_some() {
            break;
        }
    }
    Ok((command_succeeded, final_diag))
}

#[tokio::test]
async fn test_e0010_code_action_to_execute_command_full_flow() -> TestResult<()> {
    if !uv_available() {
        return Ok(());
    }

    let dir = create_uv_project("bsk_uv_e0010_full_flow");

    let py_path = dir.join("main.py");
    std::fs::write(&py_path, "import six\n")?;

    let root_uri = format!("file://{}", dir.display());
    let file_uri = format!("file://{}", py_path.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // Step 1: Wait for BSK-E0010 diagnostic.
    let diag_msg = wait_for_file_diagnostics(&mut fixture, "main.py", 15)
        .await
        .ok_or("no diagnostics for main.py")?;

    assert!(
        diag_msg.contains("BSK-E0010"),
        "should fire BSK-E0010 for unresolved import: {diag_msg}"
    );

    // Step 2: Request code actions for the BSK-E0010 diagnostic.
    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let e0010_diag = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some("BSK-E0010"))
        .ok_or("no BSK-E0010 diagnostic")?;

    let ca_resp = fixture
        .request(
            300,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": file_uri },
                "range": e0010_diag["range"],
                "context": { "diagnostics": [e0010_diag] }
            }),
        )
        .await?
        .ok_or("no code action response")?;

    let ca_json: serde_json::Value = serde_json::from_str(&ca_resp)?;
    let actions = ca_json["result"]
        .as_array()
        .ok_or("expected code actions array")?;

    // Step 3: Find the uv add code action.
    let uv_add_action = actions
        .iter()
        .find(|a| {
            a["command"]["command"].as_str() == Some("basilisk.uv.add")
                || a["title"].as_str().is_some_and(|t| t.contains("uv add"))
        })
        .ok_or(format!(
            "no basilisk.uv.add code action offered for BSK-E0010. Actions: {actions:?}"
        ))?;

    assert!(
        uv_add_action.to_string().contains("six"),
        "uv add action should reference 'six': {uv_add_action}"
    );

    // Step 4: Execute the command and wait for diagnostics.
    let (command_succeeded, final_diag) =
        execute_and_wait_for_diags(&mut fixture, 301, "basilisk.uv.add", &["six"]).await?;

    assert!(command_succeeded, "uv add six should succeed");

    // Step 5: Verify BSK-E0010 for 'six' is cleared.
    let updated_msg = final_diag.ok_or("should receive updated diagnostics after uv add")?;
    let updated_json: serde_json::Value = serde_json::from_str(&updated_msg)?;
    let updated_diags = updated_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let still_has_e0010 = updated_diags.iter().any(|d| {
        d["code"].as_str() == Some("BSK-E0010")
            && d["message"].as_str().is_some_and(|m| m.contains("six"))
    });

    assert!(
        !still_has_e0010,
        "BSK-E0010 for 'six' should be cleared after executing the code action: {updated_msg}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
