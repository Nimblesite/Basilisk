//! Tests for [STUBRES-CREATE-LOCAL]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#stubres-create-local
// E2E test for the `basilisk.stubs.createLocal` quick fix (BSK-E0152).
//
// Drives the full pipeline through `workspace/executeCommand`:
//   1. the command dispatches to the stub handler
//   2. the handler scaffolds `<root>/.basilisk/stubs/<module>.pyi`
//   3. it returns `{ created: true, path }`
//
// Unlike the uv tests this needs no external tooling — it only writes a file.

use super::ws_test_common::*;

#[tokio::test]
async fn test_create_local_stub_dispatches_and_writes_pyi() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_create_local_stub");
    std::fs::create_dir_all(&dir)?;
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    let resp = fixture
        .request(
            300,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.stubs.createLocal",
                "arguments": ["acme_private_pkg"]
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "createLocal should not return an error: {resp}"
    );
    assert_eq!(
        parsed["result"]["created"], true,
        "a fresh stub must report created=true: {resp}"
    );

    // The .pyi must exist on disk under the resolver's auto-included cache dir.
    let stub_path = dir
        .join(".basilisk")
        .join("stubs")
        .join("acme_private_pkg.pyi");
    assert!(
        stub_path.is_file(),
        "stub must be written to {}",
        stub_path.display()
    );
    let body = std::fs::read_to_string(&stub_path)?;
    assert!(
        body.contains("BSK-E0154")
            && body
                .lines()
                .all(|line| line.trim().is_empty() || line.trim_start().starts_with('#')),
        "stub skeleton must be strict (comments only, no live declarations): {body}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_add_stub_member_appends_to_stub() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_add_member");
    let stubs_dir = dir.join(".basilisk").join("stubs");
    std::fs::create_dir_all(&stubs_dir)?;
    let stub = stubs_dir.join("cowsay.pyi");
    std::fs::write(&stub, "# strict stub for cowsay\n")?;
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    let snippet = "def get_output_string(arg0: Any, arg1: Any) -> Any: ...";
    let resp = fixture
        .request(
            310,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.stubs.addMember",
                "arguments": [stub.to_string_lossy(), snippet]
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(parsed.get("error").is_none(), "addMember errored: {resp}");
    assert_eq!(parsed["result"]["added"], true, "{resp}");

    let body = std::fs::read_to_string(&stub)?;
    assert!(body.contains("from typing import Any"), "{body}");
    assert!(body.contains(snippet), "{body}");

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_create_local_stub_no_op_when_module_arg_missing() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_create_local_stub_noarg");
    std::fs::create_dir_all(&dir)?;
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // No arguments: the handler must no-op gracefully, not error or write a file.
    let resp = fixture
        .request(
            301,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.stubs.createLocal",
                "arguments": []
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "missing argument must not surface an error: {resp}"
    );
    assert!(
        parsed["result"].is_null(),
        "missing argument must return a null result (graceful no-op): {resp}"
    );
    assert!(
        !dir.join(".basilisk").exists(),
        "no stub directory should be created when the module arg is missing"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
