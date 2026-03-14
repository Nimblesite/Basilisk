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
    diag_json
        .get("params")?
        .get("diagnostics")?
        .as_array()?
        .iter()
        .find(|d| d.get("code").and_then(|c| c.as_str()) == Some(code))
}

/// Assert that a diagnostic has a valid LSP range (all four fields present and >= 0).
///
/// # Panics
///
/// Panics if the diagnostic is missing a `range` field or if any of the
/// `start.line`, `start.character`, `end.line`, `end.character` values are absent.
pub fn assert_valid_range(diag: &serde_json::Value, label: &str) {
    let range = diag.get("range");
    let range_ref = range.filter(|r| !r.is_null());
    assert!(
        range_ref.is_some(),
        "{label}: diagnostic must have a range: {diag}"
    );
    // SAFETY: assert above guarantees Some
    let range = range_ref;
    let sl = range
        .and_then(|r| r.get("start"))
        .and_then(|s| s.get("line"))
        .and_then(serde_json::Value::as_u64);
    let sc = range
        .and_then(|r| r.get("start"))
        .and_then(|s| s.get("character"))
        .and_then(serde_json::Value::as_u64);
    let el = range
        .and_then(|r| r.get("end"))
        .and_then(|s| s.get("line"))
        .and_then(serde_json::Value::as_u64);
    let ec = range
        .and_then(|r| r.get("end"))
        .and_then(|s| s.get("character"))
        .and_then(serde_json::Value::as_u64);
    assert!(
        sl.is_some() && sc.is_some() && el.is_some() && ec.is_some(),
        "{label}: range must have start/end line+character: {range:?}"
    );
}
