// Tests for LSP: `ws_test_workspace_symbols`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_workspace_symbols() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Open two documents with distinct symbols.
    let doc1 = "class Greeter:\n    name: str\n\ndef greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    let doc2 = "class Calculator:\n    value: int\n\ndef compute(x: int, y: int) -> int:\n    return x + y";

    fixture.did_open("file:///ws_sym_a.py", doc1).await?;
    let _ = fixture.wait_for_diagnostics().await;

    fixture.did_open("file:///ws_sym_b.py", doc2).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Query all symbols — empty string returns everything.
    let resp_all = fixture
        .request(500, "workspace/symbol", serde_json::json!({ "query": "" }))
        .await?
        .ok_or("no workspace/symbol response for empty query")?;

    assert!(
        resp_all.contains("\"result\""),
        "expected result in response: {resp_all}"
    );
    // Both documents' classes should appear.
    assert!(
        resp_all.contains("\"Greeter\""),
        "expected Greeter in workspace symbols: {resp_all}"
    );
    assert!(
        resp_all.contains("\"Calculator\""),
        "expected Calculator in workspace symbols: {resp_all}"
    );
    // Functions from both documents should appear.
    assert!(
        resp_all.contains("\"greet\""),
        "expected greet in workspace symbols: {resp_all}"
    );
    assert!(
        resp_all.contains("\"compute\""),
        "expected compute in workspace symbols: {resp_all}"
    );

    // Query filtered — only symbols matching "calc" (case-insensitive).
    let resp_filtered = fixture
        .request(
            501,
            "workspace/symbol",
            serde_json::json!({ "query": "calc" }),
        )
        .await?
        .ok_or("no workspace/symbol response for filtered query")?;

    assert!(
        resp_filtered.contains("\"Calculator\""),
        "expected Calculator with query 'calc': {resp_filtered}"
    );
    assert!(
        !resp_filtered.contains("\"Greeter\""),
        "Greeter should be filtered out with query 'calc': {resp_filtered}"
    );
    assert!(
        !resp_filtered.contains("\"greet\""),
        "greet should be filtered out with query 'calc': {resp_filtered}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_workspace_symbols_empty_query() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Open a document with known symbols.
    let code = "class Dog:\n    breed: str\n\ndef bark(volume: int) -> str:\n    return \"woof\"";
    fixture.did_open("file:///ws_sym_empty.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Empty query should return all symbols from all open documents.
    let resp = fixture
        .request(700, "workspace/symbol", serde_json::json!({ "query": "" }))
        .await?
        .ok_or("no workspace/symbol response for empty query")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let symbols = result
        .as_array()
        .ok_or("workspace/symbol result should be an array")?;

    // Empty query should still return symbols — not an empty array.
    assert!(
        !symbols.is_empty(),
        "empty query should still return symbols: {resp}"
    );

    // We opened a file with Dog class and bark function — both should appear.
    assert!(
        resp.contains("\"Dog\""),
        "expected Dog class in workspace symbols with empty query: {resp}"
    );
    assert!(
        resp.contains("\"bark\""),
        "expected bark function in workspace symbols with empty query: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_workspace_symbols_no_match() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Open a document with known symbols.
    let code = "class Apple:\n    color: str\n\ndef eat(fruit: str) -> str:\n    return fruit";
    fixture.did_open("file:///ws_sym_nomatch.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Query for something that matches nothing in the document.
    let resp = fixture
        .request(
            701,
            "workspace/symbol",
            serde_json::json!({ "query": "zzzznonexistent" }),
        )
        .await?
        .ok_or("no workspace/symbol response for non-matching query")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    // No symbols should match a nonsense query.
    // Valid LSP responses: null (no results) or an empty array.
    if result.is_null() {
        // null means no results — this is valid.
    } else {
        let symbols = result
            .as_array()
            .ok_or("workspace/symbol result should be null or an array")?;
        assert!(
            symbols.is_empty(),
            "query with no matching symbols should return empty array or null, got {} symbols: {resp}",
            symbols.len()
        );
    }

    Ok(())
}
