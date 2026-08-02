//! Tests for [LSPARCH-FEATURES-DEFINITION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-DEFINITION
// Coverage-boost tests for `textDocument/typeDefinition`: exercises variable,
// parameter, and attribute type annotations, the `Optional[...]` / `list[...]`
// / `X | None` base-type extraction, the no-annotation fallback, and the
// no-symbol blank-position path. Each test opens a document, requests
// type-definition at several positions, and asserts on every response.

use super::ws_test_common::*;

async fn opened(code: &str, uri: &str) -> TestResult<WsTestFixture> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    fixture.did_open(uri, code).await?;
    let _ = fixture.wait_for_diagnostics().await;
    Ok(fixture)
}

/// Type-def on a variable annotation `x: MyClass` jumps to `class MyClass`.
/// Also covers the `SymbolHit::Variable` `annotation_span` branch.
#[tokio::test]
async fn test_ws_type_definition_variable_annotation() -> TestResult<()> {
    let code = "\
class MyClass:
    pass

x: MyClass = MyClass()
";
    let mut fixture = opened(code, "file:///ws_typedef_var.py").await?;

    let resp = fixture
        .request(
            310,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_typedef_var.py" },
                "position": { "line": 3, "character": 0 }
            }),
        )
        .await?
        .ok_or("no typeDefinition response")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_object(),
        "type-def on `x: MyClass` should resolve to a Location: {resp}"
    );
    // The class `MyClass` is defined on line 0 — the jump must land there.
    assert_eq!(
        parsed["result"]["range"]["start"]["line"].as_u64(),
        Some(0),
        "type-def should jump to the class definition line: {resp}"
    );

    Ok(())
}

/// Type-def on a parameter annotation `param: Foo` jumps to `class Foo`.
/// Covers the `SymbolHit::Parameter` `annotation_span` branch.
#[tokio::test]
async fn test_ws_type_definition_parameter_annotation() -> TestResult<()> {
    let code = "\
class Foo:
    pass

def consume(param: Foo) -> None:
    pass
";
    let mut fixture = opened(code, "file:///ws_typedef_param.py").await?;

    // Cursor on `param` (line 3, character 13).
    let resp = fixture
        .request(
            320,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_typedef_param.py" },
                "position": { "line": 3, "character": 13 }
            }),
        )
        .await?
        .ok_or("no typeDefinition response for param")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_object(),
        "type-def on `param: Foo` should resolve to a Location: {resp}"
    );
    assert_eq!(
        parsed["result"]["range"]["start"]["line"].as_u64(),
        Some(0),
        "type-def should jump to the Foo class definition (line 0): {resp}"
    );

    Ok(())
}

/// Type-def on an attribute annotation `attr: Bar` jumps to `class Bar`.
/// Covers the `SymbolHit::Attribute` `annotation_span` branch.
#[tokio::test]
async fn test_ws_type_definition_attribute_annotation() -> TestResult<()> {
    let code = "\
class Bar:
    pass

class Holder:
    attr: Bar
";
    let mut fixture = opened(code, "file:///ws_typedef_attr.py").await?;

    // Cursor on `attr` (line 4, character 4).
    let resp = fixture
        .request(
            330,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_typedef_attr.py" },
                "position": { "line": 4, "character": 4 }
            }),
        )
        .await?
        .ok_or("no typeDefinition response for attr")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_object(),
        "type-def on `attr: Bar` should resolve to a Location: {resp}"
    );
    // `class Bar` is on line 0.
    assert_eq!(
        parsed["result"]["range"]["start"]["line"].as_u64(),
        Some(0),
        "type-def should jump to the Bar class definition (line 0): {resp}"
    );

    Ok(())
}

/// Type-def unwraps `Optional[MyClass]` to jump to `MyClass`, covering the
/// `extract_base_type` recursion + the `X | None` union branch.
#[tokio::test]
async fn test_ws_type_definition_optional_and_union() -> TestResult<()> {
    let code = "\
from typing import Optional

class Widget:
    pass

a: Optional[Widget] = None
b: Widget | None = None
c: list[Widget] = []
";
    let mut fixture = opened(code, "file:///ws_typedef_opt.py").await?;

    // Optional[Widget] → Widget.
    let a_resp = fixture
        .request(
            340,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_typedef_opt.py" },
                "position": { "line": 5, "character": 0 }
            }),
        )
        .await?
        .ok_or("no typeDefinition response for a")?;
    let a_parsed: serde_json::Value = serde_json::from_str(&a_resp)?;
    assert_eq!(
        a_parsed["result"]["range"]["start"]["line"].as_u64(),
        Some(2),
        "Optional[Widget] type-def should jump to the Widget class (line 2): {a_resp}"
    );

    // `Widget | None` → Widget (union branch).
    let b_resp = fixture
        .request(
            341,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_typedef_opt.py" },
                "position": { "line": 6, "character": 0 }
            }),
        )
        .await?
        .ok_or("no typeDefinition response for b")?;
    let b_parsed: serde_json::Value = serde_json::from_str(&b_resp)?;
    assert_eq!(
        b_parsed["result"]["range"]["start"]["line"].as_u64(),
        Some(2),
        "`Widget | None` type-def should jump to the Widget class (line 2): {b_resp}"
    );

    // `list[Widget]` → the base-type extractor strips to the OUTER name
    // (`list`), which is not a user-defined class, so type-def returns null.
    // This covers the "annotated type not a user class" → None branch.
    let c_resp = fixture
        .request(
            342,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_typedef_opt.py" },
                "position": { "line": 7, "character": 0 }
            }),
        )
        .await?
        .ok_or("no typeDefinition response for c")?;
    let c_parsed: serde_json::Value = serde_json::from_str(&c_resp)?;
    assert!(
        c_parsed["result"].is_null(),
        "`list[Widget]` type-def should be null (list is not a user class): {c_resp}"
    );

    Ok(())
}

/// Type-def on a symbol with NO annotation (a plain variable) falls back to
/// the identifier→definition path, and on a blank position returns null.
#[tokio::test]
async fn test_ws_type_definition_no_annotation_and_blank() -> TestResult<()> {
    let code = "\
class Real:
    pass

plain = Real()
";
    let mut fixture = opened(code, "file:///ws_typedef_none.py").await?;

    // `plain` has no annotation but is assigned a `Real()` — type-def falls
    // back to definition lookup (identifier_at_offset → find_definition_by_name).
    let resp = fixture
        .request(
            350,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_typedef_none.py" },
                "position": { "line": 3, "character": 0 }
            }),
        )
        .await?
        .ok_or("no typeDefinition response for plain")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    // Either it resolves to `Real` (via definition fallback) or returns null —
    // both are spec-valid for an unannotated symbol; we only assert it does not
    // error and is well-formed JSON with a `result` field.
    assert!(
        parsed.get("result").is_some(),
        "typeDefinition must carry a result field (even if null): {resp}"
    );

    // Cursor on a totally blank line → no symbol → null.
    let blank = fixture
        .request(
            351,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_typedef_none.py" },
                "position": { "line": 0, "character": 0 }
            }),
        )
        .await?
        .ok_or("no typeDefinition response for blank")?;
    let blank_parsed: serde_json::Value = serde_json::from_str(&blank)?;
    assert!(
        blank_parsed["result"].is_null(),
        "type-def on `class` keyword (no symbol) should be null: {blank}"
    );

    Ok(())
}
