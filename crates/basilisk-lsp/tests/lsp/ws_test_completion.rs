//! Tests for [LSPARCH-FEATURES-COMPLETION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-COMPLETION
// Tests for LSP: `ws_test_completion`.

use super::ws_test_common::*;

// ── Completion (IntelliSense) tests via WebSocket ───────────────────────────

#[tokio::test]
async fn test_ws_initialize_advertises_completion() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(response.contains("\"completionProvider\""));
    assert!(response.contains("\".\""));
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_returns_functions_and_classes() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///comp.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            10,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///comp.py" },
                "position": { "line": 9, "character": 0 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"greet\""),
        "should complete function 'greet': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"Animal\""),
        "should complete class 'Animal': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"x\""),
        "should complete variable 'x': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"print\""),
        "should complete builtin 'print': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"len\""),
        "should complete builtin 'len': {resp}"
    );

    // Hardened: parse and verify completion list structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let items = parsed["result"]["items"]
        .as_array()
        .or_else(|| parsed["result"].as_array())
        .ok_or("completion result should contain items array")?;

    // Hardened: completion list must be non-empty
    assert!(
        !items.is_empty(),
        "completion list must be non-empty: {resp}"
    );

    // Hardened: each item must have a non-empty label and a kind field
    for item in items {
        let label = item["label"].as_str().unwrap_or("");
        assert!(
            !label.is_empty(),
            "each completion item must have a non-empty label: {resp}"
        );
        assert!(
            item.get("kind").is_some() && !item["kind"].is_null(),
            "each completion item must have a 'kind' field, missing for label '{label}': {resp}"
        );
    }

    // Hardened: verify JSON-RPC envelope
    assert_eq!(
        parsed["jsonrpc"], "2.0",
        "must be valid JSON-RPC 2.0: {resp}"
    );
    assert_eq!(
        parsed["id"], 10,
        "response id must match request id: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_prefix_filtering() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
def greet(name: str) -> str:
    return name

def goodbye(name: str) -> str:
    return name

def helper(x: int) -> int:
    return x

gr";
    fixture.did_open("file:///prefix.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            11,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///prefix.py" },
                "position": { "line": 9, "character": 2 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"greet\""),
        "should match 'greet' for prefix 'gr': {resp}"
    );
    assert!(
        !resp.contains("\"label\":\"helper\""),
        "should NOT match 'helper' for prefix 'gr': {resp}"
    );
    assert!(
        !resp.contains("\"label\":\"goodbye\""),
        "should NOT match 'goodbye' for prefix 'gr': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_imports() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
from typing import Optional, List
import os

";
    fixture.did_open("file:///imports.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            12,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///imports.py" },
                "position": { "line": 3, "character": 0 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"Optional\""),
        "should complete imported 'Optional': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"List\""),
        "should complete imported 'List': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"os\""),
        "should complete imported module 'os': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_dot_on_class() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
class Dog:
    name: str
    breed: str
    def bark(self) -> str:
        return \"woof\"
    def fetch(self, item: str) -> str:
        return item

Dog.";
    fixture.did_open("file:///dot.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            13,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///dot.py" },
                "position": { "line": 8, "character": 4 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"name\""),
        "should complete attribute 'name': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"breed\""),
        "should complete attribute 'breed': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"bark\""),
        "should complete method 'bark': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"fetch\""),
        "should complete method 'fetch': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_self_dot() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
class Cat:
    color: str
    age: int
    def meow(self) -> str:
        return \"meow\"
    def describe(self) -> str:
        return self.";
    fixture.did_open("file:///selfdot.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            14,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///selfdot.py" },
                "position": { "line": 6, "character": 20 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"color\""),
        "should complete self.color: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"age\""),
        "should complete self.age: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"meow\""),
        "should complete self.meow: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"describe\""),
        "should complete self.describe: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_builtins() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "pri";
    fixture.did_open("file:///builtins.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            15,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///builtins.py" },
                "position": { "line": 0, "character": 3 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"print\""),
        "should complete builtin 'print' for prefix 'pri': {resp}"
    );
    assert!(
        !resp.contains("\"label\":\"len\""),
        "should NOT include 'len' for prefix 'pri': {resp}"
    );
    assert!(
        !resp.contains("\"label\":\"map\""),
        "should NOT include 'map' for prefix 'pri': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_function_detail_shows_params() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
def calculate(x: int, y: int, op: str) -> int:
    return x

cal";
    fixture.did_open("file:///detail.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            16,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///detail.py" },
                "position": { "line": 3, "character": 3 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"calculate\""),
        "should complete 'calculate': {resp}"
    );
    assert!(
        resp.contains("x, y, op"),
        "should show params in detail: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_on_empty_file() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    fixture.did_open("file:///empty.py", "").await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            17,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///empty.py" },
                "position": { "line": 0, "character": 0 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"print\""),
        "empty file should still offer builtins: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"int\""),
        "empty file should still offer 'int': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"str\""),
        "empty file should still offer 'str': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"True\""),
        "empty file should still offer 'True': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"Exception\""),
        "empty file should still offer 'Exception': {resp}"
    );
    Ok(())
}
