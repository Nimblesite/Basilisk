//! Tests for LSP: ws_test_selection_ranges.

#![allow(dead_code, unused_imports)]

mod ws_test_common;
use ws_test_common::*;

// ── Selection Ranges ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_selection_ranges() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Class with a method that has a typed parameter — cursor on the parameter name
    // should yield nested ranges: param name → function def → class def → whole doc.
    let code = "\
class Greeter:
    def greet(self, name: str) -> str:
        return f\"Hello, {name}!\"
";
    fixture.did_open("file:///ws_selection.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor on the 'n' of `name` parameter (line 1, character 20).
    let resp = fixture
        .request(
            620,
            "textDocument/selectionRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_selection.py" },
                "positions": [{ "line": 1, "character": 20 }]
            }),
        )
        .await?
        .ok_or("no selection range response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("selection range result should be an array")?;

    // One position → one SelectionRange.
    assert_eq!(
        ranges.len(),
        1,
        "should have exactly 1 selection range for 1 position: {resp}"
    );

    let sel = &ranges[0];
    // The innermost range should exist.
    assert!(
        sel.get("range").is_some(),
        "selection range should have a range: {resp}"
    );

    // Walk the parent chain — there should be at least 2 levels
    // (innermost + at least one parent containing the whole document).
    let mut depth = 1;
    let mut current = sel.clone();
    while let Some(parent) = current.get("parent") {
        if parent.is_null() {
            break;
        }
        depth += 1;
        current = parent.clone();
    }
    assert!(
        depth >= 2,
        "selection range should have nested parents (depth >= 2), got {depth}: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_selection_ranges_has_parent_chain() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Deeply nested structure to ensure parent chain hierarchy.
    let code = "\
class Container:
    def process(self, data: str) -> str:
        result: str = data.upper()
        return result
";
    fixture.did_open("file:///ws_sel_chain.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor on 'result' inside the method body (line 2, character 8).
    let resp = fixture
        .request(
            730,
            "textDocument/selectionRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_sel_chain.py" },
                "positions": [{ "line": 2, "character": 8 }]
            }),
        )
        .await?
        .ok_or("no selection range response for parent chain test")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("selection range result should be an array")?;

    assert_eq!(
        ranges.len(),
        1,
        "should have exactly 1 selection range for 1 position: {resp}"
    );

    let sel = &ranges[0];

    // The innermost range must exist with a valid range object.
    let inner_range = sel
        .get("range")
        .ok_or("selection range should have a range")?;
    assert!(
        inner_range.get("start").is_some() && inner_range.get("end").is_some(),
        "innermost range should have start and end positions: {resp}"
    );

    // Walk up the parent chain and verify hierarchy:
    // Each parent's range should be equal to or larger than its child.
    let mut depth = 1;
    let mut current = sel.clone();
    let mut prev_start_line = inner_range["start"]["line"].as_u64().unwrap_or(u64::MAX);
    let mut prev_end_line = inner_range["end"]["line"].as_u64().unwrap_or(0);

    while let Some(parent) = current.get("parent") {
        if parent.is_null() {
            break;
        }
        depth += 1;

        let parent_range = parent
            .get("range")
            .ok_or("parent selection range should have a range")?;
        let parent_start = parent_range["start"]["line"].as_u64().unwrap_or(u64::MAX);
        let parent_end = parent_range["end"]["line"].as_u64().unwrap_or(0);

        // Parent range must be at least as large as child range.
        assert!(
            parent_start <= prev_start_line && parent_end >= prev_end_line,
            "parent range ({parent_start}..{parent_end}) should contain child range ({prev_start_line}..{prev_end_line}): {resp}"
        );

        prev_start_line = parent_start;
        prev_end_line = parent_end;
        current = parent.clone();
    }

    // For a variable inside a method inside a class, we expect at least 3 levels:
    // variable/statement -> method -> class (or more).
    assert!(
        depth >= 3,
        "selection range chain should have at least 3 levels of nesting (var -> method -> class), got {depth}: {resp}"
    );

    Ok(())
}
