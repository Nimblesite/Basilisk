//! Behavioral pins for [STUBRES-TYPESHED-WARN]: a root whose typeshed pin is
//! not on this machine runs NO analysis, so BOTH activation seams — the
//! initialize path (`resolve_typeshed_for_roots`) and the shared
//! `StagedResolution::publish` seam behind config edits and post-download
//! re-resolution — must drive an unmissable `window/showMessage` ERROR.
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN and
//! the call-site guard in `src/server/typeshed_status.rs`
//! (`every_no_source_activation_seam_raises_an_immediate_error`).

use super::ws_test_common::*;
use super::ws_test_configuration_editor::root_uri;

/// A full 40-hex pin that no machine's store can hold — resolution must land
/// on a terminal `NoSource` generation ([STUBRES-TYPESHED-PIN]).
const MISSING_PIN: &str = "0123456789012345678901234567890123456789";

/// The pyproject content that pins typeshed to [`MISSING_PIN`] against an
/// empty private store, so the test never reads the developer's real store.
fn missing_pin_pyproject(store: &std::path::Path) -> String {
    format!(
        "[tool.basilisk]\ntypeshed-commit = \"{MISSING_PIN}\"\ntypeshed-store-path = \"{}\"\n",
        store.to_string_lossy()
    )
}

/// Pump the connection until a `window/showMessage` ERROR toast carrying every
/// needle arrives; `None` when the message budget or timeout runs out.
async fn error_toast_containing(
    fixture: &mut WsTestFixture,
    needles: &[&str],
    budget: usize,
) -> Option<String> {
    for _ in 0..budget {
        let msg = fixture.recv().await?;
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg) else {
            continue;
        };
        if parsed["method"].as_str() != Some("window/showMessage")
            || parsed["params"]["type"].as_u64() != Some(1)
        {
            continue;
        }
        let text = parsed["params"]["message"].as_str().unwrap_or_default();
        if needles.iter().all(|needle| text.contains(needle)) {
            return Some(text.to_owned());
        }
    }
    None
}

/// [STUBRES-TYPESHED-WARN]: a workspace that initializes onto a pin absent
/// from this machine runs NO analysis, so `initialize` itself must drive an
/// unmissable `window/showMessage` ERROR — a trace-log line or a status
/// payload only bespoke clients render would let the broken deployment
/// masquerade as a clean workspace (the CODE RED failure).
#[tokio::test]
async fn missing_pin_at_initialize_raises_an_error_toast() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let store = fixture.workspace_root.join("empty_typeshed_store");
    std::fs::create_dir_all(&store)?;
    std::fs::write(
        fixture.workspace_root.join("pyproject.toml"),
        missing_pin_pyproject(&store),
    )?;

    // Manual handshake: the shared `initialize()` helper asserts a Ready
    // payload, and the ERROR toast may be emitted before the response.
    let root = root_uri(&fixture);
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": root,
                "capabilities": {},
                "trace": "off"
            }
        }))
        .await?;
    let mut initialize_payload = None;
    let mut toast = None;
    for _ in 0..30 {
        if initialize_payload.is_some() && toast.is_some() {
            break;
        }
        let Some(msg) = fixture.recv().await else {
            break;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg) else {
            continue;
        };
        if parsed["id"].as_u64() == Some(1) && parsed.get("result").is_some() {
            initialize_payload = Some(msg);
            fixture
                .send_json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "initialized",
                    "params": {}
                }))
                .await?;
            continue;
        }
        if parsed["method"].as_str() == Some("window/showMessage")
            && parsed["params"]["type"].as_u64() == Some(1)
            && parsed["params"]["message"]
                .as_str()
                .is_some_and(|text| text.contains("type checking is disabled"))
        {
            toast = Some(msg);
        }
    }
    let payload = initialize_payload.ok_or("no response to initialize")?;
    assert!(
        payload.contains("\"lifecycle\":{\"kind\":\"NoSource\"}"),
        "initialize payload must carry the terminal NoSource status: {payload}"
    );
    let toast = toast.ok_or(
        "no window/showMessage ERROR arrived for the missing pin — a NoSource \
         initialize is silent again (CODE RED regression)",
    )?;
    assert!(
        toast.contains(MISSING_PIN),
        "the toast must name the unresolvable pin: {toast}"
    );
    Ok(())
}

/// [STUBRES-TYPESHED-WARN]: editing the on-disk configuration to a pin absent
/// from this machine — through NO LSP channel — must raise the same ERROR
/// toast via the shared `StagedResolution::publish` seam. Guards the gap where
/// only the initialize path was wired and every later `NoSource` stayed silent.
#[tokio::test]
async fn missing_pin_disk_edit_raises_an_error_toast() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Open a file and await its diagnostics before editing: this gives the
    // server-owned watcher time to seed its baseline from the ORIGINAL
    // pyproject, so the pin edit below is unambiguously a content change and
    // not swallowed by a seed-vs-edit race. Mirrors the reliable pattern in
    // `disk_config_edit_without_client_watcher_republishes_and_notifies`.
    let uri = format!(
        "file://{}/no_source_edit_app.py",
        fixture.workspace_root.to_string_lossy()
    );
    fixture
        .did_open(&uri, "def handler(value):\n    return value\n")
        .await?;
    let _ = fixture.wait_for_diagnostics().await?;

    let store = fixture.workspace_root.join("empty_typeshed_store");
    std::fs::create_dir_all(&store)?;
    std::fs::write(
        fixture.workspace_root.join("pyproject.toml"),
        missing_pin_pyproject(&store),
    )?;

    let toast = error_toast_containing(
        &mut fixture,
        &["type checking is disabled", MISSING_PIN],
        40,
    )
    .await
    .ok_or(
        "no window/showMessage ERROR arrived after the on-disk pin edit — the \
         publish seam is silent for NoSource generations",
    )?;
    assert!(
        toast.contains("basilisk typeshed download"),
        "the toast must give the remedy: {toast}"
    );
    Ok(())
}
