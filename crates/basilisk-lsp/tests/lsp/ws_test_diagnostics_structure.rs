//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Tests for LSP: `ws_test_diagnostics_structure`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_diagnostic_structure_is_well_formed() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "def f(x, y):\n    return x + y\n";
    fixture.did_open("file:///structure.py", code).await?;
    let raw = fixture.wait_for_diagnostics().await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;

    // Verify outer JSON-RPC envelope
    assert_eq!(json["jsonrpc"], "2.0", "must be JSON-RPC 2.0: {raw}");
    assert_eq!(
        json["method"], "textDocument/publishDiagnostics",
        "must be publishDiagnostics: {raw}"
    );
    assert!(
        json["params"]["uri"].as_str().is_some(),
        "must have uri in params: {raw}"
    );

    let diagnostics = json["params"]["diagnostics"]
        .as_array()
        .ok_or("diagnostics must be array")?;

    for diag in diagnostics {
        // code
        assert!(
            diag["code"].as_str().is_some_and(|c| c.starts_with("BSK-")),
            "code must start with BSK-: {diag}"
        );
        // message
        assert!(
            diag["message"].as_str().is_some_and(|m| !m.is_empty()),
            "message must be non-empty: {diag}"
        );
        // severity (1=Error, 2=Warning, 3=Info, 4=Hint)
        let severity = diag["severity"].as_u64().unwrap_or(0);
        assert!(
            (1..=4).contains(&severity),
            "severity must be 1-4, got {severity}: {diag}"
        );
        // range validity
        assert_valid_range(diag, "structure test");
        // codeDescription url
        let url = diag["codeDescription"]["href"].as_str().unwrap_or("");
        assert!(
            url.starts_with("https://"),
            "codeDescription.href must be a https URL: {diag}"
        );
    }
    Ok(())
}
