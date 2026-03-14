//! JSON-RPC diagnostic parsing and assertion helpers.
//!
//! These helpers work with raw `serde_json::Value` diagnostics
//! from LSP `publishDiagnostics` notifications.

/// Parse the first diagnostic with the given code from a `publishDiagnostics` message.
#[must_use]
pub fn extract_diagnostic<'a>(
    diag_json: &'a serde_json::Value,
    code: &str,
) -> Option<&'a serde_json::Value> {
    diag_json["params"]["diagnostics"]
        .as_array()?
        .iter()
        .find(|d| d["code"].as_str() == Some(code))
}

/// Assert that a diagnostic has a valid LSP range (all four fields present and >= 0).
///
/// # Panics
///
/// Panics if the diagnostic is missing a `range` field or if any of the
/// `start.line`, `start.character`, `end.line`, `end.character` values are absent.
pub fn assert_valid_range(diag: &serde_json::Value, label: &str) {
    let range = &diag["range"];
    assert!(
        !range.is_null(),
        "{label}: diagnostic must have a range: {diag}"
    );
    let sl = range["start"]["line"].as_u64();
    let sc = range["start"]["character"].as_u64();
    let el = range["end"]["line"].as_u64();
    let ec = range["end"]["character"].as_u64();
    assert!(
        sl.is_some() && sc.is_some() && el.is_some() && ec.is_some(),
        "{label}: range must have start/end line+character: {range}"
    );
}
