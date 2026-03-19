// Tests for LSP: `ws_test_find_references`.

use super::ws_test_common::*;

use std::time::Duration;

use futures_util::StreamExt;

use std::time::Duration;

use futures_util::StreamExt;

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

#[tokio::test]
async fn test_ws_find_references_cross_file() -> TestResult<()> {
    // Set up workspace: helpers.py defines `greet`, main.py imports and uses it.
    let dir = unique_temp_dir("bsk_refs_cross_file");
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

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    // Drain startup messages.
    for _ in 0..20 {
        let msg = tokio::time::timeout(Duration::from_millis(500), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }

    // Find references for `greet` at its definition in helpers.py (line 0, char 4).
    let resp = fixture
        .request(
            600,
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": helpers_uri },
                "position": { "line": 0, "character": 4 },
                "context": { "includeDeclaration": true }
            }),
        )
        .await?
        .ok_or("no response to cross-file find references")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let refs = parsed["result"]
        .as_array()
        .ok_or("references result should be an array")?;

    // Should find references in BOTH helpers.py and main.py.
    let helpers_count = refs
        .iter()
        .filter(|r| r["uri"].as_str().unwrap_or("").contains("helpers.py"))
        .count();
    let main_count = refs
        .iter()
        .filter(|r| r["uri"].as_str().unwrap_or("").contains("main.py"))
        .count();

    assert!(
        helpers_count >= 1,
        "should find at least 1 reference in helpers.py (definition), got {helpers_count}: {resp}"
    );
    assert!(
        main_count >= 1,
        "should find at least 1 reference in main.py (usage), got {main_count}: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
