//! Tests for [CONFIGEDITOR-ADOPTION] and the legacy `basilisk.disableRule`
//! command. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR.
//!
//! Adoption command round trips over the real WebSocket server: adopt file,
//! adopt workspace, save-driven auto-graduation, and unadopt — each landing
//! through the client `workspace/applyEdit` transaction.

use super::ws_test_common::*;
use super::ws_test_configuration_editor::{
    answer_apply_edit, error_kind, file_uri, request_answering_apply_edit, snapshot_result,
};

// Implements [CONFIGEDITOR-ADOPTION]: adopt demotes current errors to exact-
// path warnings through the client edit; unadopt removes exactly that entry.
#[tokio::test]
async fn adoption_commands_round_trip_through_active_configuration() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let file_name = "config_editor_adopt.py";
    let uri = file_uri(&fixture, file_name);
    fixture
        .did_open(&uri, "def f(x) -> int:\n    return 1\n")
        .await?;
    let _ = fixture.wait_for_diagnostics().await?;

    let adopt = request_answering_apply_edit(
        &mut fixture,
        750,
        "workspace/executeCommand",
        serde_json::json!({ "command": "basilisk.adoptFile", "arguments": [uri] }),
    )
    .await?;
    assert_eq!(
        adopt.pointer("/result/adopted"),
        Some(&serde_json::json!(true)),
        "{adopt}"
    );
    assert!(adopt
        .pointer("/result/demoted")
        .and_then(serde_json::Value::as_i64)
        .is_some_and(|demoted| demoted >= 1));

    let snapshot = snapshot_result(&mut fixture, 751).await?;
    let overrides = snapshot
        .get("pathOverrides")
        .and_then(serde_json::Value::as_array)
        .ok_or("snapshot had no pathOverrides")?;
    let adopted_entry = overrides
        .iter()
        .find(|entry| entry.get("pattern").and_then(serde_json::Value::as_str) == Some(file_name));
    assert!(
        adopted_entry
            .and_then(|entry| entry.get("adoption"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        "adopted file must appear as an adoption path override: {snapshot}"
    );
    assert_eq!(
        snapshot.pointer("/debt/adoptedFiles"),
        Some(&serde_json::json!(1))
    );

    let unadopt = request_answering_apply_edit(
        &mut fixture,
        752,
        "workspace/executeCommand",
        serde_json::json!({ "command": "basilisk.unadoptFile", "arguments": [uri] }),
    )
    .await?;
    assert_eq!(
        unadopt.pointer("/result/unadopted"),
        Some(&serde_json::json!(true)),
        "{unadopt}"
    );

    // With the adoption entry gone, a second unadopt has nothing to remove.
    let repeat = request_answering_apply_edit(
        &mut fixture,
        753,
        "workspace/executeCommand",
        serde_json::json!({ "command": "basilisk.unadoptFile", "arguments": [uri] }),
    )
    .await?;
    assert_eq!(
        repeat.pointer("/result/unadopted"),
        Some(&serde_json::json!(false)),
        "{repeat}"
    );
    Ok(())
}

// Implements [CONFIGEDITOR-ADOPTION]: workspace adoption groups indexed files
// by owning root and reports the demoted total.
#[tokio::test]
async fn adopt_workspace_demotes_every_indexed_error() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_adopt_ws.py");
    fixture
        .did_open(&uri, "def f(x) -> int:\n    return 1\n")
        .await?;
    let _ = fixture.wait_for_diagnostics().await?;

    let adopt = request_answering_apply_edit(
        &mut fixture,
        760,
        "workspace/executeCommand",
        serde_json::json!({ "command": "basilisk.adoptWorkspace", "arguments": [] }),
    )
    .await?;
    assert_eq!(
        adopt.pointer("/result/adopted"),
        Some(&serde_json::json!(true)),
        "{adopt}"
    );
    assert_eq!(
        adopt.pointer("/result/files"),
        Some(&serde_json::json!(1)),
        "{adopt}"
    );
    assert!(adopt
        .pointer("/result/demoted")
        .and_then(serde_json::Value::as_i64)
        .is_some_and(|demoted| demoted >= 1));
    Ok(())
}

// Implements [CONFIGEDITOR-ADOPTION]: fixing an adopted file and saving it
// graduates the healed rules out of the active configuration automatically.
#[tokio::test]
async fn saving_a_fixed_file_graduates_adopted_rules() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_graduate.py");
    fixture
        .did_open(&uri, "def f(x) -> int:\n    return 1\n")
        .await?;
    let _ = fixture.wait_for_diagnostics().await?;

    let adopt = request_answering_apply_edit(
        &mut fixture,
        770,
        "workspace/executeCommand",
        serde_json::json!({ "command": "basilisk.adoptFile", "arguments": [uri] }),
    )
    .await?;
    assert_eq!(
        adopt.pointer("/result/adopted"),
        Some(&serde_json::json!(true)),
        "{adopt}"
    );

    // Fix the violation, then save. The server re-checks on save and must
    // graduate the healed adoption override through another client edit.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": "def f(x: int) -> int:\n    return 1\n" }]
            }
        }))
        .await?;
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": { "textDocument": { "uri": uri } }
        }))
        .await?;

    let mut graduated = false;
    for _ in 0..30 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        let parsed: serde_json::Value = serde_json::from_str(&msg)?;
        if answer_apply_edit(&mut fixture, &parsed).await? {
            continue;
        }
        if parsed.get("method").and_then(serde_json::Value::as_str)
            == Some("basilisk/configurationChanged")
            && parsed.pointer("/params/reason") == Some(&serde_json::json!("autoGraduate"))
        {
            graduated = true;
            break;
        }
    }
    assert!(
        graduated,
        "saving the fixed file must auto-graduate its adopted rules"
    );

    let snapshot = snapshot_result(&mut fixture, 771).await?;
    assert_eq!(
        snapshot.pointer("/debt/adoptedFiles"),
        Some(&serde_json::json!(0)),
        "graduation must clear the adoption entry: {snapshot}"
    );
    Ok(())
}

// Implements [CONFIGEDITOR-OPERATIONS]: the legacy disableRule command routes
// through the same validated editor service as the typed protocol.
#[tokio::test]
async fn disable_rule_command_persists_and_validates() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_disable.py");
    fixture.did_open(&uri, "def f(x):\n    return x\n").await?;
    let _ = fixture.wait_for_diagnostics().await?;

    let disable = request_answering_apply_edit(
        &mut fixture,
        780,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.disableRule",
            "arguments": [{ "rule": "BSK-W0014", "severity": "warning" }]
        }),
    )
    .await?;
    assert_eq!(
        disable.pointer("/result/rule"),
        Some(&serde_json::json!("BSK-W0014")),
        "{disable}"
    );
    assert!(disable
        .pointer("/result/path")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|path| path.ends_with("pyproject.toml")));

    let unknown = request_answering_apply_edit(
        &mut fixture,
        781,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.disableRule",
            "arguments": [{ "rule": "NOT-A-RULE", "severity": "off" }]
        }),
    )
    .await?;
    assert_eq!(error_kind(&unknown), Some("unknownRule"), "{unknown}");

    let foreign = request_answering_apply_edit(
        &mut fixture,
        782,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.disableRule",
            "arguments": [{
                "rule": "BSK-W0014",
                "severity": "off",
                "uri": "https://example.com/app.py"
            }]
        }),
    )
    .await?;
    assert!(
        foreign.get("error").is_some(),
        "non-file URIs must be rejected: {foreign}"
    );
    Ok(())
}
