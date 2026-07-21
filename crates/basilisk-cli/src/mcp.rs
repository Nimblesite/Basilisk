//! Implements [MCP-TYPESHED-STATUS]. See
//! docs/specs/CHECKER-MCP-SPEC.md#MCP-TYPESHED-STATUS
//!
//! Minimal Model Context Protocol server over stdio. The transport is kept
//! deliberately small: one read-only tool reports the exact typeshed status
//! produced by the shared acquisition subsystem. JSON-RPC messages are one
//! UTF-8 JSON value per line; stdout is reserved exclusively for responses.

use std::path::Path;
use std::sync::OnceLock;

use serde_json::{json, Value};

const PROTOCOL_VERSION: &str = "2025-11-25";
const STATUS_TOOL: &str = "basilisk_typeshed_status";
const MAX_MESSAGE_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    AwaitingInitialize,
    AwaitingInitialized,
    Ready,
}

#[derive(Debug)]
enum IncomingLine {
    Json(String),
    InvalidUtf8,
    TooLarge,
}

/// Run the MCP server on process stdin/stdout for `workspace`.
///
/// # Errors
///
/// Returns a descriptive error when the transport cannot be read or written.
/// Acquisition failures are reported as MCP tool errors so the stdio session
/// remains valid for subsequent protocol requests.
pub(crate) fn run(workspace: &Path) -> Result<(), String> {
    let input = std::io::stdin();
    let output = std::io::stdout();
    let status = OnceLock::new();
    run_transport(input.lock(), output.lock(), || {
        status
            .get_or_init(|| status_for_workspace(workspace))
            .clone()
    })
}

/// Serve requests using injected streams and status provider.
///
/// The seam makes protocol behavior hermetic while production still consumes
/// the same runtime status object as the CLI and LSP.
fn run_transport<R, W, F>(mut reader: R, mut writer: W, status: F) -> Result<(), String>
where
    R: std::io::BufRead,
    W: std::io::Write,
    F: Fn() -> Result<Value, String>,
{
    let mut lifecycle = Lifecycle::AwaitingInitialize;
    while let Some(line) = read_line_limited(&mut reader)? {
        let response = match line {
            IncomingLine::Json(line) => handle_line(&line, &mut lifecycle, &status),
            IncomingLine::InvalidUtf8 => Some(error_response(
                Value::Null,
                -32700,
                "MCP message is not valid UTF-8",
            )),
            IncomingLine::TooLarge => Some(error_response(
                Value::Null,
                -32600,
                "MCP message exceeds 1 MiB",
            )),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut writer, &response)
                .map_err(|error| format!("failed to encode MCP response: {error}"))?;
            writer
                .write_all(b"\n")
                .and_then(|()| writer.flush())
                .map_err(|error| format!("failed to write MCP stdout: {error}"))?;
        }
    }
    Ok(())
}

fn read_line_limited<R: std::io::BufRead>(reader: &mut R) -> Result<Option<IncomingLine>, String> {
    let mut bytes = Vec::new();
    let mut too_large = false;
    loop {
        let available = reader
            .fill_buf()
            .map_err(|error| format!("failed to read MCP stdin: {error}"))?;
        if available.is_empty() {
            if bytes.is_empty() && !too_large {
                return Ok(None);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |position| position + 1);
        if !too_large {
            // Keep at most one byte beyond the content limit. That byte is
            // either the permitted newline or proof that the line is too big.
            let remaining = (MAX_MESSAGE_BYTES + 1).saturating_sub(bytes.len());
            let copied = consumed.min(remaining);
            let prefix = available
                .get(..copied)
                .ok_or_else(|| "MCP input prefix exceeded buffered input".to_owned())?;
            bytes.extend_from_slice(prefix);
            too_large = copied < consumed;
        }
        reader.consume(consumed);
        if newline.is_some() {
            break;
        }
    }

    if bytes.last() == Some(&b'\n') {
        let _ = bytes.pop();
    }
    if bytes.last() == Some(&b'\r') {
        let _ = bytes.pop();
    }
    if too_large || bytes.len() > MAX_MESSAGE_BYTES {
        return Ok(Some(IncomingLine::TooLarge));
    }
    match String::from_utf8(bytes) {
        Ok(line) => Ok(Some(IncomingLine::Json(line))),
        Err(_) => Ok(Some(IncomingLine::InvalidUtf8)),
    }
}

fn handle_line<F>(line: &str, lifecycle: &mut Lifecycle, status: &F) -> Option<Value>
where
    F: Fn() -> Result<Value, String>,
{
    let request: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            return Some(error_response(
                Value::Null,
                -32700,
                &format!("invalid JSON: {error}"),
            ));
        }
    };
    handle_message(&request, lifecycle, status)
}

fn handle_message<F>(request: &Value, lifecycle: &mut Lifecycle, status: &F) -> Option<Value>
where
    F: Fn() -> Result<Value, String>,
{
    let Some(object) = request.as_object() else {
        return Some(error_response(
            Value::Null,
            -32600,
            "request must be an object",
        ));
    };
    let id = object.get("id").cloned();
    if id
        .as_ref()
        .is_some_and(|id| !(id.is_string() || id.is_number() || id.is_null()))
    {
        return Some(error_response(Value::Null, -32600, "invalid request id"));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        let response_id = id.clone().map_or(Value::Null, std::convert::identity);
        return Some(error_response(
            response_id,
            -32600,
            "invalid JSON-RPC request",
        ));
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        let response_id = id.clone().map_or(Value::Null, std::convert::identity);
        return Some(error_response(
            response_id,
            -32600,
            "invalid JSON-RPC request",
        ));
    }
    let Some(id) = id else {
        handle_notification(method, lifecycle);
        return None;
    };
    match method {
        "initialize" => Some(initialize_response(id, object.get("params"), lifecycle)),
        _ if *lifecycle != Lifecycle::Ready => {
            Some(error_response(id, -32002, "server is not initialized"))
        }
        "ping" => Some(success_response(id, json!({}))),
        "tools/list" => Some(success_response(id, tools_result())),
        "tools/call" => Some(call_tool(id, object.get("params"), status)),
        _ => Some(error_response(id, -32601, "method not found")),
    }
}

fn handle_notification(method: &str, lifecycle: &mut Lifecycle) {
    if method == "notifications/initialized" && *lifecycle == Lifecycle::AwaitingInitialized {
        *lifecycle = Lifecycle::Ready;
    }
}

fn initialize_response(id: Value, params: Option<&Value>, lifecycle: &mut Lifecycle) -> Value {
    if *lifecycle != Lifecycle::AwaitingInitialize {
        return error_response(id, -32600, "server is already initialized");
    }
    let requested = params
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str);
    if requested.is_none() {
        return error_response(id, -32602, "protocolVersion is required");
    }
    // If the client requests an unsupported version, MCP requires the server
    // to return a version it does support so the client can decide whether to
    // continue or disconnect.
    *lifecycle = Lifecycle::AwaitingInitialized;
    success_response(
        id,
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": "basilisk",
                "title": "Basilisk Type Checker",
                "version": env!("CARGO_PKG_VERSION"),
                "description": "Read-only Basilisk service status"
            },
            "instructions": "Use basilisk_typeshed_status to inspect the active standard-library source and its status warnings."
        }),
    )
}

fn tools_result() -> Value {
    json!({
        "tools": [{
            "name": STATUS_TOOL,
            "title": "Typeshed source status",
            "description": "Return the active typeshed source, exact commit/tree identities, licensing state, and ordered warnings.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false
            },
            "outputSchema": status_schema(),
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            },
            "execution": { "taskSupport": "forbidden" }
        }]
    })
}

fn status_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "active_source": {
                "type": "string",
                "enum": ["custom", "exact-commit", "bundled"]
            },
            "commit_identity": {
                "anyOf": [
                    { "type": "string", "pattern": "^[0-9a-f]{40}$" },
                    { "type": "null" }
                ]
            },
            "tree_identity": {
                "anyOf": [
                    { "type": "string", "pattern": "^[0-9a-f]{40}$" },
                    { "type": "null" }
                ]
            },
            "license_status": {
                "type": "string",
                "enum": ["approved", "changed", "not supplied"]
            },
            "license_reference": { "type": ["string", "null"] },
            "warnings": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "code": { "type": "string" },
                        "message": { "type": "string" }
                    },
                    "required": ["code", "message"],
                    "additionalProperties": false
                }
            }
        },
        "required": [
            "active_source", "commit_identity", "tree_identity",
            "license_status", "license_reference", "warnings"
        ],
        "additionalProperties": false
    })
}

fn call_tool<F>(id: Value, params: Option<&Value>, status: &F) -> Value
where
    F: Fn() -> Result<Value, String>,
{
    let name = params
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str);
    if name != Some(STATUS_TOOL) || !empty_arguments(params) {
        return error_response(id, -32602, "unknown tool or invalid arguments");
    }
    match status() {
        Ok(document) => match serde_json::to_string(&document) {
            Ok(text) => success_response(
                id,
                json!({
                    "content": [{ "type": "text", "text": text }],
                    "structuredContent": document,
                    "isError": false
                }),
            ),
            Err(error) => error_response(id, -32603, &format!("status encoding failed: {error}")),
        },
        Err(error) => success_response(
            id,
            json!({
                "content": [{ "type": "text", "text": error }],
                "isError": true
            }),
        ),
    }
}

fn empty_arguments(params: Option<&Value>) -> bool {
    params
        .and_then(|params| params.get("arguments"))
        .is_none_or(|arguments| arguments.as_object().is_some_and(serde_json::Map::is_empty))
}

fn success_response(id: Value, result: Value) -> Value {
    Value::Object(serde_json::Map::from_iter([
        ("jsonrpc".to_owned(), Value::String("2.0".to_owned())),
        ("id".to_owned(), id),
        ("result".to_owned(), result),
    ]))
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    let error = serde_json::Map::from_iter([
        ("code".to_owned(), Value::Number(code.into())),
        ("message".to_owned(), Value::String(message.to_owned())),
    ]);
    Value::Object(serde_json::Map::from_iter([
        ("jsonrpc".to_owned(), Value::String("2.0".to_owned())),
        ("id".to_owned(), id),
        ("error".to_owned(), Value::Object(error)),
    ]))
}

/// Resolve the shared runtime status for the MCP tool.
///
/// This adapter is intentionally the only acquisition dependency in the MCP
/// transport; CLI/LSP/MCP therefore serialize one status model and preserve
/// its warning order. [STUBRES-TYPESHED-WARN]
fn status_for_workspace(workspace: &Path) -> Result<Value, String> {
    let config = basilisk_lsp::config::load_analysis_config(workspace);
    let request = basilisk_lsp::config::typeshed_request(&config)?;
    let manager = basilisk_stubs::typeshed::runtime::production_manager(request);
    let status = manager.status().map_err(|error| error.to_string())?;
    Ok(status_document(&status))
}

/// The active source IS the trust story — custom = user-managed, bundled =
/// build-vetted, exact commit = attested at download and re-proven offline —
/// so there are no separate transport/provenance fields to drift out of sync
/// ([STUBRES-TYPESHED-WARN]).
fn status_document(status: &basilisk_stubs::typeshed::source::TypeshedStatus) -> Value {
    let license_status = match status.license_status {
        basilisk_stubs::typeshed::source::LicenseStatus::Approved => "approved",
        basilisk_stubs::typeshed::source::LicenseStatus::Changed => "changed",
        basilisk_stubs::typeshed::source::LicenseStatus::NotSupplied => "not supplied",
    };
    let warnings: Vec<Value> = status
        .warnings
        .iter()
        .map(
            |warning| json!({ "code": warning.code.as_str(), "message": warning.message.as_str() }),
        )
        .collect();
    let commit = status.commit.map(|identity| identity.to_hex());
    let tree = status.tree.map(|identity| identity.to_hex());
    json!({
        "active_source": status.active_source.as_str(),
        "commit_identity": commit.as_deref(),
        "tree_identity": tree.as_deref(),
        "license_status": license_status,
        "license_reference": status.license_reference.as_deref(),
        "warnings": warnings
    })
}

#[cfg(test)]
#[path = "mcp/tests.rs"]
mod tests;
