#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

#[tokio::test]
async fn test_ws_find_references() -> TestResult<()> {
    let uri = "file:///ws_refs.py";
    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

result: str = greet(\"world\")
";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        340,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 4 },
            "context": { "includeDeclaration": true }
        }),
    )
    .await?;

    // Should find at least 2 references (definition + usage)
    let count = resp.matches("ws_refs.py").count();
    assert!(
        count >= 2,
        "should find at least 2 references for 'greet' (found {count}): {resp}"
    );

    // Hardened: parse and verify exact reference count and structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let refs = parsed["result"]
        .as_array()
        .ok_or("references result should be an array")?;

    // Exactly 2 references: definition on line 0 + usage on line 3
    assert_eq!(
        refs.len(),
        2,
        "should find exactly 2 references for 'greet' (definition + usage), got {}: {resp}",
        refs.len()
    );

    // Hardened: verify each reference has a valid URI
    for reference in refs {
        let uri = reference["uri"].as_str().unwrap_or("");
        assert!(
            !uri.is_empty(),
            "each reference must have a non-empty URI: {resp}"
        );
        assert_eq!(
            uri, "file:///ws_refs.py",
            "each reference URI must match the opened file: {resp}"
        );
    }

    // Hardened: verify each reference has a valid range
    for reference in refs {
        let range = &reference["range"];
        assert!(!range.is_null(), "each reference must have a range: {resp}");
        assert!(
            range.get("start").is_some() && range.get("end").is_some(),
            "each reference range must have start and end: {resp}"
        );
        let start_line = range["start"]["line"].as_u64().unwrap_or(u64::MAX);
        let end_line = range["end"]["line"].as_u64().unwrap_or(0);
        assert!(
            start_line <= end_line,
            "reference range start line must be <= end line: {resp}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_find_references_unknown_symbol_returns_null() -> TestResult<()> {
    let uri = "file:///ws_edge_refs.py";
    let code = "x: int = 42\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        404,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 5, "character": 0 },
            "context": { "includeDeclaration": true }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    assert!(
        result.is_null() || result.as_array().is_some_and(Vec::is_empty),
        "find references on unknown symbol should return null or empty: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_find_references_class() -> TestResult<()> {
    let uri = "file:///ws_refs_class.py";
    let code = "\
class Dog:
    name: str

def adopt(pet: Dog) -> Dog:
    return pet
";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        954,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 6 },
            "context": { "includeDeclaration": true }
        }),
    )
    .await?;

    // "Dog" appears 3 times: class def + param annotation + return annotation.
    let count = resp.matches("ws_refs_class.py").count();
    assert!(
        count >= 3,
        "should find at least 3 references for 'Dog' (found {count}): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_find_references_include_declaration() -> TestResult<()> {
    let uri = "file:///ws_refs_decl.py";
    let code = "\
def helper(x: int) -> int:
    return x + 1

a: int = helper(10)
b: int = helper(20)
";

    let (_fixture, resp_with) = open_and_request(
        uri,
        code,
        1104,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 4 },
            "context": { "includeDeclaration": true }
        }),
    )
    .await?;

    // Should find at least 3: definition + 2 usages.
    let count_with = resp_with.matches("ws_refs_decl.py").count();
    assert!(
        count_with >= 3,
        "with includeDeclaration: true, should find at least 3 references (def + 2 usages), got {count_with}: {resp_with}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_find_references_word_boundary() -> TestResult<()> {
    let uri = "file:///ws_refs_boundary.py";
    // "greet" and "greeting" are different identifiers — searching for "greet"
    // should NOT match "greeting" due to word boundary checking.
    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def greeting(name: str) -> str:
    return f\"Hi, {name}!\"

a: str = greet(\"world\")
b: str = greeting(\"world\")
";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        1105,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 4 },
            "context": { "includeDeclaration": true }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let locations = parsed["result"].as_array().ok_or("expected result array")?;

    // "greet" appears on line 0 (def) and line 6 (usage) = 2 refs.
    // "greeting" on lines 3 and 7 should NOT be included.
    // So we expect exactly 2 references.
    assert_eq!(
        locations.len(),
        2,
        "should find exactly 2 references for 'greet' (not matching 'greeting'): {resp}"
    );

    // Verify none of the locations point to line 3 or line 7 (greeting lines).
    for loc in locations {
        let line = loc["range"]["start"]["line"].as_u64().unwrap_or(99);
        assert!(
            line != 3 && line != 7,
            "reference should NOT match 'greeting' on line {line}: {resp}"
        );
    }

    Ok(())
}
