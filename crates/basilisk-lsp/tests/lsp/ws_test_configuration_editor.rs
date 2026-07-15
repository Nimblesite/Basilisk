//! Tests for [LSPARCH-CONFIG-EDITOR], [CONFIGEDITOR-OPERATIONS],
//! [CONFIGEDITOR-MODEL], and [CONFIGEDITOR-SOURCES-OPEN-BUFFER].
//! See docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md.
//!
//! Drives the typed configuration-editor protocol v2 end to end over the real
//! WebSocket server: snapshot → preview → apply (answering the server's
//! `workspace/applyEdit` round trip), rule occurrences, and open-buffer
//! configuration authority.

use super::ws_test_common::*;

pub const SNAPSHOT: &str = "basilisk/configurationSnapshot";
pub const PREVIEW: &str = "basilisk/previewConfigurationChange";
pub const APPLY: &str = "basilisk/applyConfigurationChange";
pub const OCCURRENCES: &str = "basilisk/ruleOccurrences";

pub fn root_uri(fixture: &WsTestFixture) -> String {
    format!("file://{}", fixture.workspace_root.to_string_lossy())
}

pub fn file_uri(fixture: &WsTestFixture, name: &str) -> String {
    format!("{}/{name}", root_uri(fixture))
}

/// Answer a server→client `workspace/applyEdit` request with `{applied: true}`
/// exactly as a real editor would. Returns whether `parsed` was one.
pub async fn answer_apply_edit(
    fixture: &mut WsTestFixture,
    parsed: &serde_json::Value,
) -> TestResult<bool> {
    if parsed.get("method").and_then(serde_json::Value::as_str) != Some("workspace/applyEdit") {
        return Ok(false);
    }
    let edit_id = parsed
        .get("id")
        .cloned()
        .ok_or("workspace/applyEdit request carried no id")?;
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": edit_id,
            "result": { "applied": true }
        }))
        .await?;
    Ok(true)
}

/// Send a request and pump messages until the matching response arrives,
/// answering every interleaved server→client `workspace/applyEdit` request
/// along the way.
pub async fn request_answering_apply_edit(
    fixture: &mut WsTestFixture,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> TestResult<serde_json::Value> {
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }))
        .await?;
    for _ in 0..30 {
        let Some(msg) = fixture.recv().await else {
            return Err(format!("{method}: server went silent before responding").into());
        };
        let parsed: serde_json::Value = serde_json::from_str(&msg)?;
        if answer_apply_edit(fixture, &parsed).await? {
            continue;
        }
        if parsed.get("id") == Some(&serde_json::json!(id)) {
            return Ok(parsed);
        }
    }
    Err(format!("{method}: no response after 30 messages").into())
}

/// Extract the JSON-RPC error `data.kind` discriminator.
pub fn error_kind(response: &serde_json::Value) -> Option<&str> {
    response
        .pointer("/error/data/kind")
        .and_then(serde_json::Value::as_str)
}

pub fn rule_state<'a>(
    snapshot: &'a serde_json::Value,
    code: &str,
) -> Option<&'a serde_json::Value> {
    snapshot.get("rules")?.as_array()?.iter().find(|state| {
        state
            .pointer("/descriptor/code")
            .and_then(serde_json::Value::as_str)
            == Some(code)
    })
}

pub fn tag_state<'a>(snapshot: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    snapshot
        .get("tags")?
        .as_array()?
        .iter()
        .find(|state| state.get("name").and_then(serde_json::Value::as_str) == Some(name))
}

pub async fn snapshot_result(
    fixture: &mut WsTestFixture,
    id: u64,
) -> TestResult<serde_json::Value> {
    let root = root_uri(fixture);
    let response = request_answering_apply_edit(
        fixture,
        id,
        SNAPSHOT,
        serde_json::json!({ "rootUri": root }),
    )
    .await?;
    response
        .get("result")
        .filter(|result| !result.is_null())
        .cloned()
        .ok_or_else(|| format!("configurationSnapshot returned no result: {response}").into())
}

// Implements [CONFIGEDITOR-MODEL]: the snapshot is the root, the config
// document URI, a revision, rule states (entry + effective severity +
// diagnostic count), and tag states with their entries — nothing else.
#[tokio::test]
async fn configuration_snapshot_reflects_live_diagnostics() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_snapshot.py");
    fixture.did_open(&uri, "def f(x):\n    return x\n").await?;
    let _ = fixture.wait_for_diagnostics().await?;

    let snapshot = snapshot_result(&mut fixture, 700).await?;

    assert!(snapshot
        .get("revision")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|revision| !revision.is_empty()));
    assert!(snapshot
        .get("configUri")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|config_uri| config_uri.ends_with("pyproject.toml")));
    let annotation =
        rule_state(&snapshot, "BSK-0001").ok_or("BSK-0001 missing from snapshot rules")?;
    assert_eq!(
        annotation.pointer("/entry/kind"),
        Some(&serde_json::json!("Error"))
    );
    assert_eq!(
        annotation.pointer("/effectiveSeverity/kind"),
        Some(&serde_json::json!("Error"))
    );
    assert!(annotation
        .get("diagnosticCount")
        .and_then(serde_json::Value::as_i64)
        .is_some_and(|count| count >= 1));
    // Removed v1 machinery must be absent from the wire ([CONFIGEDITOR-ACCEPTANCE]).
    for gone in ["presets", "debt", "pathOverrides", "problems", "source"] {
        assert!(
            snapshot.get(gone).is_none(),
            "removed field '{gone}' must not be serialized: {snapshot}"
        );
    }
    // The provenance tag facets are present ([CONFIGEDITOR-TAGS]).
    let pep_tag = tag_state(&snapshot, "pep").ok_or("pep tag missing from snapshot")?;
    assert_eq!(
        pep_tag.pointer("/kind/kind"),
        Some(&serde_json::json!("Provenance"))
    );
    Ok(())
}

// Implements [CONFIGEDITOR-OPERATIONS]: a root the server does not own is an
// authority error, never a silent fallback to some other root.
#[tokio::test]
async fn configuration_snapshot_rejects_unknown_roots() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    for (id, bad_root) in [
        (710_u64, "file:///definitely/not/a/workspace/root"),
        (711, "not a uri at all"),
        (712, "https://example.com/workspace"),
    ] {
        let response = request_answering_apply_edit(
            &mut fixture,
            id,
            SNAPSHOT,
            serde_json::json!({ "rootUri": bad_root }),
        )
        .await?;
        assert_eq!(
            error_kind(&response),
            Some("invalidMutation"),
            "{bad_root} must be rejected: {response}"
        );
    }
    Ok(())
}

// Implements [CONFIGEDITOR-SOURCES-OPEN-BUFFER]: an open configuration buffer
// is authoritative over disk while open, and disk resumes authority on close.
#[tokio::test]
async fn open_configuration_buffer_overrides_disk_until_closed() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let config_uri = file_uri(&fixture, "pyproject.toml");

    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": config_uri,
                    "languageId": "toml",
                    "version": 3,
                    "text": "[tool.basilisk.rules]\n\"BSK-0001\" = \"info\"\n"
                }
            }
        }))
        .await?;

    let buffered = snapshot_result(&mut fixture, 790).await?;
    let buffered_rule =
        rule_state(&buffered, "BSK-0001").ok_or("BSK-0001 missing from buffered snapshot")?;
    assert_eq!(
        buffered_rule.pointer("/entry/kind"),
        Some(&serde_json::json!("Info")),
        "open buffer must override the on-disk severity: {buffered}"
    );

    fixture.did_close(&config_uri).await?;
    let from_disk = snapshot_result(&mut fixture, 791).await?;
    let disk_rule =
        rule_state(&from_disk, "BSK-0001").ok_or("BSK-0001 missing from disk snapshot")?;
    assert_eq!(
        disk_rule.pointer("/entry/kind"),
        Some(&serde_json::json!("Error")),
        "closing the buffer must restore disk authority: {from_disk}"
    );
    Ok(())
}
