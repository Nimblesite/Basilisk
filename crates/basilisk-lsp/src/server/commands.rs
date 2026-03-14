//! Execute-command handlers for the Basilisk LSP server.
//!
//! Covers `workspace/executeCommand` dispatch and the individual command
//! implementations: `basilisk.organizeImports`, `basilisk.startDebugSession`,
//! `basilisk.stopDebugSession`, and `basilisk.disableRule`.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{ExecuteCommandParams, MessageType};
use tracing::{debug, error, info, warn};

use super::LspServer;

/// Dispatch `workspace/executeCommand` to the appropriate handler.
pub(super) async fn dispatch_execute_command(
    server: &LspServer,
    params: ExecuteCommandParams,
) -> LspResult<Option<serde_json::Value>> {
    server
        .client
        .log_message(
            MessageType::INFO,
            format!(
                "Basilisk: execute_command '{}' with {} arg(s)",
                params.command,
                params.arguments.len()
            ),
        )
        .await;

    match params.command.as_str() {
        basilisk_common::commands::ORGANIZE_IMPORTS => {
            execute_organize_imports(server, &params.arguments).await
        }
        basilisk_common::commands::START_DEBUG_SESSION => {
            execute_start_debug_session(server, &params.arguments).await
        }
        basilisk_common::commands::STOP_DEBUG_SESSION => {
            execute_stop_debug_session(server, &params.arguments).await
        }
        basilisk_common::commands::DISABLE_RULE => {
            execute_disable_rule(server, &params.arguments).await
        }
        unknown => {
            server
                .client
                .log_message(
                    MessageType::WARNING,
                    format!("Basilisk: unknown command '{unknown}'"),
                )
                .await;
            Ok(None)
        }
    }
}

/// Handle `basilisk.organizeImports`.
async fn execute_organize_imports(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(uri_value) = args.first() else {
        return Ok(None);
    };
    let Some(uri_str) = uri_value.as_str() else {
        return Ok(None);
    };
    let Ok(uri) = tower_lsp::lsp_types::Url::parse(uri_str) else {
        return Ok(None);
    };

    let source = server
        .with_index(|idx| idx.get_text(&uri))
        .await
        .unwrap_or_default();

    if source.is_empty() {
        return Ok(None);
    }

    let Some(action) = crate::code_actions::organize_imports(&uri, &source) else {
        return Ok(None);
    };

    if let Some(edit) = action.edit {
        let _ = server.client.apply_edit(edit).await;
    }

    Ok(None)
}

/// Handle `basilisk.startDebugSession`.
///
/// Spawns debugpy on a free TCP port and returns the connection details.
/// The editor connects its DAP client directly to that port.
pub(super) async fn execute_start_debug_session(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_start_debug_session called");
    // Extract optional python interpreter override from args.
    let python_override = args
        .first()
        .and_then(|v| v.get("python"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let workspace = server.workspace_roots.read().await;
    let root = workspace
        .first()
        .map_or_else(|| std::path::Path::new("."), std::path::PathBuf::as_path);
    let python = python_override.unwrap_or_else(|| crate::debug::resolve_python(root));
    drop(workspace);

    debug!(python = %python, "resolved python interpreter");

    // Verify debugpy is installed.
    if let Err(err) = crate::debug::check_debugpy(&python).await {
        error!(python = %python, %err, "debugpy check failed");
        server
            .client
            .log_message(MessageType::ERROR, err.to_string())
            .await;
        return Err(tower_lsp::jsonrpc::Error {
            code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32001),
            message: err.to_string().into(),
            data: None,
        });
    }

    // Spawn debugpy and wait for it to accept connections.
    match server.debug_manager.start_session(&python).await {
        Ok((host, port, session_id)) => {
            info!(host = %host, port, session_id = %session_id, "debug session started");
            server
                .client
                .log_message(
                    MessageType::INFO,
                    format!("Basilisk: debug session {session_id} started on {host}:{port}"),
                )
                .await;
            Ok(Some(serde_json::json!({
                "host": host,
                "port": port,
                "sessionId": session_id
            })))
        }
        Err(err) => {
            error!(%err, "failed to start debug session");
            server
                .client
                .log_message(MessageType::ERROR, err.to_string())
                .await;
            Err(tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32002),
                message: err.to_string().into(),
                data: None,
            })
        }
    }
}

/// Handle `basilisk.stopDebugSession`.
pub(super) async fn execute_stop_debug_session(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let session_id = args
        .first()
        .and_then(|v| v.get("sessionId"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    info!(session_id, "execute_stop_debug_session called");
    let stopped = server.debug_manager.stop_session(session_id).await;

    if stopped {
        info!(session_id, "debug session stopped successfully");
        server
            .client
            .log_message(
                MessageType::INFO,
                format!("Basilisk: debug session {session_id} stopped"),
            )
            .await;
    } else {
        warn!(session_id, "stop_session: no such session");
    }

    Ok(Some(serde_json::json!({ "stopped": stopped })))
}

/// Handle `basilisk.disableRule`.
///
/// Reads the workspace root's `pyproject.toml`, inserts/updates a rule entry
/// under `[tool.basilisk.rules]`, and applies the edit via `workspace/applyEdit`.
async fn execute_disable_rule(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(arg) = args.first() else {
        return Ok(None);
    };
    let Some(rule) = arg.get("rule").and_then(|v| v.as_str()) else {
        warn!("disableRule: missing 'rule' argument");
        return Ok(None);
    };
    let severity = arg
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("off");

    info!(rule, severity, "disableRule requested");

    let roots = server.workspace_roots.read().await;
    let Some(root) = roots.first() else {
        warn!("disableRule: no workspace root available");
        return Ok(None);
    };

    let pyproject_path = root.join("pyproject.toml");
    let existing = std::fs::read_to_string(&pyproject_path).unwrap_or_default();

    let updated = insert_rule_override(&existing, rule, severity);

    if let Err(err) = std::fs::write(&pyproject_path, updated.as_bytes()) {
        error!(%err, "failed to write pyproject.toml");
        server
            .client
            .log_message(
                MessageType::ERROR,
                format!("Basilisk: failed to write pyproject.toml: {err}"),
            )
            .await;
        return Err(tower_lsp::jsonrpc::Error {
            code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32003),
            message: format!("Failed to write pyproject.toml: {err}").into(),
            data: None,
        });
    }

    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: disabled rule {rule} in pyproject.toml"),
        )
        .await;

    Ok(Some(serde_json::json!({
        "rule": rule,
        "severity": severity,
        "path": pyproject_path.display().to_string(),
    })))
}

/// Insert or update a rule override in `pyproject.toml` content.
///
/// Adds `[tool.basilisk.rules]` section if missing, then sets `RULE = "severity"`.
fn insert_rule_override(content: &str, rule: &str, severity: &str) -> String {
    let section_header = "[tool.basilisk.rules]";
    let entry = format!("{rule} = \"{severity}\"");

    // Check if the rule already exists — update in place.
    if content.contains(section_header) {
        let rule_pattern = format!("{rule} = ");
        if content.contains(&rule_pattern) {
            // Replace existing rule line.
            let mut result = String::with_capacity(content.len());
            for line in content.lines() {
                if line.trim_start().starts_with(&rule_pattern) {
                    result.push_str(&entry);
                } else {
                    result.push_str(line);
                }
                result.push('\n');
            }
            return result;
        }
        // Section exists but rule doesn't — append after section header.
        let mut result = String::with_capacity(content.len() + entry.len() + 2);
        for line in content.lines() {
            result.push_str(line);
            result.push('\n');
            if line.trim() == section_header {
                result.push_str(&entry);
                result.push('\n');
            }
        }
        return result;
    }

    // No section at all — append at end.
    let mut result = content.to_owned();
    if !result.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }
    result.push('\n');
    result.push_str(section_header);
    result.push('\n');
    result.push_str(&entry);
    result.push('\n');
    result
}
