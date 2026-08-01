//! Tests for [LSPARCH-FEATURES-COMPLETION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-COMPLETION
// Tests for LSP: `ws_test_completion_receivers` — how a dot receiver's class is
// resolved, on both paths `receiver_type_name` takes.
//
// The member data is loaded in every case here; the bugs were in the KEY used
// to look it up.
//
//   annotation path — the key was the annotation's raw source text, so
//   `list[int]` matched no class while bare `list` worked (GitHub #388).
//
//   inferred path — the key was a rendered DISPLAY STRING, so a `str` literal
//   keyed on `LiteralString` and a list literal on `list[int]`, neither of
//   which names a class (GitHub #389).

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

// ── Inferred receivers (GitHub #389) ─────────────────────────────────────────

/// GitHub #389: `s = "abc"` inferred `LiteralString`, which was rendered to a
/// display string and used as a class-name key. No class is named
/// `LiteralString`, so the most ordinary line in Python offered nothing —
/// while every other way of producing the same `str` worked.
#[tokio::test]
async fn test_ws_completion_str_literal_receiver_offers_members() -> TestResult<()> {
    let annotated =
        dot_completion_labels("file:///recv_str_annotated.py", "s: str = \"abc\"\ns.", 570).await?;
    assert!(
        annotated.contains(&"split".to_owned()),
        "baseline: an annotated `str` must offer `split`: {annotated:?}"
    );

    let inferred =
        dot_completion_labels("file:///recv_str_inferred.py", "s = \"abc\"\ns.", 571).await?;
    assert!(
        inferred.contains(&"split".to_owned()),
        "`s = \"abc\"` must offer the same members as `s: str = \"abc\"` \
         (which offered {}): {inferred:?}",
        annotated.len()
    );

    Ok(())
}

/// The same display-string round-trip broke inferred CONTAINER receivers:
/// `xs = [1, 2]` keyed on `list[int]`, `d = {"k": "v"}` on `dict[str, str]`.
#[tokio::test]
async fn test_ws_completion_inferred_container_receivers_offer_members() -> TestResult<()> {
    let list_labels =
        dot_completion_labels("file:///recv_list_inferred.py", "xs = [1, 2]\nxs.", 572).await?;
    assert!(
        list_labels.contains(&"append".to_owned()),
        "`xs = [1, 2]` must offer `append`: {list_labels:?}"
    );

    let dict_labels = dot_completion_labels(
        "file:///recv_dict_inferred.py",
        "d = {\"k\": \"v\"}\nd.",
        573,
    )
    .await?;
    assert!(
        dict_labels.contains(&"keys".to_owned()),
        "`d = {{\"k\": \"v\"}}` must offer `keys`: {dict_labels:?}"
    );

    Ok(())
}

/// A `str` LITERAL receiver selects the PEP 675 `LiteralString` overloads; an
/// ordinary `str` receiver must not.
///
/// The flag carrying that distinction was dead on the inferred path — it tested
/// `inferred == "str"`, which a `LiteralString` render never satisfies — so
/// fixing the lookup had to revive the flag, not hardcode `false`.
///
/// Asserted through HOVER on `join`, because that is where the flag is
/// observable: it filters overload declarations, and `str.join` has a
/// `LiteralString` overload beside the plain `str` one. Completion cannot show
/// this — both receivers offer the same 64 member NAMES, so an item-count
/// comparison would pass with the flag still dead.
#[tokio::test]
async fn test_ws_hover_str_literal_receiver_selects_literalstring_overload() -> TestResult<()> {
    // `s = "abc"` is provably a LiteralString: the overload must be offered.
    let inferred_hover = hover_at(
        "file:///recv_lit_flag.py",
        "s = \"abc\"\nout = s.join([])\n",
        1,
        9,
        574,
    )
    .await?;
    assert!(
        inferred_hover.contains("LiteralString"),
        "`s = \"abc\"` is as provably a LiteralString as `\"abc\"` itself, so \
         hovering `join` must offer the PEP 675 overload: {inferred_hover}"
    );

    // `s: str` is NOT provably literal — the same overload must be filtered out,
    // which is what makes the assertion above meaningful.
    let annotated_hover = hover_at(
        "file:///recv_str_flag.py",
        "s: str = \"abc\"\nout = s.join([])\n",
        1,
        9,
        575,
    )
    .await?;
    assert!(
        !annotated_hover.contains("LiteralString"),
        "an ordinary `str` receiver must NOT be offered the LiteralString \
         overload: {annotated_hover}"
    );

    Ok(())
}

/// Receivers that were already working must stay working: an int literal takes
/// the same inferred path and must not regress.
#[tokio::test]
async fn test_ws_completion_int_literal_receiver_still_works() -> TestResult<()> {
    let labels = dot_completion_labels("file:///recv_int.py", "n = 5\nn.", 576).await?;
    assert!(
        labels.contains(&"bit_length".to_owned()),
        "`n = 5` must keep offering `bit_length`: {labels:?}"
    );

    Ok(())
}

/// An untypeable receiver still resolves to nothing rather than a guess.
#[tokio::test]
async fn test_ws_completion_unknown_receiver_offers_nothing() -> TestResult<()> {
    let labels = dot_completion_labels(
        "file:///recv_unknown.py",
        "def src(): ...\n\n\nv = src()\nv.",
        577,
    )
    .await?;
    assert!(
        labels.is_empty(),
        "an untypeable receiver must offer no builtin members: {labels:?}"
    );

    Ok(())
}
