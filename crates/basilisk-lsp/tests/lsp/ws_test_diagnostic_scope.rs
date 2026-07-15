//! Tests for [LSPARCH-DIAGNOSTIC-SCOPE] and [LSPARCH-CONFIG-SEEDING].
//! See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-DIAGNOSTIC-SCOPE.
//!
//! End-to-end coverage of the diagnostic-scope union, the
//! `basilisk.analyze = false` pep-only opt-out, and the one-time
//! strict-by-default configuration seed over the real WebSocket server.

use super::ws_test_common::*;

/// A source that fires one check-scope (pep) diagnostic
/// (`returns_compatibility`) AND one analyze-scope diagnostic (`BSK-0001`,
/// enabled by the fixture config).
const MIXED_SCOPE_SOURCE: &str = "def count(x) -> str:\n    return 42\n";

/// Initialize the fixture's workspace root with explicit
/// `initializationOptions`.
async fn initialize_with_options(
    fixture: &mut WsTestFixture,
    options: serde_json::Value,
) -> TestResult<()> {
    let root_uri = format!("file://{}", fixture.workspace_root.to_string_lossy());
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {},
                "trace": "off",
                "initializationOptions": options
            }
        }))
        .await?;
    let _ = fixture.recv().await.ok_or("no response to initialize")?;
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }))
        .await?;
    Ok(())
}

/// Codes carried by the first `publishDiagnostics` for `uri`.
async fn published_codes(fixture: &mut WsTestFixture, uri: &str) -> TestResult<Vec<String>> {
    for _ in 0..10 {
        let msg = fixture
            .wait_for_diagnostics()
            .await
            .map_err(|error| format!("no diagnostics for {uri}: {error}"))?;
        let parsed: serde_json::Value = serde_json::from_str(&msg)?;
        if parsed.pointer("/params/uri").and_then(serde_json::Value::as_str) != Some(uri) {
            continue;
        }
        let codes = parsed
            .pointer("/params/diagnostics")
            .and_then(serde_json::Value::as_array)
            .map(|diagnostics| {
                diagnostics
                    .iter()
                    .filter_map(|diagnostic| diagnostic.get("code"))
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        return Ok(codes);
    }
    Err(format!("no publishDiagnostics arrived for {uri}").into())
}

// Implements [LSPARCH-DIAGNOSTIC-SCOPE]: by default the LSP publishes the
// UNION of both command scopes — every pep rule plus every configured
// analyze rule — through one diagnostics stream.
#[tokio::test]
async fn default_scope_publishes_the_union_of_both_scopes() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = format!(
        "file://{}/scope_union.py",
        fixture.workspace_root.to_string_lossy()
    );
    fixture.did_open(&uri, MIXED_SCOPE_SOURCE).await?;

    let codes = published_codes(&mut fixture, &uri).await?;
    assert!(
        codes.iter().any(|code| code == "returns_compatibility"),
        "check-scope (pep) diagnostics must publish: {codes:?}"
    );
    assert!(
        codes.iter().any(|code| code == "BSK-0001"),
        "configured analyze-scope diagnostics must publish in the union: {codes:?}"
    );
    Ok(())
}

// Implements [LSPARCH-DIAGNOSTIC-SCOPE]: `basilisk.analyze = false`
// (initializationOptions) restricts publication to check scope — the edge
// filter is `is_pep_rule`; project configuration never selects scope.
#[tokio::test]
async fn analyze_opt_out_filters_published_diagnostics_to_pep_only() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    initialize_with_options(
        &mut fixture,
        serde_json::json!({ "basilisk": { "analyze": false } }),
    )
    .await?;
    let uri = format!(
        "file://{}/scope_pep_only.py",
        fixture.workspace_root.to_string_lossy()
    );
    fixture.did_open(&uri, MIXED_SCOPE_SOURCE).await?;

    let codes = published_codes(&mut fixture, &uri).await?;
    assert!(
        codes.iter().any(|code| code == "returns_compatibility"),
        "check-scope diagnostics must still publish: {codes:?}"
    );
    assert!(
        !codes.iter().any(|code| code == "BSK-0001"),
        "analyze-scope diagnostics must be filtered out by the opt-out: {codes:?}"
    );
    Ok(())
}

// Implements [LSPARCH-CONFIG-SEEDING]: opening a workspace root whose
// ancestor walk finds no [tool.basilisk] table writes the two-line
// strict-by-default seed into the root's pyproject.toml before first
// analysis — exactly once, never resurrecting a deleted entry.
#[tokio::test]
async fn initialization_seeds_an_unconfigured_root_exactly_once() -> TestResult<()> {
    let root = unique_temp_dir("bsk_seed_e2e");
    std::fs::create_dir_all(&root)?;
    let root_uri = format!("file://{}", root.to_string_lossy());

    // First initialization: the bare root gets the seed.
    {
        let mut fixture = WsTestFixture::new().await?;
        let _ = initialize_with_root(&mut fixture, &root_uri, "openFilesOnly").await?;
        let seeded = std::fs::read_to_string(root.join("pyproject.toml"))
            .map_err(|error| format!("seed file must exist after initialize: {error}"))?;
        assert!(
            seeded.contains("[tool.basilisk.rule-tags]"),
            "the seed writes the rule-tags table: {seeded}"
        );
        assert!(
            seeded.contains("basilisk") && seeded.contains("error"),
            "the seed is the one strict-by-default tag entry: {seeded}"
        );
    }

    // The user deletes the entry but the (never-pruned) table remains; a
    // second initialization must not resurrect it.
    std::fs::write(root.join("pyproject.toml"), "[tool.basilisk.rule-tags]\n")?;
    {
        let mut fixture = WsTestFixture::new().await?;
        let _ = initialize_with_root(&mut fixture, &root_uri, "openFilesOnly").await?;
        let content = std::fs::read_to_string(root.join("pyproject.toml"))?;
        assert_eq!(
            content, "[tool.basilisk.rule-tags]\n",
            "an existing table must block re-seeding"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

// Implements [LSPARCH-CONFIG-SEEDING]: the seed turns the analyze-scope house
// rules on at error out of the box, so a bare project immediately gets
// strict-by-default diagnostics.
#[tokio::test]
async fn seeded_root_publishes_analyze_diagnostics_out_of_the_box() -> TestResult<()> {
    let root = unique_temp_dir("bsk_seed_diag");
    std::fs::create_dir_all(&root)?;
    let root_uri = format!("file://{}", root.to_string_lossy());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "openFilesOnly").await?;
    let uri = format!("{root_uri}/seeded.py");
    fixture.did_open(&uri, "def f(x):\n    return x\n").await?;

    let codes = published_codes(&mut fixture, &uri).await?;
    assert!(
        codes.iter().any(|code| code == "BSK-0001"),
        "the seed's basilisk=error tag entry must enable the house rules: {codes:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}
