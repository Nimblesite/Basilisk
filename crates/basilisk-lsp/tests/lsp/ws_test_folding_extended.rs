//! Tests for [LSPARCH-FEATURES-FOLDING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-FOLDING
// Coverage-boost tests for `textDocument/foldingRange`: exercises the
// multi-group import-block fold (two import groups separated by a blank line
// or non-import statement) and the empty-document edge.

use super::ws_test_common::*;

/// Two SEPARATE import groups (separated by a non-import line) each produce
/// their own `Imports` fold. Covers the `group_count >= 2` branch in
/// `folding.rs`.
#[tokio::test]
async fn test_ws_folding_multiple_import_groups() -> TestResult<()> {
    let code = "\
import os
import sys

x: int = 1

import json
import logging
";
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    fixture.did_open("file:///ws_fold_groups.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1000,
            "textDocument/foldingRange",
            serde_json::json!({ "textDocument": { "uri": "file:///ws_fold_groups.py" } }),
        )
        .await?
        .ok_or("no foldingRange response")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let ranges = parsed["result"]
        .as_array()
        .ok_or("folding result should be an array")?;

    // At least two import folds (one per group).
    let import_folds = ranges
        .iter()
        .filter(|r| r["kind"].as_str() == Some("imports"))
        .count();
    assert!(
        import_folds >= 2,
        "should have at least 2 import-group folds, got {import_folds}: {resp}"
    );

    // Every import fold should span at least one line.
    for fold in ranges.iter().filter(|r| r["kind"].as_str() == Some("imports")) {
        let start = fold["startLine"].as_u64().unwrap_or(u64::MAX);
        let end = fold["endLine"].as_u64().unwrap_or(0);
        assert!(
            end >= start,
            "import fold end should be >= start: {fold}"
        );
    }

    Ok(())
}

/// A document with NO foldable constructs returns null (or an empty array).
#[tokio::test]
async fn test_ws_folding_empty_document() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    fixture.did_open("file:///ws_fold_empty.py", "").await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1010,
            "textDocument/foldingRange",
            serde_json::json!({ "textDocument": { "uri": "file:///ws_fold_empty.py" } }),
        )
        .await?
        .ok_or("no foldingRange response for empty doc")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null() || parsed["result"].as_array().is_some_and(Vec::is_empty),
        "empty document should yield null/empty folds: {resp}"
    );

    Ok(())
}
