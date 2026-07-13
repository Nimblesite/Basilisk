//! Tests for [LSPARCH-CONFIG-EDITOR] / [CONFIGEDITOR-OPERATIONS].
//! See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG-EDITOR.
//!
//! Preview → apply transaction tests for the typed configuration-editor
//! protocol: revision gating, selector validation, single-use previews, and
//! rule-occurrence paging over the real WebSocket server.

use super::ws_test_common::*;
use super::ws_test_configuration_editor::{
    error_kind, file_uri, request_answering_apply_edit, root_uri, rule_state, snapshot_result,
    APPLY, OCCURRENCES, PREVIEW,
};

/// Assert the disable-`BSK-E0001` preview shape and return its preview id.
fn assert_disable_preview_shape(preview: &serde_json::Value) -> TestResult<String> {
    assert_eq!(
        preview.get("expandedRuleCodes"),
        Some(&serde_json::json!(["BSK-E0001"]))
    );
    let change = preview
        .pointer("/changes/0")
        .ok_or("preview reported no concrete change")?;
    assert_eq!(
        change.pointer("/previousSetting/kind"),
        Some(&serde_json::json!("Error"))
    );
    assert_eq!(
        change.pointer("/resultingSetting/kind"),
        Some(&serde_json::json!("Disabled"))
    );
    let before = preview
        .pointer("/impact/diagnosticsBefore")
        .and_then(serde_json::Value::as_i64)
        .ok_or("impact missing diagnosticsBefore")?;
    let after = preview
        .pointer("/impact/diagnosticsAfter")
        .and_then(serde_json::Value::as_i64)
        .ok_or("impact missing diagnosticsAfter")?;
    assert!(
        after < before,
        "disabling a firing rule must project fewer diagnostics ({after} !< {before})"
    );
    Ok(preview
        .get("previewId")
        .and_then(serde_json::Value::as_str)
        .ok_or("preview carried no previewId")?
        .to_owned())
}

// Implements [CONFIGEDITOR-OPERATIONS]: preview → apply is a revision-checked
// transaction that lands through the client edit and refreshes the snapshot.
#[tokio::test]
async fn preview_apply_round_trip_disables_a_rule() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_apply.py");
    fixture.did_open(&uri, "def f(x):\n    return x\n").await?;
    let _ = fixture.wait_for_diagnostics().await?;

    let snapshot = snapshot_result(&mut fixture, 720).await?;
    let revision = snapshot
        .get("revision")
        .and_then(serde_json::Value::as_str)
        .ok_or("snapshot carried no revision")?
        .to_owned();
    let root = root_uri(&fixture);

    let preview_response = request_answering_apply_edit(
        &mut fixture,
        721,
        PREVIEW,
        serde_json::json!({
            "rootUri": root,
            "baseRevision": revision,
            "mutations": [{
                "selector": { "kind": "Codes", "codes": ["BSK-E0001"] },
                "setting": { "kind": "Disabled" },
                "scope": { "kind": "Project" }
            }]
        }),
    )
    .await?;
    let preview = preview_response
        .get("result")
        .filter(|result| !result.is_null())
        .ok_or_else(|| format!("preview returned no result: {preview_response}"))?;
    let preview_id = assert_disable_preview_shape(preview)?;

    let apply_response = request_answering_apply_edit(
        &mut fixture,
        722,
        APPLY,
        serde_json::json!({
            "rootUri": root,
            "previewId": preview_id,
            "baseRevision": revision
        }),
    )
    .await?;
    let applied = apply_response
        .get("result")
        .filter(|result| !result.is_null())
        .ok_or_else(|| format!("apply returned no result: {apply_response}"))?;
    let disabled =
        rule_state(applied, "BSK-E0001").ok_or("BSK-E0001 missing from applied snapshot")?;
    assert_eq!(
        disabled.pointer("/configuredSeverity/kind"),
        Some(&serde_json::json!("Disabled"))
    );
    assert_eq!(
        disabled.pointer("/effectiveSeverity/kind"),
        Some(&serde_json::json!("Disabled"))
    );
    assert_ne!(
        applied.get("revision").and_then(serde_json::Value::as_str),
        Some(revision.as_str()),
        "apply must advance the configuration revision"
    );

    // A preview is single-use: replaying the same id must fail loudly.
    let replay = request_answering_apply_edit(
        &mut fixture,
        723,
        APPLY,
        serde_json::json!({
            "rootUri": root,
            "previewId": preview_id,
            "baseRevision": revision
        }),
    )
    .await?;
    assert_eq!(error_kind(&replay), Some("previewExpired"), "{replay}");
    Ok(())
}

// Implements [CONFIGEDITOR-OPERATIONS]: previews validate emptiness, revision,
// selector contents, and path patterns before any state is touched.
#[tokio::test]
async fn preview_rejects_invalid_requests() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_invalid.py");
    fixture.did_open(&uri, "def f(x):\n    return x\n").await?;
    let _ = fixture.wait_for_diagnostics().await?;
    let snapshot = snapshot_result(&mut fixture, 730).await?;
    let revision = snapshot
        .get("revision")
        .and_then(serde_json::Value::as_str)
        .ok_or("snapshot carried no revision")?
        .to_owned();
    let root = root_uri(&fixture);
    let disable_all = serde_json::json!([{
        "selector": { "kind": "All" },
        "setting": { "kind": "Disabled" },
        "scope": { "kind": "Project" }
    }]);

    let cases = [
        (
            731_u64,
            serde_json::json!({
                "rootUri": root,
                "baseRevision": revision,
                "mutations": []
            }),
            "invalidMutation",
        ),
        (
            732,
            serde_json::json!({
                "rootUri": root,
                "baseRevision": "stale-revision",
                "mutations": disable_all
            }),
            "revisionConflict",
        ),
        (
            733,
            serde_json::json!({
                "rootUri": root,
                "baseRevision": revision,
                "mutations": [{
                    "selector": { "kind": "Codes", "codes": ["NOT-A-RULE"] },
                    "setting": { "kind": "Disabled" },
                    "scope": { "kind": "Project" }
                }]
            }),
            "unknownRule",
        ),
        (
            734,
            serde_json::json!({
                "rootUri": root,
                "baseRevision": revision,
                "mutations": [{
                    "selector": { "kind": "Codes", "codes": ["BSK-E0001"] },
                    "setting": { "kind": "Warning" },
                    "scope": { "kind": "Path", "pattern": "../escape/**" }
                }]
            }),
            "invalidMutation",
        ),
    ];
    for (id, params, expected_kind) in cases {
        let response = request_answering_apply_edit(&mut fixture, id, PREVIEW, params).await?;
        assert_eq!(
            error_kind(&response),
            Some(expected_kind),
            "request {id} must fail with {expected_kind}: {response}"
        );
    }
    Ok(())
}

// Implements [CONFIGEDITOR-OPERATIONS]: occurrences page deterministically,
// carry fix-safety badges, and enforce the documented limit window.
#[tokio::test]
async fn rule_occurrences_page_and_reject_bad_limits() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_occurrences.py");
    fixture
        .did_open(
            &uri,
            "def first(x):\n    return 1\n\ndef second(y):\n    return 2\n",
        )
        .await?;
    let _ = fixture.wait_for_diagnostics().await?;
    let root = root_uri(&fixture);
    let selector = serde_json::json!({ "kind": "Codes", "codes": ["BSK-E0001"] });

    let first_page = request_answering_apply_edit(
        &mut fixture,
        740,
        OCCURRENCES,
        serde_json::json!({ "rootUri": root, "selector": selector, "limit": 1 }),
    )
    .await?;
    let result = first_page
        .get("result")
        .filter(|result| !result.is_null())
        .ok_or_else(|| format!("occurrences returned no result: {first_page}"))?;
    let items = result
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or("occurrences result had no items array")?;
    assert_eq!(items.len(), 1);
    let item = items.first().ok_or("first page must hold one occurrence")?;
    assert_eq!(item.get("ruleCode"), Some(&serde_json::json!("BSK-E0001")));
    assert!(item
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|item_uri| item_uri.ends_with("config_editor_occurrences.py")));
    assert_eq!(
        item.pointer("/fixSafety/kind"),
        Some(&serde_json::json!("Safe"))
    );
    let cursor = result
        .get("nextCursor")
        .and_then(serde_json::Value::as_str)
        .ok_or("two occurrences behind limit 1 must produce a cursor")?
        .to_owned();

    let second_page = request_answering_apply_edit(
        &mut fixture,
        741,
        OCCURRENCES,
        serde_json::json!({
            "rootUri": root,
            "selector": selector,
            "cursor": cursor,
            "limit": 100
        }),
    )
    .await?;
    let second_items = second_page
        .pointer("/result/items")
        .and_then(serde_json::Value::as_array)
        .ok_or("second occurrences page had no items")?;
    assert_eq!(second_items.len(), 1);
    assert_eq!(second_page.pointer("/result/nextCursor"), None);

    for (id, limit) in [(742_u64, 0_i64), (743, 1001), (744, -3)] {
        let response = request_answering_apply_edit(
            &mut fixture,
            id,
            OCCURRENCES,
            serde_json::json!({ "rootUri": root, "selector": selector, "limit": limit }),
        )
        .await?;
        assert_eq!(
            error_kind(&response),
            Some("invalidMutation"),
            "limit {limit} must be rejected: {response}"
        );
    }
    Ok(())
}
