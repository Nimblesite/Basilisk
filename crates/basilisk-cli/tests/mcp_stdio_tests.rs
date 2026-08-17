//! End-to-end tests for [MCP-STDIO] / [MCP-TYPESHED-STATUS].
//! See docs/specs/CHECKER-MCP-SPEC.md.

use std::io::Write as _;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

const PROTOCOL_VERSION: &str = "2025-11-25";

fn custom_typeshed_workspace() -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let workspace = tempfile::tempdir()?;
    let stdlib = workspace.path().join("typeshed").join("stdlib");
    std::fs::create_dir_all(&stdlib)?;
    std::fs::write(stdlib.join("VERSIONS"), "os: 3.8-\n")?;
    std::fs::write(stdlib.join("os.pyi"), "def getcwd() -> str: ...\n")?;
    std::fs::write(
        workspace.path().join("pyproject.toml"),
        "[tool.basilisk]\ntypeshed-path = \"typeshed\"\n",
    )?;
    Ok(workspace)
}

fn run_session(workspace: &std::path::Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"e2e","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"basilisk_typeshed_status","arguments":{}}}),
    ];
    let mut payload = Vec::new();
    for request in requests {
        payload.extend_from_slice(serde_json::to_string(&request)?.as_bytes());
        payload.push(b'\n');
    }
    run_raw_session(workspace, &payload)
}

/// Drive one MCP session over the spawned binary's stdio with raw bytes, so
/// tests can send lines no serializer would produce (invalid UTF-8, arrays).
fn run_raw_session(
    workspace: &std::path::Path,
    payload: &[u8],
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("mcp")
        .arg("--workspace")
        .arg(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("MCP stdin was not piped")?;
    stdin.write_all(payload)?;
    drop(stdin);
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "MCP exited {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    String::from_utf8(output.stdout)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

#[test]
fn stdio_server_lists_and_returns_ordered_structured_status(
) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = custom_typeshed_workspace()?;
    let responses = run_session(workspace.path())?;
    assert_eq!(
        responses.len(),
        3,
        "notifications must not receive responses"
    );
    let initialize = responses.first().ok_or("initialize response missing")?;
    let list = responses.get(1).ok_or("tools/list response missing")?;
    let call = responses.get(2).ok_or("tools/call response missing")?;
    assert_eq!(
        initialize
            .pointer("/result/protocolVersion")
            .and_then(Value::as_str),
        Some(PROTOCOL_VERSION)
    );
    assert_eq!(
        list.pointer("/result/tools/0/name").and_then(Value::as_str),
        Some("basilisk_typeshed_status")
    );
    let status = call
        .pointer("/result/structuredContent")
        .ok_or("structuredContent missing")?;
    assert_eq!(
        status.get("active_source").and_then(Value::as_str),
        Some("custom")
    );
    assert_eq!(
        status.get("license_status").and_then(Value::as_str),
        Some("not supplied")
    );
    assert!(
        status.get("provenance").is_none(),
        "active_source IS the trust story — no provenance field may reappear"
    );
    assert!(
        status.get("signed_release").is_none(),
        "active_source IS the trust story — no signed_release field may reappear"
    );
    let warnings = status
        .get("warnings")
        .and_then(Value::as_array)
        .ok_or("ordered warnings missing")?;
    assert_eq!(
        warnings
            .first()
            .and_then(|warning| warning.get("code"))
            .and_then(Value::as_str),
        Some("typeshed_source_unpinned")
    );
    assert!(
        warnings.iter().any(|warning| {
            warning.get("code").and_then(Value::as_str) == Some("typeshed_source_user_managed")
        }),
        "custom status must disclose user-managed contents and terms: {warnings:?}"
    );
    assert_eq!(
        call.pointer("/result/isError").and_then(Value::as_bool),
        Some(false)
    );
    Ok(())
}

#[test]
fn stdio_server_emits_only_json_rpc_on_stdout() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = custom_typeshed_workspace()?;
    for response in run_session(workspace.path())? {
        assert_eq!(response.get("jsonrpc").and_then(Value::as_str), Some("2.0"));
        assert!(response.get("id").is_some());
    }
    Ok(())
}

#[test]
fn packaged_binary_advertises_mcp_capability() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .args(["--version", "--json"])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "version command exited {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let version: Value = serde_json::from_slice(&output.stdout)?;
    let capabilities = version
        .get("capabilities")
        .and_then(Value::as_array)
        .ok_or("Shipwright capabilities missing")?;
    assert!(
        capabilities
            .iter()
            .any(|entry| entry.as_str() == Some("mcp")),
        "packaged basilisk binary must advertise MCP: {version}"
    );
    Ok(())
}

/// [MCP-STDIO]: the spawned binary answers every malformed request shape with
/// the prescribed JSON-RPC error and keeps the session alive throughout —
/// invalid UTF-8, non-object requests, bad ids, missing methods, wrong
/// protocol versions, re-initialization, and unknown tools.
#[test]
fn protocol_guard_paths_respond_and_the_session_survives() -> Result<(), Box<dyn std::error::Error>>
{
    let workspace = custom_typeshed_workspace()?;
    let mut payload: Vec<u8> = vec![0xFF, 0xFE, b'\n'];
    for request in [
        json!([1]),
        json!({"jsonrpc":"2.0","id":true,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":1}),
        json!({"jsonrpc":"1.0","id":2,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":3,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":4,"method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"e2e","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":5,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":6,"method":"resources/list"}),
        json!({"jsonrpc":"2.0","id":7,"method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION}}),
        json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"nope","arguments":{}}}),
    ] {
        payload.extend_from_slice(serde_json::to_string(&request)?.as_bytes());
        payload.push(b'\n');
    }
    let responses = run_raw_session(workspace.path(), &payload)?;
    let codes: Vec<Option<i64>> = responses
        .iter()
        .map(|response| response.pointer("/error/code").and_then(Value::as_i64))
        .collect();
    assert_eq!(
        codes,
        vec![
            Some(-32700),
            Some(-32600),
            Some(-32600),
            Some(-32600),
            Some(-32600),
            Some(-32602),
            None,
            None,
            Some(-32601),
            Some(-32600),
            Some(-32602),
        ],
        "each guard must answer with its prescribed code: {responses:?}"
    );
    Ok(())
}
