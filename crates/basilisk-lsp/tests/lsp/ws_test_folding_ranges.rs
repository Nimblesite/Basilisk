// Tests for LSP: `ws_test_folding_ranges`.

use super::ws_test_common::*;

// ── Folding Ranges ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_folding_ranges() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Document with a multi-line class containing a multi-line function.
    let code = "\
import os
import sys

class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(name: str) -> str:
    return f\"Hello, {name}!\"
";
    fixture.did_open("file:///ws_folding.py", code).await?;
    fixture.wait_for_diagnostics().await?;

    let resp = fixture
        .request(
            610,
            "textDocument/foldingRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_folding.py" }
            }),
        )
        .await?
        .ok_or("no folding range response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("folding ranges result should be an array")?;

    // Should have ranges for: speak method, Animal class, greet function,
    // and possibly the import block.
    assert!(
        ranges.len() >= 3,
        "should have at least 3 folding ranges (class, 2 functions), got {}: {resp}",
        ranges.len()
    );

    // Verify each range has startLine and endLine.
    for range in ranges {
        assert!(
            range.get("startLine").is_some(),
            "folding range should have startLine: {resp}"
        );
        assert!(
            range.get("endLine").is_some(),
            "folding range should have endLine: {resp}"
        );
    }

    // Verify we have a region kind for the class/function ranges.
    let region_count = ranges
        .iter()
        .filter(|r| r["kind"].as_str() == Some("region"))
        .count();
    assert!(
        region_count >= 3,
        "should have at least 3 region-kind folding ranges, got {region_count}: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_folding_ranges_import_block() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Document with consecutive imports that should fold as one block.
    let code = "\
import os
import sys
import json
import typing

def main() -> None:
    pass
";
    fixture.did_open("file:///ws_fold_imports.py", code).await?;
    fixture.wait_for_diagnostics().await?;

    let resp = fixture
        .request(
            720,
            "textDocument/foldingRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_fold_imports.py" }
            }),
        )
        .await?
        .ok_or("no folding range response for import block")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("folding ranges result should be an array")?;

    // We should have at least 1 folding range for the imports block
    // and 1 for the main function.
    assert!(
        ranges.len() >= 2,
        "should have at least 2 folding ranges (imports + function), got {}: {resp}",
        ranges.len()
    );

    // Find a folding range that starts at line 0 (first import) and
    // covers at least through line 3 (last import).
    let has_import_fold = ranges.iter().any(|range| {
        let start = range["startLine"].as_u64().unwrap_or(u64::MAX);
        let end = range["endLine"].as_u64().unwrap_or(0);
        // Import block spans lines 0-3.
        start == 0 && end >= 3
    });

    assert!(
        has_import_fold,
        "consecutive imports should produce a folding range starting at line 0 covering through line 3: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_folding_ranges_nested_class_and_function() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Nested structures: class containing two methods.
    let code = "\
class Outer:
    def method_a(self) -> int:
        x: int = 1
        return x

    def method_b(self) -> str:
        y: str = \"hello\"
        return y

def standalone(val: int) -> int:
    return val + 1
";
    fixture.did_open("file:///ws_fold_nested.py", code).await?;
    fixture.wait_for_diagnostics().await?;

    let resp = fixture
        .request(
            721,
            "textDocument/foldingRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_fold_nested.py" }
            }),
        )
        .await?
        .ok_or("no folding range response for nested structures")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("folding ranges result should be an array")?;

    // We expect separate folding ranges for:
    // 1. Outer class (lines 0-8)
    // 2. method_a (lines 1-3)
    // 3. method_b (lines 5-7)
    // 4. standalone function (lines 9-10)
    assert!(
        ranges.len() >= 4,
        "should have at least 4 folding ranges (class + 2 methods + standalone), got {}: {resp}",
        ranges.len()
    );

    // Collect all (startLine, endLine) pairs.
    let fold_pairs: Vec<(u64, u64)> = ranges
        .iter()
        .filter_map(|range| {
            let start = range["startLine"].as_u64()?;
            let end = range["endLine"].as_u64()?;
            Some((start, end))
        })
        .collect();

    // The Outer class fold should start at line 0.
    let has_class_fold = fold_pairs.iter().any(|(start, _)| *start == 0);
    assert!(
        has_class_fold,
        "Outer class should have a folding range starting at line 0: {resp}"
    );

    // There should be a method fold starting at line 1 (method_a).
    let has_method_a_fold = fold_pairs.iter().any(|(start, _)| *start == 1);
    assert!(
        has_method_a_fold,
        "method_a should have a folding range starting at line 1: {resp}"
    );

    // There should be a method fold starting at line 5 (method_b).
    let method_b_has_fold = fold_pairs.iter().any(|(start, _)| *start == 5);
    assert!(
        method_b_has_fold,
        "method_b should have a folding range starting at line 5: {resp}"
    );

    Ok(())
}
