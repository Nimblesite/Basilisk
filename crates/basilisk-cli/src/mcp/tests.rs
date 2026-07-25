use super::*;

fn status() -> Value {
    json!({
        "active_source": "bundled",
        "commit_identity": "0123456789012345678901234567890123456789",
        "tree_identity": "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
        "license_status": "approved",
        "license_reference": "typeshed://LICENSE",
        "warnings": [
            { "code": "typeshed_source_unpinned", "message": "Pin a commit to make this reproducible", "docs_url": "https://www.basilisk-python.dev/errors/typeshed_source_unpinned" },
            { "code": "typeshed_source_license_changed", "message": "Basilisk update/review required", "docs_url": "https://www.basilisk-python.dev/errors/typeshed_source_license_changed" },
            { "code": "typeshed_source_user_managed", "message": "Folder supplies its own license", "docs_url": "https://www.basilisk-python.dev/errors/typeshed_source_user_managed" }
        ]
    })
}

fn exchange(messages: &[Value]) -> Result<Vec<Value>, String> {
    exchange_with_status(messages, || Ok(status()))
}

fn exchange_with_status<F>(messages: &[Value], status: F) -> Result<Vec<Value>, String>
where
    F: Fn() -> Result<Value, String>,
{
    let input = messages
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .join("\n");
    let mut output = Vec::new();
    run_transport(std::io::Cursor::new(input), &mut output, status)?;
    String::from_utf8(output)
        .map_err(|error| error.to_string())?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(|error| error.to_string()))
        .collect()
}

#[test]
fn lifecycle_lists_and_calls_structured_status() -> Result<(), String> {
    let responses = exchange(&[
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":STATUS_TOOL,"arguments":{}}}),
    ])?;
    assert_eq!(responses.len(), 3);
    assert_eq!(
        responses
            .first()
            .and_then(|value| value.pointer("/result/protocolVersion"))
            .and_then(Value::as_str),
        Some(PROTOCOL_VERSION)
    );
    assert_eq!(
        responses
            .get(1)
            .and_then(|value| value.pointer("/result/tools/0/name"))
            .and_then(Value::as_str),
        Some(STATUS_TOOL)
    );
    let call = responses
        .get(2)
        .ok_or_else(|| "tool response missing".to_owned())?;
    let document = call
        .pointer("/result/structuredContent")
        .ok_or_else(|| "structured status missing".to_owned())?;
    let text = call
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .ok_or_else(|| "text status missing".to_owned())?;
    let text_document: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    assert_eq!(text_document, *document);
    let warnings = document
        .get("warnings")
        .and_then(Value::as_array)
        .ok_or_else(|| "structured warnings missing".to_owned())?;
    assert_eq!(
        warnings
            .first()
            .and_then(|warning| warning.get("code"))
            .and_then(Value::as_str),
        Some("typeshed_source_unpinned")
    );
    assert_eq!(
        warnings
            .first()
            .and_then(|warning| warning.get("docs_url"))
            .and_then(Value::as_str),
        Some("https://www.basilisk-python.dev/errors/typeshed_source_unpinned")
    );
    assert_eq!(
        warnings
            .get(1)
            .and_then(|warning| warning.get("code"))
            .and_then(Value::as_str),
        Some("typeshed_source_license_changed")
    );
    assert_eq!(
        warnings
            .get(2)
            .and_then(|warning| warning.get("code"))
            .and_then(Value::as_str),
        Some("typeshed_source_user_managed")
    );
    Ok(())
}

#[test]
fn tool_contract_declares_closed_output_and_honest_annotations() {
    let result = tools_result();
    assert_eq!(
        result
            .pointer("/tools/0/inputSchema/additionalProperties")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        result
            .pointer("/tools/0/outputSchema/additionalProperties")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        result
            .pointer("/tools/0/outputSchema/properties/commit_identity/anyOf/0/pattern")
            .and_then(Value::as_str),
        Some("^[0-9a-f]{40}$")
    );
    assert_eq!(
        result
            .pointer("/tools/0/outputSchema/properties/active_source/enum/0")
            .and_then(Value::as_str),
        Some("custom")
    );
    assert_eq!(
        result
            .pointer("/tools/0/outputSchema/properties/license_status/enum/2")
            .and_then(Value::as_str),
        Some("not supplied")
    );
    assert!(
        result
            .pointer("/tools/0/outputSchema/properties/transport")
            .is_none(),
        "the closed envelope must not resurrect the removed transport field"
    );
    assert!(
        result
            .pointer("/tools/0/outputSchema/properties/signed_release")
            .is_none(),
        "the closed envelope must not resurrect the removed signed_release field"
    );
    assert_eq!(
        result
            .pointer("/tools/0/annotations/readOnlyHint")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        result
            .pointer("/tools/0/annotations/openWorldHint")
            .and_then(Value::as_bool),
        Some(false),
        "status resolution is offline by construction [STUBRES-TYPESHED-OFFLINE] — \
         the tool must declare itself closed-world"
    );
}

#[test]
fn acquisition_failure_is_a_tool_error_without_partial_status() -> Result<(), String> {
    let responses = exchange_with_status(
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":STATUS_TOOL,"arguments":{}}}),
        ],
        || Err("custom typeshed failed without fallback".to_owned()),
    )?;
    let call = responses
        .get(1)
        .ok_or_else(|| "tool error response missing".to_owned())?;
    assert_eq!(
        call.pointer("/result/isError").and_then(Value::as_bool),
        Some(true)
    );
    assert!(call.pointer("/result/structuredContent").is_none());
    assert!(call.pointer("/error").is_none());
    Ok(())
}

#[test]
fn shared_custom_status_projects_to_the_closed_mcp_envelope() {
    use basilisk_stubs::typeshed::source::{
        LicenseStatus, SourceKind, StatusWarning, TypeshedStatus,
    };
    use basilisk_stubs::typeshed::warning::{TypeshedWarning, UnpinnedKind};

    let shared = TypeshedStatus {
        active_source: SourceKind::Custom,
        commit: None,
        tree: None,
        license_status: LicenseStatus::NotSupplied,
        license_reference: None,
        warnings: StatusWarning::list(&[
            TypeshedWarning::UserManaged,
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
        ]),
    };
    let document = status_document(&shared);
    assert_eq!(
        document.get("active_source").and_then(Value::as_str),
        Some("custom")
    );
    assert_eq!(
        document.get("license_status").and_then(Value::as_str),
        Some("not supplied")
    );
    assert!(
        document.get("transport").is_none(),
        "active_source IS the trust story — no transport field may reappear"
    );
    assert!(
        document.get("provenance").is_none(),
        "active_source IS the trust story — no provenance field may reappear"
    );
    assert!(
        document.get("signed_release").is_none(),
        "active_source IS the trust story — no signed_release field may reappear"
    );
    assert!(document.pointer("/warnings/0/severity").is_none());
    assert_eq!(
        document.pointer("/warnings/0/code").and_then(Value::as_str),
        Some("typeshed_source_unpinned")
    );
    assert_eq!(
        document
            .pointer("/warnings/0/docs_url")
            .and_then(Value::as_str),
        Some("https://www.basilisk-python.dev/errors/typeshed_source_unpinned")
    );
}

#[test]
fn shared_oid_type_rejects_truncated_git_identity() {
    assert!(basilisk_stubs::typeshed::gittree::Oid::from_hex("83c2518").is_err());
}

#[test]
fn malformed_and_pre_initialization_requests_are_protocol_errors() -> Result<(), String> {
    let mut lifecycle = Lifecycle::AwaitingInitialize;
    let parse = handle_line("not-json", &mut lifecycle, &|| Ok(status()))
        .ok_or_else(|| "parse error response missing".to_owned())?;
    assert_eq!(
        parse.pointer("/error/code").and_then(Value::as_i64),
        Some(-32700)
    );
    let early = handle_line(
        &serde_json::to_string(&json!({"jsonrpc":"2.0","id":"early","method":"tools/list"}))
            .map_err(|error| error.to_string())?,
        &mut lifecycle,
        &|| Ok(status()),
    )
    .ok_or_else(|| "initialization error response missing".to_owned())?;
    assert_eq!(
        early.pointer("/error/code").and_then(Value::as_i64),
        Some(-32002)
    );
    Ok(())
}

#[test]
fn negotiation_and_notification_order_follow_lifecycle() -> Result<(), String> {
    let mut lifecycle = Lifecycle::AwaitingInitialize;
    let _ = handle_line(
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        &mut lifecycle,
        &|| Ok(status()),
    );
    assert_eq!(lifecycle, Lifecycle::AwaitingInitialize);

    let response = handle_line(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2099-01-01"}}"#,
        &mut lifecycle,
        &|| Ok(status()),
    )
    .ok_or_else(|| "initialize response missing".to_owned())?;
    assert_eq!(
        response
            .pointer("/result/protocolVersion")
            .and_then(Value::as_str),
        Some(PROTOCOL_VERSION)
    );
    assert_eq!(lifecycle, Lifecycle::AwaitingInitialized);
    Ok(())
}

/// [MCP-STDIO]: every malformed request shape gets the prescribed JSON-RPC
/// error — nothing is silently dropped and nothing kills the session.
#[test]
fn malformed_request_shapes_each_get_the_prescribed_error() -> Result<(), String> {
    let responses = exchange(&[
        json!([1, 2, 3]),
        json!({"jsonrpc":"2.0","id":true,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":4}),
        json!({"jsonrpc":"1.0","id":5,"method":"ping"}),
    ])?;
    let codes: Vec<Option<i64>> = responses
        .iter()
        .map(|response| response.pointer("/error/code").and_then(Value::as_i64))
        .collect();
    assert_eq!(codes, vec![Some(-32600); 4]);
    Ok(())
}

/// [MCP-STDIO]: lifecycle guards — initialize without a protocol version,
/// re-initialize, unknown methods, unknown tools, and ping.
#[test]
fn lifecycle_guards_cover_reinit_unknown_methods_and_bad_tools() -> Result<(), String> {
    let responses = exchange(&[
        json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"t","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":3,"method":"resources/list"}),
        json!({"jsonrpc":"2.0","id":4,"method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION}}),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"unknown_tool","arguments":{}}}),
    ])?;
    let codes: Vec<Option<i64>> = responses
        .iter()
        .map(|response| response.pointer("/error/code").and_then(Value::as_i64))
        .collect();
    assert_eq!(
        codes,
        vec![
            Some(-32602),
            None,
            None,
            Some(-32601),
            Some(-32600),
            Some(-32602)
        ]
    );
    assert_eq!(
        responses
            .get(2)
            .and_then(|response| response.pointer("/result")),
        Some(&json!({})),
        "ping must answer with an empty result"
    );
    Ok(())
}

/// [MCP-STDIO]: a line that is not UTF-8 is a parse error, and the session
/// keeps serving afterwards.
#[test]
fn invalid_utf8_input_is_a_parse_error_and_the_session_survives() -> Result<(), String> {
    let mut input: Vec<u8> = vec![0xFF, 0xFE, b'\n'];
    input.extend_from_slice(
        br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
    );
    let mut output = Vec::new();
    run_transport(std::io::Cursor::new(input), &mut output, || Ok(status()))?;
    let responses = String::from_utf8(output)
        .map_err(|error| error.to_string())?
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        responses
            .first()
            .and_then(|response| response.pointer("/error/code"))
            .and_then(Value::as_i64),
        Some(-32700)
    );
    assert!(
        responses
            .get(1)
            .and_then(|response| response.pointer("/result/protocolVersion"))
            .is_some(),
        "the session must keep serving after a non-UTF-8 line"
    );
    Ok(())
}

/// [STUBRES-TYPESHED-WARN]: every license state projects to its wire word.
#[test]
fn status_document_maps_every_license_state() {
    use basilisk_stubs::typeshed::source::{LicenseStatus, SourceKind, TypeshedStatus};
    for (state, expected) in [
        (LicenseStatus::Approved, "approved"),
        (LicenseStatus::Changed, "changed"),
        (LicenseStatus::NotSupplied, "not supplied"),
    ] {
        let document = status_document(&TypeshedStatus {
            active_source: SourceKind::Bundled,
            commit: None,
            tree: None,
            license_status: state,
            license_reference: None,
            warnings: Vec::new(),
        });
        assert_eq!(
            document.get("license_status").and_then(Value::as_str),
            Some(expected)
        );
    }
}

#[test]
fn oversized_line_is_drained_before_the_next_request() -> Result<(), String> {
    let mut input = vec![b' '; MAX_MESSAGE_BYTES + 1];
    input.push(b'\n');
    input.extend_from_slice(
        br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
    );
    let mut output = Vec::new();
    run_transport(std::io::Cursor::new(input), &mut output, || Ok(status()))?;
    let responses = String::from_utf8(output)
        .map_err(|error| error.to_string())?
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        responses
            .first()
            .and_then(|response| response.pointer("/error/code"))
            .and_then(Value::as_i64),
        Some(-32600)
    );
    assert_eq!(
        responses
            .get(1)
            .and_then(|response| response.pointer("/result/protocolVersion"))
            .and_then(Value::as_str),
        Some(PROTOCOL_VERSION)
    );
    Ok(())
}
