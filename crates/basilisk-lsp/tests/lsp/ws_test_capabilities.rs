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
