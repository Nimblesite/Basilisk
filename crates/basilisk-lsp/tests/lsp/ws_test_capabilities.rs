//! Tests for [LSPARCH-TESTING], [LSPARCH-CMDREG] (server advertises every command via
//! `executeCommandProvider`), [LSPARCH-FEATURES-TYPEHIER] (typeHierarchyProvider).
//! See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDREG
// Tests for LSP: `ws_test_capabilities`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_initialize_advertises_all_phase2_capabilities() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(
        response.contains("\"definitionProvider\""),
        "should advertise definition: {response}"
    );
    assert!(
        response.contains("\"documentSymbolProvider\""),
        "should advertise document symbols: {response}"
    );
    assert!(
        response.contains("\"signatureHelpProvider\""),
        "should advertise signature help: {response}"
    );
    assert!(
        response.contains("\"referencesProvider\""),
        "should advertise references: {response}"
    );
    assert!(
        response.contains("\"renameProvider\""),
        "should advertise rename: {response}"
    );
    assert!(
        response.contains("\"inlayHintProvider\""),
        "should advertise inlay hints: {response}"
    );
    assert!(
        response.contains("\"semanticTokensProvider\""),
        "should advertise semantic tokens: {response}"
    );
    assert!(
        response.contains("\"documentFormattingProvider\""),
        "should advertise document formatting: {response}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_initialize_advertises_execute_command_provider() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(
        response.contains("executeCommandProvider"),
        "initialize response should advertise executeCommandProvider: {response}"
    );
    // The server is the single source of truth for commands.
    // ALL commands must be advertised — see LSP-ARCHITECTURE-SPEC.md § Command Registration Rule.
    for cmd in basilisk_common::commands::ALL {
        assert!(
            response.contains(cmd),
            "executeCommandProvider must advertise '{cmd}': {response}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_initialize_advertises_type_hierarchy_provider() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(
        response.contains("\"typeHierarchyProvider\""),
        "initialize response should advertise typeHierarchyProvider: {response}"
    );

    // Parse the full response and verify the capability value is `true`.
    let parsed: serde_json::Value = serde_json::from_str(&response)?;
    let caps = parsed
        .get("result")
        .and_then(|r| r.get("capabilities"))
        .ok_or("missing capabilities in initialize response")?;

    assert_eq!(
        caps.get("typeHierarchyProvider"),
        Some(&serde_json::Value::Bool(true)),
        "typeHierarchyProvider should be true: {response}"
    );

    Ok(())
}

// Tests [LSPARCH-RESOLVED-ENV] (server/resolved_env.rs, wired in server/init.rs):
// initialize surfaces the resolved python/uv/binary environment so editors can
// render what auto-detect found instead of a bare placeholder (GitHub #153).
#[tokio::test]
async fn test_ws_initialize_advertises_resolved_environment() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    let parsed: serde_json::Value = serde_json::from_str(&response)?;
    let env = parsed
        .pointer("/result/capabilities/experimental/basilisk/resolvedEnvironment")
        .ok_or("missing experimental.basilisk.resolvedEnvironment in initialize response")?;

    for slot in ["python", "uv", "binary"] {
        assert!(
            env.get(slot).is_some(),
            "resolvedEnvironment must carry a `{slot}` slot (object or null): {env}"
        );
    }

    // The server always knows its own binary — never null, absolute path,
    // version present (this is what fixes the blank Binary row of #153).
    let binary = env
        .get("binary")
        .and_then(serde_json::Value::as_object)
        .ok_or("resolvedEnvironment.binary must be an object")?;
    let path = binary
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or("binary.path must be a string")?;
    assert!(
        std::path::Path::new(path).is_absolute(),
        "binary.path must be the absolute path of the running server: {path}"
    );
    assert!(
        binary
            .get("version")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|version| !version.is_empty()),
        "binary.version must be populated: {binary:?}"
    );

    // Regression: resolvedEnvironment must MERGE into the experimental
    // capabilities, not replace them — configurationEditor
    // ([LSPARCH-CONFIG-EDITOR-PROTOCOL], server/init.rs build_capabilities)
    // gates the editor command in every client and once vanished when this
    // payload overwrote the whole `experimental` object.
    assert_eq!(
        parsed.pointer("/result/capabilities/experimental/basilisk/configurationEditor"),
        Some(&serde_json::Value::Bool(true)),
        "configurationEditor must survive alongside resolvedEnvironment: {response}"
    );
    Ok(())
}

// Tests [STUBRES-TYPESHED-WARN] and [LSPCFGED-TYPESHED-SERVICE-INFO]:
// source status is ordinary LSP metadata, never a Python diagnostic.
#[tokio::test]
async fn test_ws_initialize_advertises_typeshed_status() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    let parsed: serde_json::Value = serde_json::from_str(&response)?;
    let statuses = parsed
        .pointer("/result/capabilities/experimental/basilisk/typeshedStatuses")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing experimental.basilisk.typeshedStatuses in initialize response")?;
    let entry = statuses
        .first()
        .ok_or("missing root-keyed Typeshed status in initialize response")?;
    assert!(
        entry
            .get("rootUri")
            .is_some_and(serde_json::Value::is_string),
        "missing rootUri: {entry}"
    );
    let status = entry
        .get("status")
        .ok_or("missing typed Typeshed state in initialize response")?;

    // [LSPCFGED-TYPESHED]: resolution is a local read completed during
    // `initialize`, so the payload carries the TERMINAL generation — there is
    // no acquiring state for any client to render as a blocking overlay.
    assert_eq!(
        status
            .pointer("/lifecycle/kind")
            .and_then(serde_json::Value::as_str),
        Some("Ready"),
        "initialize must expose the terminal resolved generation: {status}"
    );
    assert!(
        status.get("activeSource").is_some(),
        "a Ready status names its active source: {status}"
    );
    assert!(status
        .get("warnings")
        .is_some_and(serde_json::Value::is_array));
    assert!(
        parsed
            .pointer("/result/capabilities/experimental/basilisk/configurationEditor")
            .is_some(),
        "typeshedStatuses must merge with existing experimental data: {response}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_initialize_advertises_declaration_provider() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(
        response.contains("\"declarationProvider\""),
        "initialize response should advertise declarationProvider: {response}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_initialize_advertises_type_definition_provider() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(
        response.contains("\"typeDefinitionProvider\""),
        "initialize response should advertise typeDefinitionProvider: {response}"
    );
    Ok(())
}
