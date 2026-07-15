//! Tests for [AUTOFIX-ADOPTION] / [AUTOFIX-ADOPTION-FLOW] and the legacy
//! `basilisk.disableRule` command ([CONFIGEDITOR-OPERATIONS]).
//! See docs/specs/LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX-ADOPTION.
//!
//! Adoption command round trips over the real WebSocket server: adopt file,
//! adopt workspace, and unadopt — each landing as plain folder-level rule
//! entries through the client `workspace/applyEdit` transaction. There is no
//! post-save graduation ([AUTOFIX-ADOPTION-RULES]).

use super::ws_test_common::*;
use super::ws_test_configuration_editor::{
    error_kind, file_uri, request_answering_apply_edit, rule_state, snapshot_result,
};

// Implements [AUTOFIX-ADOPTION-FLOW]: adopt demotes current error codes to
// plain warning entries in the root config through the client edit; unadopt
// removes exactly those entries again.
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

    // The debt landed as a plain warning-severity rule entry — one visible
    // line of configuration, no markers ([AUTOFIX-ADOPTION]).
    let snapshot = snapshot_result(&mut fixture, 751).await?;
    let adopted =
        rule_state(&snapshot, "BSK-0001").ok_or("BSK-0001 missing from adopted snapshot")?;
    assert_eq!(
        adopted.pointer("/entry/kind"),
        Some(&serde_json::json!("Warning")),
        "adoption must demote the firing error to a warning entry: {snapshot}"
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
    let restored = snapshot_result(&mut fixture, 753).await?;
    let rule = rule_state(&restored, "BSK-0001").ok_or("BSK-0001 missing after unadopt")?;
    assert_eq!(
        rule.get("entry"),
        None,
        "unadopt must delete the warning entry: {restored}"
    );

    // With the warning entries gone, a second unadopt has nothing to remove.
    let repeat = request_answering_apply_edit(
        &mut fixture,
        754,
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

// Implements [AUTOFIX-ADOPTION-FLOW]: workspace adoption groups indexed files
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

// Implements [CONFIGEDITOR-OPERATIONS]: the legacy disableRule command routes
// through the same validated editor service as the typed protocol, and is
// rejected for pep-tagged rules ([CHKARCH-CONFIG-MODEL]).
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
            "arguments": [{ "rule": "BSK-0014", "severity": "warning" }]
        }),
    )
    .await?;
    assert_eq!(
        disable.pointer("/result/rule"),
        Some(&serde_json::json!("BSK-0014")),
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

    // [CHKARCH-CONFIG-MODEL]: pep rules are graded, never disabled.
    let pep = request_answering_apply_edit(
        &mut fixture,
        782,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.disableRule",
            "arguments": [{ "rule": "assignment_compatibility", "severity": "off" }]
        }),
    )
    .await?;
    assert_eq!(error_kind(&pep), Some("pepRuleDisable"), "{pep}");

    let foreign = request_answering_apply_edit(
        &mut fixture,
        783,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.disableRule",
            "arguments": [{
                "rule": "BSK-0014",
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
