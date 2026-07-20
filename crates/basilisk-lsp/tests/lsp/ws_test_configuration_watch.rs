//! Tests for [LSPARCH-CONFIG]. See
//! docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG.
//!
//! The LSP itself watches the active configuration source (the root's
//! `pyproject.toml`) and reactively pushes updates — republished diagnostics
//! plus `basilisk/configurationChanged` — for EVERY way the configuration can
//! change, including external disk edits that arrive through no LSP channel
//! at all. Clients without file-watcher support (Zed advertises none —
//! docs/specs/ZED-SPEC.md) get identical behaviour to VS Code.

use super::ws_test_common::*;
use super::ws_test_configuration_editor::{
    answer_apply_edit, file_uri, root_uri, rule_state, APPLY,
};
use super::ws_test_configuration_preview::{preview_id_of, preview_result, snapshot_revision};

/// [LSPARCH-CONFIG]: rewriting `pyproject.toml` on disk — with NO
/// `workspace/didChangeWatchedFiles`, no open config buffer, no LSP message
/// of any kind — must be detected by the server's own watcher, which then
/// rechecks the workspace, republishes the open file's diagnostics at the new
/// severity, and emits `basilisk/configurationChanged` so UIs refresh.
#[tokio::test]
async fn disk_config_edit_without_client_watcher_republishes_and_notifies() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let uri = format!(
        "file://{}/watched_config_app.py",
        fixture.workspace_root.to_string_lossy()
    );
    fixture
        .did_open(&uri, "def handler(value):\n    return value\n")
        .await?;
    let raw = fixture.wait_for_diagnostics().await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;
    let diag = extract_diagnostic(&json, "BSK-0001")
        .ok_or("BSK-0001 should fire before the config change")?;
    assert_eq!(
        diag["severity"].as_u64(),
        Some(1),
        "BSK-0001 must start at error severity: {diag}"
    );

    // External edit: BSK-0001 drops to "warning" on disk. Deliberately sent
    // through no LSP channel — the server must notice on its own.
    std::fs::write(
        fixture.workspace_root.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"BSK-0001\" = \"warning\"\n\"BSK-0002\" = \"error\"\n",
    )?;

    let mut saw_configuration_changed = false;
    let mut saw_warning_severity = false;
    for _ in 0..20 {
        if saw_configuration_changed && saw_warning_severity {
            break;
        }
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains("\"method\":\"basilisk/configurationChanged\"") {
            saw_configuration_changed = true;
        }
        if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") {
            let parsed: serde_json::Value = serde_json::from_str(&msg)?;
            if parsed["params"]["uri"].as_str() == Some(uri.as_str()) {
                if let Some(updated) = extract_diagnostic(&parsed, "BSK-0001") {
                    saw_warning_severity = updated["severity"].as_u64() == Some(2);
                }
            }
        }
    }

    assert!(
        saw_configuration_changed,
        "server never sent basilisk/configurationChanged after an external \
         disk edit — the configuration is not watched server-side"
    );
    assert!(
        saw_warning_severity,
        "BSK-0001 was never republished at warning severity after the \
         external disk edit — diagnostics went stale"
    );
    Ok(())
}

/// What the client observed while the apply round trip ran: the refresh tail
/// fires before the apply response, so both signals arrive interleaved.
struct ReactivitySignals {
    configuration_changed: bool,
    warning_republished: bool,
}

/// Send `basilisk/applyConfigurationChange` and pump the connection, answering
/// the server's `workspace/applyEdit` request, until the apply response AND
/// both reactivity signals (republish at the new severity for `python_uri`,
/// `basilisk/configurationChanged`) have been observed — or the message budget
/// runs out, leaving the flags false for the caller's assertions.
async fn apply_observing_reactivity(
    fixture: &mut WsTestFixture,
    id: u64,
    preview_id: &str,
    python_uri: &str,
) -> TestResult<ReactivitySignals> {
    let root = root_uri(fixture);
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": APPLY,
            "params": { "rootUri": root, "previewId": preview_id }
        }))
        .await?;
    let mut signals = ReactivitySignals {
        configuration_changed: false,
        warning_republished: false,
    };
    let mut apply_succeeded = false;
    for _ in 0..40 {
        if apply_succeeded && signals.configuration_changed && signals.warning_republished {
            break;
        }
        let Some(msg) = fixture.recv().await else {
            break;
        };
        let parsed: serde_json::Value = serde_json::from_str(&msg)?;
        if answer_apply_edit(fixture, &parsed).await? {
            continue;
        }
        if parsed.get("id") == Some(&serde_json::json!(id)) {
            apply_succeeded = !parsed["result"].is_null();
            continue;
        }
        match parsed.get("method").and_then(serde_json::Value::as_str) {
            Some("basilisk/configurationChanged") => signals.configuration_changed = true,
            Some("textDocument/publishDiagnostics")
                if parsed["params"]["uri"].as_str() == Some(python_uri) =>
            {
                if let Some(updated) = extract_diagnostic(&parsed, "BSK-0001") {
                    signals.warning_republished |= updated["severity"].as_u64() == Some(2);
                }
            }
            _ => {}
        }
    }
    if !apply_succeeded {
        return Err("applyConfigurationChange never returned a successful result".into());
    }
    Ok(signals)
}

/// Answer `workspace/applyEdit` the way a REAL editor does: write the edit's
/// replacement text to disk before replying `applied: true`. The plain
/// `answer_apply_edit` helper only claims success, so it never exercises the
/// interaction between an applied edit and the server's own disk watcher
/// ([LSPARCH-CONFIG]) — the path every VS Code / Zed user actually runs.
/// Returns whether `parsed` was a `workspace/applyEdit` request.
async fn answer_apply_edit_writing_disk(
    fixture: &mut WsTestFixture,
    parsed: &serde_json::Value,
) -> TestResult<bool> {
    if parsed.get("method").and_then(serde_json::Value::as_str) != Some("workspace/applyEdit") {
        return Ok(false);
    }
    let operations = parsed
        .pointer("/params/edit/documentChanges")
        .and_then(serde_json::Value::as_array)
        .ok_or("workspace/applyEdit carried no documentChanges operations")?;
    for operation in operations {
        let (Some(uri), Some(new_text)) = (
            operation
                .pointer("/textDocument/uri")
                .and_then(serde_json::Value::as_str),
            operation
                .pointer("/edits/0/newText")
                .and_then(serde_json::Value::as_str),
        ) else {
            continue;
        };
        let path = uri
            .strip_prefix("file://")
            .ok_or("configuration edit URI was not a file URI")?;
        std::fs::write(path, new_text)?;
    }
    answer_apply_edit(fixture, parsed).await
}

/// Drive one snapshot → preview → apply cycle against a real editor that
/// writes the configuration file to disk, then pump the connection until the
/// open Python file's diagnostics are republished with `code` at `severity`.
async fn apply_severity_and_await_republish(
    fixture: &mut WsTestFixture,
    id: u64,
    python_uri: &str,
    code: &str,
    severity: u64,
) -> TestResult<bool> {
    let revision = snapshot_revision(fixture, id).await?;
    let preview = preview_result(
        fixture,
        id + 1,
        &revision,
        serde_json::json!([{
            "kind": "SetRule",
            "code": code,
            "severity": { "kind": if severity == 1 { "Error" } else { "Warning" } }
        }]),
    )
    .await?;
    let preview_id = preview_id_of(&preview)?;
    let root = root_uri(fixture);
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id + 2,
            "method": APPLY,
            "params": { "rootUri": root, "previewId": preview_id }
        }))
        .await?;
    for _ in 0..60 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        let parsed: serde_json::Value = serde_json::from_str(&msg)?;
        if answer_apply_edit_writing_disk(fixture, &parsed).await? {
            continue;
        }
        if parsed.get("method").and_then(serde_json::Value::as_str)
            == Some("textDocument/publishDiagnostics")
            && parsed["params"]["uri"].as_str() == Some(python_uri)
        {
            if let Some(updated) = extract_diagnostic(&parsed, code) {
                if updated["severity"].as_u64() == Some(severity) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Answer `workspace/applyEdit` for a configuration source the editor holds
/// OPEN: apply the edit to the buffer (`didChange` at the next version), write
/// it to disk, and report success — exactly what VS Code does when
/// `pyproject.toml` is showing in a tab. This drives the server's
/// `pending_open_edits` path ([CONFIGEDITOR-SOURCES-OPEN-BUFFER]) rather than
/// the closed-source `applied` overlay. Returns the version it published.
async fn answer_apply_edit_for_open_source(
    fixture: &mut WsTestFixture,
    parsed: &serde_json::Value,
    config_uri: &str,
    version: i32,
) -> TestResult<bool> {
    if parsed.get("method").and_then(serde_json::Value::as_str) != Some("workspace/applyEdit") {
        return Ok(false);
    }
    let new_text = parsed
        .pointer("/params/edit/documentChanges/0/edits/0/newText")
        .and_then(serde_json::Value::as_str)
        .ok_or("configuration edit carried no replacement text")?
        .to_owned();
    let path = config_uri
        .strip_prefix("file://")
        .ok_or("configuration URI was not a file URI")?;
    std::fs::write(path, &new_text)?;
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": config_uri, "version": version },
                "contentChanges": [{ "text": new_text }]
            }
        }))
        .await?;
    answer_apply_edit(fixture, parsed).await
}

/// [LSPARCH-CONFIG] / [CONFIGEDITOR-SOURCES-OPEN-BUFFER]: applying a severity
/// change while the editor holds `pyproject.toml` open must take effect on the
/// FIRST apply. Regression test for the reported "apply is one change behind",
/// where the panel kept rendering the PREVIOUS apply's severities.
#[tokio::test]
async fn consecutive_applies_with_open_config_buffer_are_never_one_behind() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let uri = file_uri(&fixture, "open_buffer_apply.py");
    fixture
        .did_open(&uri, "def handler(value):\n    return value\n")
        .await?;
    let _ = fixture.wait_for_diagnostics().await?;

    // The editor is showing pyproject.toml, so the server must treat the open
    // buffer as authoritative for every apply that follows.
    let config_path = fixture.workspace_root.join("pyproject.toml");
    let config_uri = format!("file://{}", config_path.to_string_lossy());
    let config_text = std::fs::read_to_string(&config_path)?;
    fixture.did_open(&config_uri, &config_text).await?;

    for (index, (id, severity)) in [(970_u64, 2_u64), (980, 1), (990, 2)].into_iter().enumerate() {
        let revision = snapshot_revision(&mut fixture, id).await?;
        let preview = preview_result(
            &mut fixture,
            id + 1,
            &revision,
            serde_json::json!([{
                "kind": "SetRule",
                "code": "BSK-0001",
                "severity": { "kind": if severity == 1 { "Error" } else { "Warning" } }
            }]),
        )
        .await?;
        let preview_id = preview_id_of(&preview)?;
        let root = root_uri(&fixture);
        fixture
            .send_json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": id + 2,
                "method": APPLY,
                "params": { "rootUri": root, "previewId": preview_id }
            }))
            .await?;

        // The editor's buffer advances one version per applied edit.
        let next_version = i32::try_from(index).unwrap_or(0) + 2;
        let mut republished = false;
        let mut applied_snapshot = None;
        for _ in 0..60 {
            if republished && applied_snapshot.is_some() {
                break;
            }
            let Some(msg) = fixture.recv().await else {
                break;
            };
            let parsed: serde_json::Value = serde_json::from_str(&msg)?;
            if answer_apply_edit_for_open_source(&mut fixture, &parsed, &config_uri, next_version)
                .await?
            {
                continue;
            }
            if parsed.get("id") == Some(&serde_json::json!(id + 2)) {
                applied_snapshot = parsed.get("result").filter(|r| !r.is_null()).cloned();
                continue;
            }
            if parsed.get("method").and_then(serde_json::Value::as_str)
                == Some("textDocument/publishDiagnostics")
                && parsed["params"]["uri"].as_str() == Some(uri.as_str())
            {
                if let Some(updated) = extract_diagnostic(&parsed, "BSK-0001") {
                    republished |= updated["severity"].as_u64() == Some(severity);
                }
            }
        }

        let snapshot = applied_snapshot
            .ok_or_else(|| format!("apply #{} returned no snapshot", index + 1))?;
        let effective = rule_state(&snapshot, "BSK-0001")
            .and_then(|state| state.pointer("/effectiveSeverity/kind"))
            .and_then(serde_json::Value::as_str)
            .ok_or("applied snapshot carried no BSK-0001 effective severity")?;
        let expected = if severity == 1 { "Error" } else { "Warning" };
        assert_eq!(
            effective,
            expected,
            "apply #{}: the snapshot the configuration editor renders is one \
             apply behind — it reports {effective}, not the just-applied \
             {expected}",
            index + 1
        );
        assert!(
            republished,
            "apply #{}: BSK-0001 was never republished at the applied severity \
             — diagnostics are one apply behind",
            index + 1
        );
    }
    Ok(())
}

/// [LSPARCH-CONFIG] / [CONFIGEDITOR-OPERATIONS]: every rule-severity change
/// applied through the configuration editor must take effect on the FIRST
/// apply, including the second and later applies in one session — against a
/// real editor that writes the configuration file to disk. Regression test for
/// the reported "apply is one change behind": the UI kept showing the previous
/// apply's severities because the refresh after an applied edit went stale.
#[tokio::test]
async fn consecutive_ui_applies_each_take_effect_immediately() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let uri = file_uri(&fixture, "consecutive_apply.py");
    fixture
        .did_open(&uri, "def handler(value):\n    return value\n")
        .await?;
    let raw = fixture.wait_for_diagnostics().await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;
    let diag = extract_diagnostic(&json, "BSK-0001")
        .ok_or("BSK-0001 should fire before any configuration change")?;
    assert_eq!(
        diag["severity"].as_u64(),
        Some(1),
        "BSK-0001 must start at error severity: {diag}"
    );

    assert!(
        apply_severity_and_await_republish(&mut fixture, 940, &uri, "BSK-0001", 2).await?,
        "first apply: BSK-0001 was never republished at warning severity"
    );
    assert!(
        apply_severity_and_await_republish(&mut fixture, 950, &uri, "BSK-0001", 1).await?,
        "second apply: BSK-0001 was never republished at error severity — the \
         configuration editor is one apply behind"
    );
    assert!(
        apply_severity_and_await_republish(&mut fixture, 960, &uri, "BSK-0001", 2).await?,
        "third apply: BSK-0001 was never republished at warning severity — the \
         configuration editor is one apply behind"
    );
    Ok(())
}

/// [LSPARCH-CONFIG] / [CONFIGEDITOR-OPERATIONS]: changing a rule severity
/// through the configuration-editor UI protocol (snapshot → preview → apply)
/// must run the same shared refresh tail as an external disk edit — the open
/// Python file's diagnostics are republished at the new severity and
/// `basilisk/configurationChanged` is pushed, with no client polling.
#[tokio::test]
async fn ui_apply_republishes_diagnostics_and_notifies() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let uri = file_uri(&fixture, "ui_apply_reactivity.py");
    fixture
        .did_open(&uri, "def handler(value):\n    return value\n")
        .await?;
    let raw = fixture.wait_for_diagnostics().await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;
    let diag = extract_diagnostic(&json, "BSK-0001")
        .ok_or("BSK-0001 should fire before the configuration change")?;
    assert_eq!(
        diag["severity"].as_u64(),
        Some(1),
        "BSK-0001 must start at error severity: {diag}"
    );

    let revision = snapshot_revision(&mut fixture, 910).await?;
    let preview = preview_result(
        &mut fixture,
        911,
        &revision,
        serde_json::json!([{
            "kind": "SetRule",
            "code": "BSK-0001",
            "severity": { "kind": "Warning" }
        }]),
    )
    .await?;
    let preview_id = preview_id_of(&preview)?;

    let signals = apply_observing_reactivity(&mut fixture, 912, &preview_id, &uri).await?;
    assert!(
        signals.configuration_changed,
        "server never sent basilisk/configurationChanged after a UI apply — \
         the configuration editor is not reactive"
    );
    assert!(
        signals.warning_republished,
        "BSK-0001 was never republished at warning severity after a UI \
         apply — diagnostics went stale"
    );
    Ok(())
}
