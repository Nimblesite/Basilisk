//! Tests for [LSPARCH-TESTING], [LSPARCH-FEATURES-COMPLETION],
//! [LSPARCH-FEATURES-INLAYHINTS], and [LSPFMT-CAPABILITIES].
//! See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
//!
//! This is deliberately an editor journey, not a handler test: every action is
//! a JSON-RPC message over the same WebSocket transport used by the VSIX.

use super::ws_test_common::*;
#[path = "ws_test_editor_journey_assertions.rs"]
mod ws_test_editor_journey_assertions;
use ws_test_editor_journey_assertions::*;

#[tokio::test]
#[expect(clippy::too_many_lines)]
async fn test_ws_editor_journey_resolves_and_refreshes_rich_features() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;

    // Interaction 1: the editor negotiates every feature used below.
    let initialize_raw = fixture.initialize().await?;
    let initialize: serde_json::Value = serde_json::from_str(&initialize_raw)?;
    let capabilities = &initialize["result"]["capabilities"];
    assert_eq!(initialize["jsonrpc"], "2.0");
    assert_eq!(initialize["id"], 1);
    assert_eq!(capabilities["completionProvider"]["resolveProvider"], true);
    assert_eq!(capabilities["inlayHintProvider"], true);
    assert_eq!(capabilities["colorProvider"], true);
    assert_eq!(capabilities["documentRangeFormattingProvider"], true);
    assert_eq!(capabilities["hoverProvider"], true);

    let uri = format!(
        "file://{}/editor_journey.py",
        fixture.workspace_root.display()
    );
    let code = r##"from typing import Generic, ParamSpec, TypeVar, TypeVarTuple

T_co = TypeVar("T_co", bound=str, covariant=True, default=bytes)
T_contra = TypeVar("T_contra", int, str, contravariant=True)
T_auto = TypeVar("T_auto", infer_variance=True)
Ts = TypeVarTuple("Ts", default=tuple)
P = ParamSpec("P", default=str)
T_plain = TypeVar("T_plain")

class Base:
    pass

class Palette(Base, Generic[T_co, T_contra]):
    """Stores named colors."""

    title = "ocean".upper()
    accent = "#336699cc"

def render(name: str, shade: str):
    """Render a named shade."""
    return "painted"

current = render("wall", "#f00")
primary = "#ff0000"
secondary = "#00ff0080"
value=1

"##;

    // Interaction 2: opening the document drives parsing, indexing, and diagnostics.
    fixture.did_open(&uri, code).await?;
    let diagnostics_raw = fixture.wait_for_diagnostics().await?;
    assert_diagnostics_notification(&diagnostics_raw, &uri, false)?;

    // Interaction 3: one inlay request exercises variable, call-site, return,
    // TypeVar, ParamSpec, TypeVarTuple, and legacy Generic hints together.
    let inlay = request_value(
        &mut fixture,
        1000,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 27, "character": 0 }
            }
        }),
    )
    .await?;
    let hints = inlay["result"]
        .as_array()
        .ok_or("inlay result must be an array")?;
    let hint_labels = labels(hints);
    for expected in [
        "  covariant, bound: str, default: bytes",
        "  contravariant, int | str",
        "  infer_variance",
        "  TypeVarTuple, default: tuple",
        "  ParamSpec, default: str",
        "[T_co, T_contra]",
        "name=",
        "shade=",
    ] {
        assert!(
            hint_labels.contains(&expected),
            "missing `{expected}` in {hint_labels:?}"
        );
    }
    assert!(
        hint_labels.iter().any(|label| label.contains("-> str")),
        "missing inferred return hint in {hint_labels:?}"
    );
    assert!(hint_labels.iter().any(|label| label == &": int"));
    assert!(!hint_labels.iter().any(|label| label.contains("T_plain]")));
    for (label, source_line) in [
        ("  covariant, bound: str, default: bytes", "T_co ="),
        ("  contravariant, int | str", "T_contra ="),
        ("  infer_variance", "T_auto ="),
        ("  TypeVarTuple, default: tuple", "Ts ="),
        ("  ParamSpec, default: str", "P ="),
    ] {
        let hint = item_named(hints, label)?;
        assert_eq!(hint["position"], line_end_position(code, source_line)?);
        assert_eq!(hint["kind"], 1);
        assert_eq!(hint["paddingLeft"], true);
        assert!(hint["paddingRight"].is_null());
        assert!(hint["tooltip"].is_null());
        assert!(hint["textEdits"].is_null());
        assert!(hint["data"].is_null());
    }
    for hint in hints {
        let label = hint["label"].as_str().ok_or("hint label must be text")?;
        let expected_kind = if label.ends_with('=') { 2 } else { 1 };
        assert_eq!(
            hint["kind"], expected_kind,
            "hint kind must match its label: {hint}"
        );
        assert!(
            hint["position"]["line"].is_u64(),
            "missing hint line: {hint}"
        );
        assert!(
            hint["position"]["character"].is_u64(),
            "missing hint column: {hint}"
        );
        assert!(!label.is_empty());
    }
    let generic_hint = hints
        .iter()
        .find(|hint| hint["label"] == "[T_co, T_contra]")
        .ok_or("missing Generic class hint")?;
    assert_eq!(
        generic_hint["tooltip"],
        "Generic type parameters from Generic[...] base"
    );
    let palette_position = source_position(code, "Palette")?;
    assert_eq!(generic_hint["position"]["line"], palette_position["line"]);
    assert_eq!(
        generic_hint["position"]["character"].as_u64(),
        palette_position["character"]
            .as_u64()
            .map(|column| column + u64::try_from("Palette".len()).unwrap_or(0))
    );

    // Interaction 4: ask for the same completion list that IntelliSense shows.
    let completion_line = u32::try_from(code.lines().count())?;
    let completion = request_value(
        &mut fixture,
        1001,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": completion_line, "character": 0 }
        }),
    )
    .await?;
    let completion_items = completion["result"]
        .as_array()
        .or_else(|| completion["result"]["items"].as_array())
        .ok_or("completion result must be an array")?;
    assert!(
        completion_items.len() > 20,
        "expected local and builtin items"
    );
    let render = item_named(completion_items, "render")?;
    let palette = item_named(completion_items, "Palette")?;
    let current = item_named(completion_items, "current")?;
    let print = item_named(completion_items, "print")?;
    assert_eq!(render["kind"], 3);
    assert_eq!(render["detail"], "(name, shade)");
    assert_eq!(render["data"]["kind"], "function");
    assert!(render["documentation"].is_null());
    assert_eq!(palette["kind"], 7);
    assert_eq!(palette["data"]["kind"], "class");
    assert!(palette["documentation"].is_null());
    assert_eq!(current["kind"], 6);
    assert_eq!(current["data"]["kind"], "variable");
    assert!(print.get("data").is_none() || print["data"].is_null());

    // Interactions 5-8: selecting four completion entries sends the exact
    // returned items back through completionItem/resolve.
    let resolved_render =
        request_value(&mut fixture, 1002, "completionItem/resolve", render.clone()).await?;
    assert_eq!(resolved_render["result"]["label"], "render");
    assert_eq!(resolved_render["result"]["kind"], 3);
    assert_eq!(resolved_render["result"]["detail"], "(name, shade)");
    assert_eq!(resolved_render["result"]["data"], render["data"]);
    assert_eq!(
        resolved_render["result"]["documentation"]["kind"],
        "markdown"
    );
    assert_eq!(
        resolved_render["result"]["documentation"]["value"],
        "Render a named shade."
    );

    let resolved_palette = request_value(
        &mut fixture,
        1003,
        "completionItem/resolve",
        palette.clone(),
    )
    .await?;
    assert_eq!(resolved_palette["result"]["label"], "Palette");
    assert_eq!(resolved_palette["result"]["kind"], 7);
    assert_eq!(resolved_palette["result"]["detail"], "class");
    assert_eq!(resolved_palette["result"]["data"], palette["data"]);
    assert_eq!(
        resolved_palette["result"]["documentation"]["kind"],
        "markdown"
    );
    assert_eq!(
        resolved_palette["result"]["documentation"]["value"],
        "Stores named colors."
    );

    let resolved_current = request_value(
        &mut fixture,
        1004,
        "completionItem/resolve",
        current.clone(),
    )
    .await?;
    assert_eq!(resolved_current["result"]["label"], "current");
    assert_eq!(resolved_current["result"]["kind"], 6);
    assert_eq!(resolved_current["result"]["detail"], "variable");
    assert_eq!(resolved_current["result"]["data"], current["data"]);
    assert!(resolved_current["result"]["documentation"].is_null());

    let resolved_print =
        request_value(&mut fixture, 1005, "completionItem/resolve", print.clone()).await?;
    assert_eq!(
        resolved_print["result"], print,
        "no-data builtin must round-trip"
    );
    assert_eq!(resolved_print["result"]["label"], "print");
    assert_eq!(resolved_print["result"]["kind"], 3);
    assert_eq!(resolved_print["result"]["detail"], "built-in");

    // Interaction 9: hover an engine-inferred class attribute.
    let hover = request_value(
        &mut fixture,
        1006,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": source_position(code, "title")?
        }),
    )
    .await?;
    let hover_text = hover["result"]["contents"]["value"]
        .as_str()
        .or_else(|| hover["result"]["contents"].as_str())
        .ok_or("hover contents must contain text")?;
    assert!(
        hover_text.contains("(property)"),
        "unexpected hover: {hover}"
    );
    assert!(
        hover_text.contains("Palette.title"),
        "unexpected hover: {hover}"
    );
    assert!(hover_text.contains(": str"), "unexpected hover: {hover}");
    assert!(
        !hover_text.contains("Unknown"),
        "hover must not leak Unknown: {hover}"
    );

    // Interaction 10: document colors are requested for all source swatches.
    let colors = request_value(
        &mut fixture,
        1007,
        "textDocument/documentColor",
        serde_json::json!({ "textDocument": { "uri": uri } }),
    )
    .await?;
    let color_items = colors["result"]
        .as_array()
        .ok_or("documentColor result must be an array")?;
    assert_eq!(
        color_items.len(),
        4,
        "expected 3, 6, and 8 digit colors: {colors}"
    );
    let _accent_color = assert_color(color_items, code, "#336699cc", [0.2, 0.4, 0.6, 0.8])?;
    let _short_red = assert_color(color_items, code, "#f00", [1.0, 0.0, 0.0, 1.0])?;
    let opaque_red = assert_color(color_items, code, "#ff0000", [1.0, 0.0, 0.0, 1.0])?;
    let _translucent_green = assert_color(
        color_items,
        code,
        "#00ff0080",
        [0.0, 1.0, 0.0, 128.0 / 255.0],
    )?;

    // Interaction 11: opening VS Code's color picker asks for presentations.
    let presentations = request_value(
        &mut fixture,
        1008,
        "textDocument/colorPresentation",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "color": opaque_red["color"],
            "range": opaque_red["range"]
        }),
    )
    .await?;
    let presentation_items = presentations["result"]
        .as_array()
        .ok_or("colorPresentation result must be an array")?;
    assert_eq!(presentation_items.len(), 2);
    assert_eq!(presentation_items[0]["label"], "#ff0000");
    assert_eq!(presentation_items[1]["label"], "#ff0000ff");
    for item in presentation_items {
        assert_eq!(item["textEdit"]["range"], opaque_red["range"]);
        assert_eq!(item["textEdit"]["newText"], item["label"]);
        assert!(item["additionalTextEdits"].is_null());
    }

    // Interaction 12: Format Selection touches only the line the user selected.
    let value_position = source_position(code, "value=1")?;
    let value_line = value_position["line"]
        .as_u64()
        .ok_or("missing value line")?;
    let formatted = request_value(
        &mut fixture,
        1009,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": value_line, "character": 0 },
                "end": { "line": value_line, "character": 7 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )
    .await?;
    let format_edits = formatted["result"]
        .as_array()
        .ok_or("rangeFormatting must return edits")?;
    assert_eq!(format_edits.len(), 1);
    assert_eq!(format_edits[0]["newText"], "value = 1");
    assert_eq!(format_edits[0]["range"]["start"]["line"], value_line);
    assert_eq!(format_edits[0]["range"]["start"]["character"], 0);
    assert_eq!(format_edits[0]["range"]["end"]["line"], value_line);
    assert_eq!(format_edits[0]["range"]["end"]["character"], 7);

    // Interaction 13: the editor applies a full-buffer change and receives a
    // fresh diagnostic publication before making more requests.
    let updated = code.replace("value=1", "value = 2\nadded = \"new\"");
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": updated }]
            }
        }))
        .await?;
    let changed_raw = fixture.wait_for_diagnostics().await?;
    assert_diagnostics_notification(&changed_raw, &uri, false)?;

    // Interaction 14: formatting the now-clean selection is a strict no-op.
    let clean_format = request_value(
        &mut fixture,
        1010,
        "textDocument/rangeFormatting",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": value_line, "character": 0 },
                "end": { "line": value_line, "character": 9 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )
    .await?;
    assert!(clean_format["result"].is_null());
    assert_eq!(clean_format["id"], 1010);

    // Interaction 15: saving re-runs analysis on the in-memory version.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": { "textDocument": { "uri": uri }, "text": updated }
        }))
        .await?;
    let saved_raw = fixture.wait_for_diagnostics().await?;
    assert_diagnostics_notification(&saved_raw, &uri, false)?;

    // Interaction 16: closing the editor clears open-file diagnostics.
    fixture.did_close(&uri).await?;
    let closed_raw = fixture.wait_for_diagnostics().await?;
    assert_diagnostics_notification(&closed_raw, &uri, true)?;
    Ok(())
}
