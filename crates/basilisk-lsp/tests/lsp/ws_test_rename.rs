//! Tests for [LSPARCH-FEATURES-RENAME]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-RENAME
// Tests for LSP: `ws_test_rename`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_prepare_rename() -> TestResult<()> {
    let uri = "file:///rename_prepare.py";
    let code = "def greet(name: str) -> str:\n    return f'Hello, {name}!'\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        400,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 4 }
        }),
    )
    .await?;

    assert!(
        resp.contains("result"),
        "prepareRename response should contain result: {resp}"
    );

    Ok(())
}

/// # Panics
/// Panics if `file_edits` is `None` (the response did not contain edits for the URI).
#[tokio::test]
async fn test_ws_rename_symbol() -> TestResult<()> {
    let uri = "file:///rename_symbol.py";
    let code = "def greet(name: str) -> str:\n    return f'Hello, {name}!'\n\nresult: str = greet('world')\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        401,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 4 },
            "newName": "say_hello"
        }),
    )
    .await?;

    assert!(
        resp.contains("say_hello"),
        "rename response should contain say_hello: {resp}"
    );
    assert!(
        resp.contains("changes"),
        "rename response should contain changes: {resp}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let file_edits = changes[uri].as_array();
    assert!(file_edits.is_some(), "file edits must not be null: {resp}");

    let Some(edits) = file_edits else {
        panic!("file edits must not be null: {resp}");
    };
    assert!(!edits.is_empty(), "edits must be non-empty: {resp}");

    for edit in edits {
        assert!(
            !edit["range"].is_null(),
            "each edit must have a range: {edit}"
        );
        assert_eq!(
            edit["newText"].as_str(),
            Some("say_hello"),
            "each edit newText must be say_hello: {edit}"
        );
    }

    assert!(
        edits.len() >= 2,
        "should have at least 2 edits (def + call site), got {}: {resp}",
        edits.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_rename_non_symbol_position_returns_null() -> TestResult<()> {
    let uri = "file:///rename_null.py";
    let code = "x: int = 1\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        402,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 100, "character": 0 },
            "newName": "whatever"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "rename at empty position should return null result: {resp}"
    );

    Ok(())
}

/// # Panics
/// Panics if `changes[uri]` is not an array (the response did not contain edits for the URI).
#[tokio::test]
async fn test_ws_rename_multiple_occurrences() -> TestResult<()> {
    let uri = "file:///rename_multi.py";
    let code = "def helper(x: int) -> int:\n    return x + 1\n\na: int = helper(1)\nb: int = helper(2)\nc: int = helper(3)\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        403,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 4 },
            "newName": "assist"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let Some(file_edits) = changes[uri].as_array() else {
        panic!("file edits must be an array: {resp}");
    };

    assert!(
        file_edits.len() >= 4,
        "should have at least 4 edits (1 def + 3 calls), got {}: {resp}",
        file_edits.len()
    );

    for edit in file_edits {
        assert_eq!(
            edit["newText"].as_str(),
            Some("assist"),
            "all edits newText must be assist: {edit}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_rename_cross_file() -> TestResult<()> {
    // Set up workspace: helpers.py defines `greet`, main.py imports and uses it.
    let dir = unique_temp_dir("bsk_rename_cross_file");
    std::fs::create_dir_all(&dir)?;

    std::fs::write(
        dir.join("helpers.py"),
        "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n",
    )?;
    std::fs::write(
        dir.join("main.py"),
        "from helpers import greet\n\nresult: str = greet(\"world\")\n",
    )?;

    let root_uri = format!("file://{}", dir.display());
    let helpers_uri = format!("file://{}", dir.join("helpers.py").display());
    let main_uri = format!("file://{}", dir.join("main.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    // Drain startup messages.
    for _ in 0..20 {
        let msg = tokio::time::timeout(Duration::from_millis(500), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }

    // Rename `greet` at its definition in helpers.py (line 0, char 4) to `say_hello`.
    let resp = fixture
        .request(
            700,
            "textDocument/rename",
            serde_json::json!({
                "textDocument": { "uri": helpers_uri },
                "position": { "line": 0, "character": 4 },
                "newName": "say_hello"
            }),
        )
        .await?
        .ok_or("no response to cross-file rename")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    // Should have edits in helpers.py (definition site).
    let helpers_edits = changes[&helpers_uri]
        .as_array()
        .ok_or("expected edits for helpers.py")?;
    assert!(
        !helpers_edits.is_empty(),
        "should have edits in helpers.py: {resp}"
    );
    for edit in helpers_edits {
        assert_eq!(
            edit["newText"].as_str(),
            Some("say_hello"),
            "helpers.py edit newText must be say_hello: {edit}"
        );
    }

    // Should have edits in main.py (import + usage sites).
    let main_edits = changes[&main_uri]
        .as_array()
        .ok_or("expected edits for main.py")?;
    assert!(
        !main_edits.is_empty(),
        "should have edits in main.py: {resp}"
    );
    for edit in main_edits {
        assert_eq!(
            edit["newText"].as_str(),
            Some("say_hello"),
            "main.py edit newText must be say_hello: {edit}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
