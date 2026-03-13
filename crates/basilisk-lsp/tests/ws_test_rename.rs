#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

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
    assert!(
        !changes.is_null(),
        "changes must not be null: {resp}"
    );

    let file_edits = changes[uri].as_array();
    assert!(
        file_edits.is_some(),
        "file edits must not be null: {resp}"
    );

    let edits = file_edits.expect("already asserted");
    assert!(
        !edits.is_empty(),
        "edits must be non-empty: {resp}"
    );

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
    assert!(
        !changes.is_null(),
        "changes must not be null: {resp}"
    );

    let file_edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

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
