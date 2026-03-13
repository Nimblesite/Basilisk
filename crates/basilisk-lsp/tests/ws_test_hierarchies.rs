#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

#[tokio::test]
async fn test_ws_call_hierarchy() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let source = "def foo():\n    pass\n\ndef bar():\n    foo()\n    foo()\n";
    fixture
        .did_open("file:///ws_call_hierarchy.py", source)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // prepareCallHierarchy at the position of `foo` (line 0, character 4)
    let prepare_resp = fixture
        .request(
            200,
            "textDocument/prepareCallHierarchy",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_call_hierarchy.py" },
                "position": { "line": 0, "character": 4 }
            }),
        )
        .await?
        .ok_or("no prepareCallHierarchy response")?;

    assert!(
        prepare_resp.contains("\"name\":\"foo\""),
        "prepareCallHierarchy should return item named 'foo': {prepare_resp}"
    );

    // callHierarchy/incomingCalls for foo
    let incoming_resp = fixture
        .request(
            201,
            "callHierarchy/incomingCalls",
            serde_json::json!({
                "item": {
                    "name": "foo",
                    "kind": 12,
                    "uri": "file:///ws_call_hierarchy.py",
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 3 }
                    },
                    "selectionRange": {
                        "start": { "line": 0, "character": 4 },
                        "end": { "line": 0, "character": 7 }
                    }
                }
            }),
        )
        .await?
        .ok_or("no incomingCalls response")?;

    assert!(
        incoming_resp.contains("\"name\":\"bar\""),
        "incomingCalls should show 'bar' as a caller of 'foo': {incoming_resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_type_hierarchy() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let source = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str

class Puppy(Dog):
    age: int
";
    fixture
        .did_open("file:///ws_type_hierarchy.py", source)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // prepareTypeHierarchy on `Dog` (line 3, character 6 — inside the class name)
    let prepare_resp = fixture
        .request(
            300,
            "textDocument/prepareTypeHierarchy",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_type_hierarchy.py" },
                "position": { "line": 3, "character": 6 }
            }),
        )
        .await?
        .ok_or("no prepareTypeHierarchy response")?;

    assert!(
        prepare_resp.contains("\"name\":\"Dog\""),
        "prepareTypeHierarchy should return item named 'Dog': {prepare_resp}"
    );

    // typeHierarchy/supertypes for Dog -> should include Animal
    let supertypes_resp = fixture
        .request(
            301,
            "typeHierarchy/supertypes",
            serde_json::json!({
                "item": {
                    "name": "Dog",
                    "kind": 5,
                    "uri": "file:///ws_type_hierarchy.py",
                    "range": {
                        "start": { "line": 3, "character": 0 },
                        "end": { "line": 3, "character": 5 }
                    },
                    "selectionRange": {
                        "start": { "line": 3, "character": 6 },
                        "end": { "line": 3, "character": 9 }
                    },
                    "data": "Dog"
                }
            }),
        )
        .await?
        .ok_or("no supertypes response")?;

    assert!(
        supertypes_resp.contains("\"name\":\"Animal\""),
        "supertypes of Dog should include Animal: {supertypes_resp}"
    );

    // typeHierarchy/subtypes for Dog -> should include Puppy
    let subtypes_resp = fixture
        .request(
            302,
            "typeHierarchy/subtypes",
            serde_json::json!({
                "item": {
                    "name": "Dog",
                    "kind": 5,
                    "uri": "file:///ws_type_hierarchy.py",
                    "range": {
                        "start": { "line": 3, "character": 0 },
                        "end": { "line": 3, "character": 5 }
                    },
                    "selectionRange": {
                        "start": { "line": 3, "character": 6 },
                        "end": { "line": 3, "character": 9 }
                    },
                    "data": "Dog"
                }
            }),
        )
        .await?
        .ok_or("no subtypes response")?;

    assert!(
        subtypes_resp.contains("\"name\":\"Puppy\""),
        "subtypes of Dog should include Puppy: {subtypes_resp}"
    );

    Ok(())
}
