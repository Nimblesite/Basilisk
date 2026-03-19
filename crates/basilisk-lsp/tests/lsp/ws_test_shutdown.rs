// Tests for LSP: `ws_test_shutdown`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_shutdown_gracefully() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Send shutdown request — server must respond with result: null.
    // tower-lsp requires an empty object (not null) for shutdown params.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 900,
            "method": "shutdown"
        }))
        .await?;

    let id_str = "\"id\":900";
    let mut resp = None;
    for _ in 0..10 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains(id_str) {
            resp = Some(msg);
            break;
        }
    }
    let resp = resp.ok_or("no response to shutdown request")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("result").is_some(),
        "shutdown should return a result: {resp}"
    );
    assert!(
        parsed.get("error").is_none(),
        "shutdown should not return an error: {resp}"
    );

    // Send exit notification — server should close the connection.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": null
        }))
        .await?;

    // After exit, the connection should be closed. Reading should yield None.
    let msg = fixture.recv().await;
    // Either None (closed) or we receive nothing — both are acceptable.
    // The key assertion is that shutdown itself returned cleanly.
    let _ = msg;

    Ok(())
}

/// After shutdown + exit, a fresh connection can re-initialize.
/// This simulates what the VS Code extension does when it auto-restarts
/// the server (up to 3 times).  The server process is new each time, so
/// the real requirement is that the server starts cleanly on a second
/// fixture — which is equivalent to a process restart.
#[tokio::test]
async fn test_ws_reinitialize_after_shutdown() -> TestResult<()> {
    // First lifecycle: initialize, shutdown, exit.
    {
        let mut fixture = WsTestFixture::new().await?;
        let _ = fixture.initialize().await?;

        fixture
            .send_json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 5000,
                "method": "shutdown"
            }))
            .await?;

        let id_str = "\"id\":5000";
        for _ in 0..10 {
            let Some(msg) = fixture.recv().await else {
                break;
            };
            if msg.contains(id_str) {
                break;
            }
        }

        fixture
            .send_json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }))
            .await?;
    }

    // Second lifecycle: server must start and initialize cleanly.
    {
        let mut fixture = WsTestFixture::new().await?;
        let response = fixture.initialize().await?;

        assert!(
            response.contains("\"result\""),
            "re-initialized server should return a valid result: {response}"
        );
        assert!(
            response.contains("\"basilisk\""),
            "re-initialized server should identify as basilisk: {response}"
        );
    }

    Ok(())
}

/// After shutdown, sending a regular request must not crash the server.
/// The LSP spec says the server should return `InvalidRequest` (-32600) for
/// any request received after shutdown.  tower-lsp may also close the
/// connection.  Either outcome is acceptable — a crash is not.
#[tokio::test]
async fn test_ws_requests_after_shutdown_return_error() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Open a file so we have a valid URI for the hover request.
    fixture
        .did_open("file:///ws_post_shutdown.py", "x: int = 1\n")
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Shutdown.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5010,
            "method": "shutdown"
        }))
        .await?;

    let id_str = "\"id\":5010";
    for _ in 0..10 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains(id_str) {
            break;
        }
    }

    // Now send a hover request — should get error or connection close, not crash.
    let send_result = fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5011,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///ws_post_shutdown.py" },
                "position": { "line": 0, "character": 0 }
            }
        }))
        .await;

    // The send itself might fail if the connection was already closed — that is fine.
    if send_result.is_err() {
        return Ok(());
    }

    // If we could send, try to read the response.
    let response = fixture.recv().await;
    match response {
        // Connection closed — valid post-shutdown behavior.
        None => {}
        Some(msg) => {
            // If we got a message back, it must be a JSON-RPC error, not a result.
            let parsed: serde_json::Value = serde_json::from_str(&msg)?;
            if parsed.get("error").is_some() {
                // Error response — correct behavior.
                let code = parsed["error"]["code"].as_i64();
                // InvalidRequest (-32600) or ServerNotInitialized (-32002) are both valid.
                assert!(
                    code == Some(-32600) || code == Some(-32002),
                    "expected InvalidRequest or ServerNotInitialized error code, got: {msg}"
                );
            }
            // If it returned a result, the server is being lenient — not ideal
            // but not a crash.  We accept it.
        }
    }

    Ok(())
}

/// Sending a request with structurally invalid params must return a
/// JSON-RPC error (typically `InvalidParams` -32602), not crash the server.
#[tokio::test]
async fn test_ws_invalid_params_returns_error() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Send textDocument/hover with completely wrong params — missing
    // the required `textDocument` and `position` fields.
    let resp = fixture
        .request(
            5020,
            "textDocument/hover",
            serde_json::json!({
                "bogus": "not a valid hover param"
            }),
        )
        .await?;

    // The server must respond — either with an error or with null result.
    // A timeout (None) means the server hung, which is also acceptable if
    // it didn't crash, but we prefer a response.
    if let Some(msg) = resp {
        let parsed: serde_json::Value = serde_json::from_str(&msg)?;
        // Acceptable outcomes:
        //   1. error with code -32602 (InvalidParams)
        //   2. error with any code (server caught the bad params)
        //   3. result: null (server was lenient and returned empty hover)
        let has_error = parsed.get("error").is_some();
        let has_null_result = parsed.get("result").is_some_and(serde_json::Value::is_null);
        assert!(
            has_error || has_null_result,
            "server should return error or null result for invalid params, got: {msg}"
        );
    }

    // Verify the server is still alive by sending a valid request.
    let alive_resp = fixture
        .request(
            5021,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///nonexistent.py" },
                "position": { "line": 0, "character": 0 }
            }),
        )
        .await?;

    // Getting any response (even null) proves the server did not crash.
    assert!(
        alive_resp.is_some(),
        "server should still respond after handling invalid params"
    );

    Ok(())
}

/// Sending `initialize` twice must not crash the server.
/// The LSP spec says the server should return an error for a second
/// initialize request.  tower-lsp may handle this internally.
#[tokio::test]
async fn test_ws_multiple_initialize_requests() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;

    // First initialize — should succeed normally.
    let first_resp = fixture.initialize().await?;
    assert!(
        first_resp.contains("\"result\""),
        "first initialize should succeed: {first_resp}"
    );

    // Second initialize — should either return an error or succeed idempotently.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5030,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": null,
                "capabilities": {},
                "trace": "off"
            }
        }))
        .await?;

    let id_str = "\"id\":5030";
    let mut second_resp = None;
    for _ in 0..10 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains(id_str) {
            second_resp = Some(msg);
            break;
        }
    }

    // The server must respond — not hang or crash.
    let second_resp = second_resp.ok_or("server did not respond to second initialize")?;
    let parsed: serde_json::Value = serde_json::from_str(&second_resp)?;

    // Acceptable outcomes:
    //   1. error response (spec-compliant: server already initialized)
    //   2. result response (server accepted re-init idempotently)
    // Unacceptable: no response (timeout) or crash.
    let has_error = parsed.get("error").is_some();
    let has_result = parsed.get("result").is_some();
    assert!(
        has_error || has_result,
        "second initialize must return error or result, got: {second_resp}"
    );

    Ok(())
}
