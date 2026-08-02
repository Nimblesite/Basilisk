//! Tests for [LSPARCH-FEATURES-SELECTION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-SELECTION
// Coverage-boost tests for `textDocument/selectionRange`: exercises cursor
// positions on function names, annotated variables, class attributes, kwargs,
// return annotations, and import statements — the spans the basic
// `ws_test_selection_ranges` module does not place a cursor on.

use super::ws_test_common::*;

async fn opened(code: &str, uri: &str) -> TestResult<WsTestFixture> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    fixture.did_open(uri, code).await?;
    let _ = fixture.wait_for_diagnostics().await;
    Ok(fixture)
}

async fn selection_at(
    fixture: &mut WsTestFixture,
    id: u64,
    uri: &str,
    line: u32,
    character: u32,
) -> TestResult<serde_json::Value> {
    let resp = fixture
        .request(
            id,
            "textDocument/selectionRange",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "positions": [{ "line": line, "character": character }]
            }),
        )
        .await?
        .ok_or("no selectionRange response")?;
    Ok(serde_json::from_str(&resp)?)
}

/// Cursor on a function NAME and on a return annotation must each yield a
/// nested selection range chain (covers `func.name_span` + return-annotation
/// spans).
#[tokio::test]
async fn test_ws_selection_range_function_name_and_return() -> TestResult<()> {
    let code = "\
def compute(value: int) -> int:
    return value + 1
";
    let mut fixture = opened(code, "file:///ws_sel_func.py").await?;

    // Cursor on 'compute' name (line 0, char 4).
    let parsed = selection_at(&mut fixture, 900, "file:///ws_sel_func.py", 0, 4).await?;
    let ranges = parsed["result"]
        .as_array()
        .ok_or("selection result should be an array")?;
    assert_eq!(ranges.len(), 1, "one position → one range: {parsed}");
    assert!(
        ranges[0].get("range").is_some(),
        "selection should carry a range: {parsed}"
    );

    // Cursor on the return annotation `int` (line 0, char 29).
    let ret_parsed = selection_at(&mut fixture, 901, "file:///ws_sel_func.py", 0, 29).await?;
    let ret_ranges = ret_parsed["result"]
        .as_array()
        .ok_or("return-annotation selection should be an array")?;
    assert_eq!(ret_ranges.len(), 1, "one position → one range: {ret_parsed}");

    Ok(())
}

/// Cursor on an annotated variable's name and on its annotation covers the
/// variable `name_span` + `annotation_span` branches.
#[tokio::test]
async fn test_ws_selection_range_annotated_variable() -> TestResult<()> {
    let code = "count: int = 0\n";
    let mut fixture = opened(code, "file:///ws_sel_var.py").await?;

    // Cursor on 'count' (line 0, char 0).
    let name_parsed = selection_at(&mut fixture, 910, "file:///ws_sel_var.py", 0, 0).await?;
    assert!(
        name_parsed["result"].is_array(),
        "variable name selection should be an array: {name_parsed}"
    );

    // Cursor on the annotation 'int' (line 0, char 8).
    let ann_parsed = selection_at(&mut fixture, 911, "file:///ws_sel_var.py", 0, 8).await?;
    assert!(
        ann_parsed["result"].is_array(),
        "annotation selection should be an array: {ann_parsed}"
    );

    Ok(())
}

/// Cursor on a class attribute name and its annotation covers the attribute
/// `name_span` + `annotation_span` branches.
#[tokio::test]
async fn test_ws_selection_range_class_attribute() -> TestResult<()> {
    let code = "\
class Config:
    timeout: int
";
    let mut fixture = opened(code, "file:///ws_sel_attr.py").await?;

    // Cursor on 'timeout' (line 1, char 4).
    let parsed = selection_at(&mut fixture, 920, "file:///ws_sel_attr.py", 1, 4).await?;
    let ranges = parsed["result"]
        .as_array()
        .ok_or("attr selection should be an array")?;
    assert_eq!(ranges.len(), 1);
    // Walk the parent chain — attribute → class → document.
    let mut depth = 1;
    let mut current = ranges[0].clone();
    while let Some(parent) = current.get("parent") {
        if parent.is_null() {
            break;
        }
        depth += 1;
        current = parent.clone();
    }
    assert!(
        depth >= 2,
        "attribute selection should nest into at least the class: depth={depth}"
    );

    // Cursor on the attribute annotation 'int' (line 1, char 13).
    let ann_parsed = selection_at(&mut fixture, 921, "file:///ws_sel_attr.py", 1, 13).await?;
    assert!(
        ann_parsed["result"].is_array(),
        "attribute annotation selection should be an array: {ann_parsed}"
    );

    Ok(())
}

/// Cursor on an import statement covers the `import.span` branch.
#[tokio::test]
async fn test_ws_selection_range_import() -> TestResult<()> {
    let code = "import os\n";
    let mut fixture = opened(code, "file:///ws_sel_import.py").await?;

    // Cursor on 'os' (line 0, char 7).
    let parsed = selection_at(&mut fixture, 930, "file:///ws_sel_import.py", 0, 7).await?;
    assert!(
        parsed["result"].is_array(),
        "import selection should be an array: {parsed}"
    );

    Ok(())
}

/// Multiple positions in one request → one selection range per position.
#[tokio::test]
async fn test_ws_selection_range_multiple_positions() -> TestResult<()> {
    let code = "\
class Bag:
    items: list

def main() -> None:
    pass
";
    let mut fixture = opened(code, "file:///ws_sel_multi.py").await?;

    let resp = fixture
        .request(
            940,
            "textDocument/selectionRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_sel_multi.py" },
                "positions": [
                    { "line": 0, "character": 6 },
                    { "line": 1, "character": 4 },
                    { "line": 3, "character": 4 }
                ]
            }),
        )
        .await?
        .ok_or("no multi-position selectionRange response")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let ranges = parsed["result"]
        .as_array()
        .ok_or("multi-position result should be an array")?;
    assert_eq!(
        ranges.len(),
        3,
        "three positions → three selection ranges: {resp}"
    );

    Ok(())
}
