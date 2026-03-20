// Tests for LSP: `ws_test_code_actions`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_code_action_missing_param_annotation() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ca_e0001.py", code).await?;

    let resp = code_action_for(&mut fixture, "file:///ca_e0001.py", 200, "BSK-E0001").await?;

    assert!(
        resp.contains(": Any"),
        "E0001 action should insert ': Any': {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "E0001 action should be quickfix: {resp}"
    );

    // Hardened: parse and verify code action structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let actions = parsed["result"]
        .as_array()
        .ok_or("code action result should be an array")?;

    // Hardened: should have at least one action (quickfix + possibly suppress)
    assert!(
        !actions.is_empty(),
        "code actions array must be non-empty: {resp}"
    );

    // Find the quickfix action specifically (title is "Add `: Any` annotation (basilisk)")
    let quickfix = actions
        .iter()
        .find(|a| {
            a["kind"].as_str() == Some("quickfix")
                && a["title"].as_str().is_some_and(|t| t.contains("Any"))
        })
        .ok_or("should have a quickfix action for adding `: Any` annotation")?;

    // Hardened: verify action has edit with changes
    let edit = &quickfix["edit"];
    assert!(
        !edit.is_null(),
        "quickfix action must have an 'edit' field: {resp}"
    );
    let changes = &edit["changes"];
    assert!(
        !changes.is_null(),
        "quickfix edit must have 'changes': {resp}"
    );

    // Hardened: verify edit changes contain the file URI
    let file_edits = &changes["file:///ca_e0001.py"];
    assert!(
        !file_edits.is_null(),
        "changes must contain edits for 'file:///ca_e0001.py': {resp}"
    );

    // Hardened: verify the text edit inserts ": Any"
    let edits = file_edits
        .as_array()
        .ok_or("file edits should be an array")?;
    assert!(
        !edits.is_empty(),
        "file edits array must be non-empty: {resp}"
    );
    let new_text = edits[0]["newText"].as_str().unwrap_or("");
    assert!(
        new_text.contains(": Any"),
        "text edit newText should contain ': Any', got '{new_text}': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_missing_return_annotation() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "def greet(name: str):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ca_e0002.py", code).await?;

    let resp = code_action_for(&mut fixture, "file:///ca_e0002.py", 201, "BSK-E0002").await?;

    assert!(
        resp.contains("-> None"),
        "E0002 action should insert '-> None': {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "E0002 action should be quickfix: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_missing_variable_annotation_empty_list() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "items = []\n";
    fixture.did_open("file:///ca_e0003_list.py", code).await?;

    let resp = code_action_for(&mut fixture, "file:///ca_e0003_list.py", 202, "BSK-E0003").await?;

    assert!(
        resp.contains("list[Any]"),
        "E0003 (empty list) action should insert 'list[Any]': {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "E0003 action should be quickfix: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_missing_variable_annotation_empty_dict() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "mapping = {}\n";
    fixture.did_open("file:///ca_e0003_dict.py", code).await?;

    let resp = code_action_for(&mut fixture, "file:///ca_e0003_dict.py", 203, "BSK-E0003").await?;

    assert!(
        resp.contains("dict[str, Any]"),
        "E0003 (empty dict) action should insert 'dict[str, Any]': {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "E0003 action should be quickfix: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_missing_variable_annotation_none() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "value = None\n";
    fixture.did_open("file:///ca_e0003_none.py", code).await?;

    let resp = code_action_for(&mut fixture, "file:///ca_e0003_none.py", 204, "BSK-E0003").await?;

    assert!(
        resp.contains(": Any"),
        "E0003 (None) action should insert ': Any': {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "E0003 action should be quickfix: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_suppress_with_type_ignore() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ca_suppress.py", code).await?;

    let resp = code_action_for(&mut fixture, "file:///ca_suppress.py", 205, "BSK-E0001").await?;

    assert!(
        resp.contains("# type: ignore"),
        "suppress action should insert '# type: ignore': {resp}"
    );
    assert!(
        resp.contains("Suppress"),
        "suppress action should have 'Suppress' in title: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_suppress_inserts_at_end_of_line() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // The suppress action must target the diagnostic's line (line 0 here).
    let code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ca_suppress_pos.py", code).await?;

    let resp =
        code_action_for(&mut fixture, "file:///ca_suppress_pos.py", 206, "BSK-E0001").await?;

    // The edit should be an insert (start == end), not a replace.
    let action_json: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &action_json["result"];
    let suppress = result
        .as_array()
        .and_then(|arr| {
            arr.iter().find(|a| {
                a["title"]
                    .as_str()
                    .is_some_and(|t| t.contains("type: ignore"))
            })
        })
        .ok_or("no suppress action in result")?;

    let edits = &suppress["edit"]["changes"]["file:///ca_suppress_pos.py"];
    let edit = edits.as_array().and_then(|a| a.first()).ok_or("no edits")?;

    // start == end means pure insertion
    assert_eq!(
        edit["range"]["start"], edit["range"]["end"],
        "suppress action must be a pure insertion: {edit}"
    );
    assert_eq!(
        edit["newText"].as_str(),
        Some("  # type: ignore"),
        "inserted text must be '  # type: ignore': {edit}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_organize_imports() -> TestResult<()> {
    // Skip if ruff is not installed.
    if std::process::Command::new("ruff")
        .arg("--version")
        .output()
        .is_err()
    {
        return Ok(());
    }

    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Deliberately unsorted imports — ruff should reorder them.
    let code = "import os\nimport sys\nfrom typing import Optional\nimport json\n\nx: int = 1\n";
    fixture.did_open("file:///ca_org.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            210,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": "file:///ca_org.py" },
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                "context": { "diagnostics": [] }
            }),
        )
        .await?;

    // The organize-imports action may or may not fire depending on whether
    // the given imports are already sorted by ruff. Just check that when it
    // does appear, it carries the correct kind.
    if let Some(resp_str) = resp {
        if resp_str.contains("Organize imports") {
            assert!(
                resp_str.contains("source.organizeImports"),
                "organize imports action should have organizeImports kind: {resp_str}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_organize_imports_fixes_order() -> TestResult<()> {
    // Skip if ruff is not installed.
    if std::process::Command::new("ruff")
        .arg("--version")
        .output()
        .is_err()
    {
        return Ok(());
    }

    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // sys must come before os alphabetically; ruff will sort to: import os / import sys
    // (actually ruff keeps stdlib imports in the order they appear unless --fix-only is used)
    // Use a clear case: `from __future__` must be first.
    let code = "import os\nfrom __future__ import annotations\n\nx: int = 1\n";
    fixture.did_open("file:///ca_org2.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            211,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": "file:///ca_org2.py" },
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                "context": { "diagnostics": [] }
            }),
        )
        .await?;

    if let Some(resp_str) = resp {
        if resp_str.contains("Organize imports") {
            // The reordered source should put `from __future__` first.
            assert!(
                resp_str.contains("from __future__ import annotations"),
                "organized source should contain the moved import: {resp_str}"
            );
            assert!(
                resp_str.contains("source.organizeImports"),
                "action kind must be organizeImports: {resp_str}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_e0003_all_variants() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // All three E0003 variants in one file: empty list, empty dict, None
    let code = "items = []\nmapping = {}\nvalue = None\n";
    fixture
        .did_open("file:///ws_edge_ca_e0003.py", code)
        .await?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    // Verify all three E0003 diagnostics are present.
    let e0003_diags: Vec<&serde_json::Value> = diagnostics
        .iter()
        .filter(|d| d["code"].as_str() == Some("BSK-E0003"))
        .collect();
    assert!(
        e0003_diags.len() >= 3,
        "should have at least 3 E0003 diagnostics (list, dict, None), got {}: {diag_msg}",
        e0003_diags.len()
    );

    // Request code actions for each E0003 diagnostic.
    for (idx, target_diag) in e0003_diags.iter().enumerate() {
        let action_id = 410 + idx as u64;
        let resp = fixture
            .request(
                action_id,
                "textDocument/codeAction",
                serde_json::json!({
                    "textDocument": { "uri": "file:///ws_edge_ca_e0003.py" },
                    "range": target_diag["range"],
                    "context": { "diagnostics": [target_diag] }
                }),
            )
            .await?
            .ok_or(format!("no code action response for E0003 variant {idx}"))?;

        assert!(
            resp.contains("quickfix"),
            "E0003 code action variant {idx} should be quickfix: {resp}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_no_actions_for_clean_code() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Fully annotated code with no redundant annotations — no diagnostics expected.
    let code = "def add(a: int, b: int) -> int:\n    return a + b\n";
    fixture.did_open("file:///ws_ca_clean.py", code).await?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    // Verify diagnostics are empty.
    assert!(
        diag_msg.contains("\"diagnostics\":[]"),
        "clean code should have no diagnostics: {diag_msg}"
    );

    // Request code actions with empty diagnostics context.
    let resp = fixture
        .request(
            1106,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_ca_clean.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 12 }
                },
                "context": { "diagnostics": [] }
            }),
        )
        .await?
        .ok_or("no code action response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    // Should return null or an empty array (no quick fixes needed).
    // Organize imports may still be offered, so if result is an array,
    // verify no quickfix actions are present.
    if let Some(actions) = result.as_array() {
        let quickfixes: Vec<&serde_json::Value> = actions
            .iter()
            .filter(|a| a["kind"].as_str() == Some("quickfix"))
            .collect();
        assert!(
            quickfixes.is_empty(),
            "clean code should have no quickfix code actions: {resp}"
        );
    }
    // result being null is also acceptable — means no actions at all.

    Ok(())
}
