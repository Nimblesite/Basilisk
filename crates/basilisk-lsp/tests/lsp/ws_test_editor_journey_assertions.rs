use super::{TestResult, WsTestFixture};

pub(super) async fn request_value(
    fixture: &mut WsTestFixture,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> TestResult<serde_json::Value> {
    let raw = fixture
        .request(id, method, params)
        .await?
        .ok_or_else(|| format!("no response to {method}"))?;
    let response: serde_json::Value = serde_json::from_str(&raw)?;
    assert_eq!(response["jsonrpc"], "2.0", "invalid response: {raw}");
    assert_eq!(response["id"], id, "response id did not match: {raw}");
    assert!(response["id"].is_u64(), "response id changed type: {raw}");
    assert!(
        response.get("method").is_none(),
        "request returned notification: {raw}"
    );
    assert!(
        response.get("params").is_none(),
        "response leaked params: {raw}"
    );
    assert!(
        response.get("error").is_none_or(serde_json::Value::is_null),
        "{method} returned an error: {raw}"
    );
    assert!(
        response.get("result").is_some(),
        "{method} omitted result: {raw}"
    );
    Ok(response)
}

pub(super) fn assert_diagnostics_notification(
    raw: &str,
    expected_uri: &str,
    require_empty: bool,
) -> TestResult<()> {
    let notification: serde_json::Value = serde_json::from_str(raw)?;
    assert_eq!(
        notification["jsonrpc"], "2.0",
        "invalid notification: {raw}"
    );
    assert_eq!(notification["method"], "textDocument/publishDiagnostics");
    assert_eq!(notification["params"]["uri"], expected_uri);
    assert!(
        notification.get("id").is_none(),
        "notification has id: {raw}"
    );
    assert!(
        notification.get("result").is_none(),
        "notification has result: {raw}"
    );
    assert!(
        notification.get("error").is_none(),
        "notification has error: {raw}"
    );
    assert!(notification["params"]["version"].is_null());
    let diagnostics = notification["params"]["diagnostics"]
        .as_array()
        .ok_or("published diagnostics must be an array")?;
    if require_empty {
        assert!(diagnostics.is_empty(), "diagnostics must be cleared: {raw}");
    }
    assert!(!raw.contains("BSK-PARSE"), "source must parse: {raw}");
    Ok(())
}

pub(super) fn item_named(
    items: &[serde_json::Value],
    label: &str,
) -> TestResult<serde_json::Value> {
    items
        .iter()
        .find(|item| item["label"].as_str() == Some(label))
        .cloned()
        .ok_or_else(|| format!("missing item `{label}`").into())
}

pub(super) fn source_position(source: &str, fragment: &str) -> TestResult<serde_json::Value> {
    let (line, text) = source
        .lines()
        .enumerate()
        .find(|(_, text)| text.contains(fragment))
        .ok_or_else(|| format!("missing source fragment `{fragment}`"))?;
    let character = text
        .find(fragment)
        .ok_or_else(|| format!("missing source fragment `{fragment}`"))?;
    Ok(serde_json::json!({
        "line": u32::try_from(line)?,
        "character": u32::try_from(character)?,
    }))
}

pub(super) fn line_end_position(source: &str, fragment: &str) -> TestResult<serde_json::Value> {
    let (line, text) = source
        .lines()
        .enumerate()
        .find(|(_, text)| text.contains(fragment))
        .ok_or_else(|| format!("missing source fragment `{fragment}`"))?;
    Ok(serde_json::json!({
        "line": u32::try_from(line)?,
        "character": u32::try_from(text.len())?,
    }))
}

pub(super) fn labels(items: &[serde_json::Value]) -> Vec<&str> {
    items
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect()
}

pub(super) fn assert_color(
    items: &[serde_json::Value],
    source: &str,
    literal: &str,
    expected: [f64; 4],
) -> TestResult<serde_json::Value> {
    let start = source_position(source, literal)?;
    let item = items
        .iter()
        .find(|item| item["range"]["start"] == start)
        .ok_or_else(|| format!("missing color `{literal}`"))?;
    assert_eq!(item["range"]["start"], start);
    assert_eq!(item["range"]["end"]["line"], start["line"]);
    assert_eq!(
        item["range"]["end"]["character"].as_u64(),
        start["character"]
            .as_u64()
            .map(|column| column + u64::try_from(literal.len()).unwrap_or(0))
    );
    let color = item["color"].as_object().ok_or("color must be an object")?;
    assert_eq!(color.len(), 4, "RGBA must have exactly four components");
    for (component, expected_value) in ["red", "green", "blue", "alpha"].into_iter().zip(expected) {
        let actual = color[component]
            .as_f64()
            .ok_or("component must be numeric")?;
        assert!(
            (actual - expected_value).abs() < f64::from(f32::EPSILON),
            "wrong {component}: {item}"
        );
    }
    Ok(item.clone())
}
