//! Tests for [LSPARCH-FEATURES-COMPLETION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-COMPLETION
// Tests for LSP: `ws_test_completion_generics` — a receiver whose annotation is
// a SUBSCRIPTED generic offers the same members as the bare form. `list[int]`
// names `list` with an element type; it is not a different class.
//
// The member data was already loaded — bare `list` completed fine — so this was
// never a missing-stub problem. The lookup key was the annotation's raw source
// text, and `list[int]` matched no class. GitHub #388.

use super::ws_test_common::*;

/// Dot-complete at the end of the last line and return the item labels.
async fn dot_completion_labels(uri: &str, code: &str, request_id: u64) -> TestResult<Vec<String>> {
    let (mut fixture, _diag) = open_and_diagnose(uri, code).await?;
    let lines: Vec<&str> = code.lines().collect();
    let line = u32::try_from(lines.len() - 1).unwrap_or(0);
    let character = u32::try_from(lines.last().map_or(0, |l| l.len())).unwrap_or(0);

    let resp = fixture
        .request(
            request_id,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "context": { "triggerKind": 2, "triggerCharacter": "." }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let items = parsed["result"]["items"]
        .as_array()
        .or_else(|| parsed["result"].as_array())
        .map(Vec::as_slice)
        .unwrap_or_default();
    Ok(items
        .iter()
        .filter_map(|item| item["label"].as_str())
        .map(str::to_owned)
        .collect())
}

/// GitHub #388: `xs: list[int]` returned zero items while bare `xs: list`
/// returned the full member set.
#[tokio::test]
async fn test_ws_completion_subscripted_list_annotation_offers_members() -> TestResult<()> {
    let bare =
        dot_completion_labels("file:///comp_list_bare.py", "xs: list = [1]\nxs.", 560).await?;
    assert!(
        bare.contains(&"append".to_owned()),
        "baseline: bare `list` must offer `append`: {bare:?}"
    );

    let subscripted =
        dot_completion_labels("file:///comp_list_sub.py", "xs: list[int] = [1]\nxs.", 561).await?;
    assert!(
        subscripted.contains(&"append".to_owned()),
        "`list[int]` names `list` with an element type — it must offer the same \
         members as bare `list` (which offered {}): {subscripted:?}",
        bare.len()
    );

    Ok(())
}

/// The same defect on `dict`, whose two type parameters take a different
/// parsing path through the shared annotation entry point.
#[tokio::test]
async fn test_ws_completion_subscripted_dict_annotation_offers_members() -> TestResult<()> {
    let subscripted = dot_completion_labels(
        "file:///comp_dict_sub.py",
        "d: dict[str, int] = {}\nd.",
        562,
    )
    .await?;
    for member in ["keys", "items", "get"] {
        assert!(
            subscripted.contains(&member.to_owned()),
            "`dict[str, int]` must offer `{member}`: {subscripted:?}"
        );
    }

    Ok(())
}

/// A receiver annotated with a USER class keeps resolving to that class.
///
/// `receiver_type_name` returns the annotation as a class-name key, and hover
/// consumes it to find a same-file class. The shared annotation parser
/// LOWERCASES its input, so routing the builtin lookup through it must not
/// clobber a name whose case carries meaning — `Model` must not become `model`
/// and lose its members. Asserted on hover because that is the surface which
/// actually consumes the name for user classes; instance-receiver *completion*
/// on a user class is a separate gap that predates this fix.
#[tokio::test]
async fn test_ws_hover_user_class_annotation_keeps_case() -> TestResult<()> {
    let code = "class Model:\n    def validate(self) -> bool:\n        return True\n\n\nm: Model = Model()\nm.validate()\n";
    // Line 6 `m.validate()` — cursor inside `validate`.
    let resp = hover_at("file:///comp_user_class.py", code, 6, 4, 563).await?;

    assert!(
        resp.contains("validate"),
        "a user-class receiver must still resolve its own method on hover: {resp}"
    );

    Ok(())
}
