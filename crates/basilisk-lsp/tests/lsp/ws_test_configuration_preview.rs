//! Tests for [LSPARCH-CONFIG-EDITOR] / [CONFIGEDITOR-OPERATIONS] /
//! [CHKARCH-CONFIG-MODEL].
//! See docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-OPERATIONS.
//!
//! Preview → apply transaction tests for the typed configuration-editor
//! protocol v2: the four `EditorMutation`s, pep-disable rejection, revision
//! gating, single-use previews, and rule-occurrence paging over the real
//! WebSocket server.

use super::ws_test_common::*;
use super::ws_test_configuration_editor::{
    error_kind, file_uri, request_answering_apply_edit, root_uri, rule_state, snapshot_result,
    tag_state, APPLY, OCCURRENCES, PREVIEW,
};

pub async fn snapshot_revision(fixture: &mut WsTestFixture, id: u64) -> TestResult<String> {
    let snapshot = snapshot_result(fixture, id).await?;
    Ok(snapshot
        .get("revision")
        .and_then(serde_json::Value::as_str)
        .ok_or("snapshot carried no revision")?
        .to_owned())
}

pub async fn preview_result(
    fixture: &mut WsTestFixture,
    id: u64,
    revision: &str,
    mutations: serde_json::Value,
) -> TestResult<serde_json::Value> {
    let root = root_uri(fixture);
    let response = request_answering_apply_edit(
        fixture,
        id,
        PREVIEW,
        serde_json::json!({
            "rootUri": root,
            "baseRevision": revision,
            "mutations": mutations
        }),
    )
    .await?;
    response
        .get("result")
        .filter(|result| !result.is_null())
        .cloned()
        .ok_or_else(|| format!("preview returned no result: {response}").into())
}

async fn apply_preview(
    fixture: &mut WsTestFixture,
    id: u64,
    preview_id: &str,
) -> TestResult<serde_json::Value> {
    let root = root_uri(fixture);
    request_answering_apply_edit(
        fixture,
        id,
        APPLY,
        serde_json::json!({ "rootUri": root, "previewId": preview_id }),
    )
    .await
}

pub fn preview_id_of(preview: &serde_json::Value) -> TestResult<String> {
    Ok(preview
        .get("previewId")
        .and_then(serde_json::Value::as_str)
        .ok_or("preview carried no previewId")?
        .to_owned())
}

// Implements [CONFIGEDITOR-OPERATIONS]: preview → apply is a revision-checked
// transaction identified by rootUri + previewId alone; the applied snapshot
// reflects the resolved change and a preview is strictly single-use.
#[tokio::test]
async fn preview_apply_round_trip_disables_an_analyze_rule() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_apply.py");
    fixture.did_open(&uri, "def f(x):\n    return x\n").await?;
    let _ = fixture.wait_for_diagnostics().await?;

    let revision = snapshot_revision(&mut fixture, 720).await?;
    let preview = preview_result(
        &mut fixture,
        721,
        &revision,
        serde_json::json!([{
            "kind": "SetRule",
            "code": "BSK-0001",
            "severity": { "kind": "Disabled" }
        }]),
    )
    .await?;

    let change = preview
        .pointer("/changes/0")
        .ok_or("preview reported no concrete change")?;
    assert_eq!(change.get("code"), Some(&serde_json::json!("BSK-0001")));
    assert_eq!(
        change.pointer("/before/kind"),
        Some(&serde_json::json!("Error"))
    );
    assert_eq!(
        change.pointer("/after/kind"),
        Some(&serde_json::json!("Disabled"))
    );
    let errors_before = preview
        .pointer("/impact/errorsBefore")
        .and_then(serde_json::Value::as_i64)
        .ok_or("impact missing errorsBefore")?;
    let errors_after = preview
        .pointer("/impact/errorsAfter")
        .and_then(serde_json::Value::as_i64)
        .ok_or("impact missing errorsAfter")?;
    assert!(
        errors_after < errors_before,
        "disabling a firing rule must project fewer errors ({errors_after} !< {errors_before})"
    );
    let preview_id = preview_id_of(&preview)?;

    let apply_response = apply_preview(&mut fixture, 722, &preview_id).await?;
    let applied = apply_response
        .get("result")
        .filter(|result| !result.is_null())
        .ok_or_else(|| format!("apply returned no result: {apply_response}"))?;
    let disabled =
        rule_state(applied, "BSK-0001").ok_or("BSK-0001 missing from applied snapshot")?;
    assert_eq!(
        disabled.pointer("/entry/kind"),
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
    let replay = apply_preview(&mut fixture, 723, &preview_id).await?;
    assert_eq!(error_kind(&replay), Some("previewExpired"), "{replay}");
    Ok(())
}

// Implements [CONFIGEDITOR-OPERATIONS]: SetTag / RemoveTag / RemoveRule
// round-trip through preview/apply as plain `[tool.basilisk.rule-tags]` /
// `[tool.basilisk.rules]` entries.
#[tokio::test]
async fn tag_and_rule_entries_round_trip_through_preview_apply() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_tags.py");
    fixture.did_open(&uri, "def f(x):\n    return x\n").await?;
    let _ = fixture.wait_for_diagnostics().await?;

    // SetTag basilisk=warning + RemoveRule BSK-0001 in one preview.
    let revision = snapshot_revision(&mut fixture, 725).await?;
    let preview = preview_result(
        &mut fixture,
        726,
        &revision,
        serde_json::json!([
            { "kind": "SetTag", "tag": "basilisk", "severity": { "kind": "Warning" } },
            { "kind": "RemoveRule", "code": "BSK-0001" }
        ]),
    )
    .await?;
    let preview_id = preview_id_of(&preview)?;
    let apply_response = apply_preview(&mut fixture, 727, &preview_id).await?;
    let applied = apply_response
        .get("result")
        .filter(|result| !result.is_null())
        .ok_or_else(|| format!("apply returned no result: {apply_response}"))?;

    let basilisk_tag =
        tag_state(applied, "basilisk").ok_or("basilisk tag missing from applied snapshot")?;
    assert_eq!(
        basilisk_tag.pointer("/entry/kind"),
        Some(&serde_json::json!("Warning")),
        "the tag entry must land in [tool.basilisk.rule-tags]: {applied}"
    );
    let annotation =
        rule_state(applied, "BSK-0001").ok_or("BSK-0001 missing from applied snapshot")?;
    assert_eq!(
        annotation.get("entry"),
        None,
        "RemoveRule must delete the per-rule entry: {applied}"
    );
    assert_eq!(
        annotation.pointer("/effectiveSeverity/kind"),
        Some(&serde_json::json!("Warning")),
        "without its own entry the rule takes the tag entry: {applied}"
    );

    // RemoveTag switches the analyze rules back off.
    let revision = applied
        .get("revision")
        .and_then(serde_json::Value::as_str)
        .ok_or("applied snapshot carried no revision")?
        .to_owned();
    let preview = preview_result(
        &mut fixture,
        728,
        &revision,
        serde_json::json!([{ "kind": "RemoveTag", "tag": "basilisk" }]),
    )
    .await?;
    let preview_id = preview_id_of(&preview)?;
    let apply_response = apply_preview(&mut fixture, 729, &preview_id).await?;
    let applied = apply_response
        .get("result")
        .filter(|result| !result.is_null())
        .ok_or_else(|| format!("apply returned no result: {apply_response}"))?;
    let annotation =
        rule_state(applied, "BSK-0001").ok_or("BSK-0001 missing from final snapshot")?;
    assert_eq!(
        annotation.pointer("/effectiveSeverity/kind"),
        Some(&serde_json::json!("Disabled")),
        "with no entry and no tag entry an analyze rule is disabled: {applied}"
    );
    Ok(())
}

// Implements [CHKARCH-CONFIG-MODEL]: requesting `disabled` for a pep-tagged
// rule — directly or via a tag entry that would resolve one to disabled — is
// a request error. PEP rules are graded, never disabled.
#[tokio::test]
async fn pep_disable_mutations_are_request_errors() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let revision = snapshot_revision(&mut fixture, 735).await?;
    let root = root_uri(&fixture);

    for (id, mutations) in [
        (
            736_u64,
            serde_json::json!([{
                "kind": "SetRule",
                "code": "assignment_compatibility",
                "severity": { "kind": "Disabled" }
            }]),
        ),
        (
            737,
            serde_json::json!([{
                "kind": "SetTag",
                "tag": "pep",
                "severity": { "kind": "Disabled" }
            }]),
        ),
    ] {
        let response = request_answering_apply_edit(
            &mut fixture,
            id,
            PREVIEW,
            serde_json::json!({
                "rootUri": root,
                "baseRevision": revision,
                "mutations": mutations
            }),
        )
        .await?;
        assert_eq!(
            error_kind(&response),
            Some("pepRuleDisable"),
            "request {id} must reject the pep disable: {response}"
        );
    }

    // Grading a pep rule is legitimate — only `disabled` is invalid.
    let graded = preview_result(
        &mut fixture,
        738,
        &revision,
        serde_json::json!([{
            "kind": "SetRule",
            "code": "assignment_compatibility",
            "severity": { "kind": "Warning" }
        }]),
    )
    .await?;
    assert!(graded.get("previewId").is_some(), "{graded}");
    Ok(())
}

// Implements [CONFIGEDITOR-OPERATIONS]: previews validate emptiness, revision,
// and mutation contents before any state is touched.
#[tokio::test]
async fn preview_rejects_invalid_requests() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = file_uri(&fixture, "config_editor_invalid.py");
    fixture.did_open(&uri, "def f(x):\n    return x\n").await?;
    let _ = fixture.wait_for_diagnostics().await?;
    let revision = snapshot_revision(&mut fixture, 730).await?;
    let root = root_uri(&fixture);
    let disable_one = serde_json::json!([{
        "kind": "SetRule",
        "code": "BSK-0001",
        "severity": { "kind": "Disabled" }
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
                "mutations": disable_one
            }),
            "revisionConflict",
        ),
        (
            733,
            serde_json::json!({
                "rootUri": root,
                "baseRevision": revision,
                "mutations": [{
                    "kind": "SetRule",
                    "code": "NOT-A-RULE",
                    "severity": { "kind": "Warning" }
                }]
            }),
            "unknownRule",
        ),
        (
            734,
            serde_json::json!({
                "rootUri": root,
                "baseRevision": revision,
                "mutations": [{ "kind": "RemoveTag", "tag": "not-a-tag" }]
            }),
            "unknownTag",
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

// Implements [CONFIGEDITOR-OPERATIONS]: occurrences page deterministically
// through the all/codes/tags selectors and enforce the documented limit
// window.
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
    let selector = serde_json::json!({ "kind": "Codes", "codes": ["BSK-0001"] });

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
    assert_eq!(item.get("code"), Some(&serde_json::json!("BSK-0001")));
    assert!(item
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|item_uri| item_uri.ends_with("config_editor_occurrences.py")));
    assert_eq!(
        item.pointer("/severity/kind"),
        Some(&serde_json::json!("Error"))
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
