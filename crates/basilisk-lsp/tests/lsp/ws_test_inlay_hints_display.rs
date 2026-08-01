//! Tests for [LSPARCH-FEATURES-INLAYHINTS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-INLAYHINTS
// Tests for LSP: `ws_test_inlay_hints_display` — the display contract that
// `crates/basilisk-lsp/src/util.rs` states on `expr_type_display`: "never a
// guess, and never a partial `Unknown` inside a rendered type".
//
// `InferredType::Unknown` is an INTERNAL sentinel
// ([CHECKER-TYPE-INFERENCE-SPEC.md] "internal sentinel `InferredType::Unknown`");
// it is not a Python type and must never reach a user-visible label. GitHub #385.

use super::ws_test_common::*;

/// Collect every variable type hint label (`": <type>"`) from an inlay hint
/// response. A `null` result means no hints, which is an empty slice here.
fn type_hint_labels(resp: &str) -> TestResult<Vec<String>> {
    let parsed: serde_json::Value = serde_json::from_str(resp)?;
    Ok(parsed["result"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|hint| hint["label"].as_str())
        .filter(|label| label.starts_with(": "))
        .map(str::to_owned)
        .collect())
}

/// GitHub #385: container literals whose elements are untypeable rendered the
/// internal sentinel straight through — `list[Unknown]`, `dict[str, Unknown]`,
/// `tuple[Unknown, Unknown]`, `list[str | Unknown]`.
///
/// The `RhsKind` table path (`rhs_type_display`) only rejected a TOP-LEVEL
/// `Unknown`, so a sentinel nested inside a container escaped the same
/// `is_fully_known` gate that the engine fallback (`expr_type_display`) applies.
#[tokio::test]
async fn test_ws_inlay_hints_never_render_unknown_sentinel() -> TestResult<()> {
    let uri = "file:///inlay_unknown_nested.py";
    let code = concat!(
        "def src(): ...\n",
        "\n",
        "\n",
        "a = [src()]\n",
        "b = {\"k\": src()}\n",
        "c = (src(), src())\n",
        "d = [\"x\", src()]\n",
    );

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        140,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 7, "character": 0 }
            }
        }),
    )
    .await?;

    let labels = type_hint_labels(&resp)?;
    let leaked: Vec<&String> = labels
        .iter()
        .filter(|label| label.contains("Unknown"))
        .collect();
    assert!(
        leaked.is_empty(),
        "the internal `Unknown` sentinel must never reach an inlay hint label, \
         but these did: {leaked:?} (all type hints: {labels:?})"
    );

    Ok(())
}

/// The guard must not cost precision: a container whose elements ARE fully
/// known still renders its parameterized type. Guards the fix for #385 against
/// being "suppress everything".
#[tokio::test]
async fn test_ws_inlay_hints_known_containers_still_render() -> TestResult<()> {
    let uri = "file:///inlay_known_containers.py";
    let code = "a = [1, 2]\nb = {\"k\": \"v\"}\nc = (1, \"x\")\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        141,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 3, "character": 0 }
            }
        }),
    )
    .await?;

    let labels = type_hint_labels(&resp)?;
    for want in [": list[int]", ": dict[str, str]", ": tuple[int, str]"] {
        assert!(
            labels.iter().any(|label| label == want),
            "fully-known container should still hint {want:?}: {labels:?}"
        );
    }

    Ok(())
}
