//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Execute-command handlers for the Basilisk LSP server.
//!
//! Covers `workspace/executeCommand` dispatch and the individual command
//! implementations: `basilisk.organizeImports`, `basilisk.startDebugSession`,
//! `basilisk.stopDebugSession`, `basilisk.disableRule`, `basilisk.fixFile`,
//! `basilisk.fixWorkspace`, and `basilisk.uv.*` package management commands.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{ExecuteCommandParams, MessageType};
use tracing::{debug, error, info, warn};

use super::LspServer;

/// Log the incoming command for debugging.
async fn log_command(server: &LspServer, params: &ExecuteCommandParams) {
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
}

/// Dispatch `workspace/executeCommand` to the appropriate handler.
pub(super) async fn dispatch_execute_command(
    server: &LspServer,
    params: ExecuteCommandParams,
) -> LspResult<Option<serde_json::Value>> {
    log_command(server, &params).await;

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
        basilisk_common::commands::FIX_FILE => execute_fix_file(server, &params.arguments).await,
        basilisk_common::commands::FIX_WORKSPACE => {
            execute_fix_workspace(server, &params.arguments).await
        }
        basilisk_common::commands::ADOPT_FILE => {
            super::adoption::execute_adopt_file(server, &params.arguments).await
        }
        basilisk_common::commands::ADOPT_WORKSPACE => {
            super::adoption::execute_adopt_workspace(server, &params.arguments).await
        }
        basilisk_common::commands::UNADOPT_FILE => {
            super::adoption::execute_unadopt_file(server, &params.arguments).await
        }
        basilisk_common::commands::UV_SYNC => {
            super::uv_handlers::execute_uv_sync(server, &params.arguments).await
        }
        basilisk_common::commands::UV_ADD => {
            super::uv_handlers::execute_uv_add(server, &params.arguments).await
        }
        basilisk_common::commands::UV_ADD_DEV => {
            super::uv_handlers::execute_uv_add_dev(server, &params.arguments).await
        }
        basilisk_common::commands::UV_REMOVE => {
            super::uv_handlers::execute_uv_remove(server, &params.arguments).await
        }
        basilisk_common::commands::UV_LOCK => {
            super::uv_handlers::execute_uv_lock(server, &params.arguments).await
        }
        basilisk_common::commands::UV_CREATE_ENV => {
            super::uv_handlers::execute_uv_create_env(server, &params.arguments).await
        }
        basilisk_common::commands::MOVE_SYMBOL => {
            super::refactor_commands::execute_move_symbol(server, &params.arguments).await
        }
        basilisk_common::commands::DISCOVER_TESTS => {
            super::test_handlers::execute_discover_tests(server, &params.arguments).await
        }
        basilisk_common::commands::RUN_TESTS => {
            super::test_handlers::execute_run_tests(server, &params.arguments).await
        }
        basilisk_common::commands::RUN_TEST_FILE => {
            super::test_handlers::execute_run_test_file(server, &params.arguments).await
        }
        basilisk_common::commands::DEBUG_TEST => {
            super::test_handlers::execute_debug_test(server, &params.arguments).await
        }
        basilisk_common::commands::RUN_TESTS_COVERAGE => {
            super::test_handlers::execute_run_tests_coverage(server, &params.arguments).await
        }
        basilisk_common::commands::WORKSPACE_MODULES => {
            super::activity_panel::execute_workspace_modules(server, &params.arguments).await
        }
        basilisk_common::commands::TYPE_HEALTH => {
            super::activity_panel::execute_type_health(server, &params.arguments).await
        }
        basilisk_common::commands::PROFILER_START
        | basilisk_common::commands::PROFILER_STOP
        | basilisk_common::commands::PROFILER_SNAPSHOT
        | basilisk_common::commands::PROFILER_LIST
        | basilisk_common::commands::PROFILER_PROCESSES
        | basilisk_common::commands::MEMORY_START
        | basilisk_common::commands::MEMORY_SNAPSHOT
        | basilisk_common::commands::MEMORY_DIFF
        | basilisk_common::commands::MEMORY_REFERENCES
        | basilisk_common::commands::MEMORY_OBJECTS_BY_TYPE
        | basilisk_common::commands::MEMORY_GC_COLLECT
        | basilisk_common::commands::MEMORY_INGEST => {
            dispatch_profiler_or_memory(server, &params.command, &params.arguments).await
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

/// Dispatch profiler and memory commands to their respective handlers.
async fn dispatch_profiler_or_memory(
    server: &LspServer,
    command: &str,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    match command {
        basilisk_common::commands::PROFILER_START => {
            super::profiler_handlers::execute_profiler_start(server, args).await
        }
        basilisk_common::commands::PROFILER_STOP => {
            super::profiler_handlers::execute_profiler_stop(server, args).await
        }
        basilisk_common::commands::PROFILER_SNAPSHOT => {
            super::profiler_handlers::execute_profiler_snapshot(server, args).await
        }
        basilisk_common::commands::PROFILER_LIST => {
            super::profiler_handlers::execute_profiler_list(server, args).await
        }
        basilisk_common::commands::PROFILER_PROCESSES => {
            super::profiler_handlers::execute_profiler_processes(server, args).await
        }
        basilisk_common::commands::MEMORY_START => {
            super::memory_handlers::execute_memory_start(server, args).await
        }
        basilisk_common::commands::MEMORY_SNAPSHOT => {
            super::memory_handlers::execute_memory_snapshot(server, args).await
        }
        basilisk_common::commands::MEMORY_DIFF => {
            super::memory_handlers::execute_memory_diff(server, args).await
        }
        basilisk_common::commands::MEMORY_REFERENCES => {
            super::memory_handlers::execute_memory_references(server, args).await
        }
        basilisk_common::commands::MEMORY_OBJECTS_BY_TYPE => {
            super::memory_handlers::execute_memory_objects_by_type(server, args).await
        }
        basilisk_common::commands::MEMORY_GC_COLLECT => {
            super::memory_handlers::execute_memory_gc_collect(server, args).await
        }
        basilisk_common::commands::MEMORY_INGEST => {
            super::memory_handlers::execute_memory_ingest(server, args).await
        }
        _ => Ok(None),
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
    // Extract optional python interpreter override from args. A blank override
    // (empty `basilisk.python.interpreterPath`) is treated as absent so we fall
    // back to auto-detection instead of spawning `""` (os error 2).
    let python_override = args
        .first()
        .and_then(|v| v.get("python"))
        .and_then(|v| v.as_str());

    let workspace = server.workspace_roots.read().await;
    let root = workspace
        .first()
        .map_or_else(|| std::path::Path::new("."), std::path::PathBuf::as_path);
    let python = crate::debug::effective_python(python_override, root);
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

    let arg = args
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let profile_on_launch = arg
        .get("profileOnLaunch")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

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

            // If a debuggee PID is already known (attach mode), auto-profile.
            let profiler_session_id = if let Some(debuggee_pid) = extract_debuggee_pid(&arg) {
                super::profiler_handlers::maybe_profile_on_launch(server, &arg, debuggee_pid).await
            } else {
                None
            };

            Ok(Some(serde_json::json!({
                "host": host,
                "port": port,
                "sessionId": session_id,
                "profileOnLaunch": profile_on_launch,
                "profilerSessionId": profiler_session_id,
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

/// Extract a debuggee PID from the debug session args (attach mode).
fn extract_debuggee_pid(arg: &serde_json::Value) -> Option<u32> {
    arg.get("pid")
        .and_then(serde_json::Value::as_u64)
        .and_then(|p| u32::try_from(p).ok())
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

    let updated = super::rule_override::insert_rule_override(&existing, rule, severity);

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

/// Handle `basilisk.fixFile`.
///
/// Collects all fixable diagnostics for the given file URI, resolves conflicts,
/// and applies a single `WorkspaceEdit` so the user can undo in one step.
async fn execute_fix_file(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(uri_str) = args.first().and_then(|v| v.as_str()) else {
        return Ok(None);
    };
    let Ok(uri) = tower_lsp::lsp_types::Url::parse(uri_str) else {
        return Ok(None);
    };

    let action: Option<tower_lsp::lsp_types::CodeAction> = server
        .with_index(|idx| {
            let (text, _, checker_diags) = idx.get_by_uri(&uri)?;
            let lsp_diags: Vec<_> = checker_diags
                .iter()
                .map(|d| crate::workspace_analysis::bsk_to_lsp(d, &text))
                .collect();
            crate::code_actions::fix_all_in_file(&uri, &lsp_diags, &text)
        })
        .await;

    let Some(action) = action else {
        warn!(uri = %uri, "fixFile: no fixable diagnostics");
        return Ok(Some(serde_json::json!({ "fixed": 0 })));
    };

    let edit_count: usize = action
        .edit
        .as_ref()
        .and_then(|e| e.changes.as_ref())
        .map_or(0, |changes| changes.values().map(Vec::len).sum::<usize>());

    if let Some(edit) = action.edit {
        match server.client.apply_edit(edit).await {
            Ok(response) => {
                if response.applied {
                    info!(uri = %uri, edit_count, "fixFile: edits applied successfully");
                    // Clear stale diagnostics immediately — the client confirmed
                    // the edit was applied but textDocument/didChange may arrive
                    // after this response, leaving old diagnostics visible.
                    server
                        .client
                        .publish_diagnostics(uri.clone(), vec![], None)
                        .await;
                } else {
                    error!(
                        uri = %uri,
                        edit_count,
                        failure_reason = response.failure_reason.as_deref().unwrap_or("unknown"),
                        "fixFile: client REJECTED the workspace edit"
                    );
                }
            }
            Err(err) => {
                error!(uri = %uri, error = %err, "fixFile: apply_edit request failed");
            }
        }
    }

    info!(uri = %uri, edit_count, "fixFile: completed");
    server
        .client
        .log_message(
            tower_lsp::lsp_types::MessageType::INFO,
            format!("Basilisk: fixed {edit_count} issues in {uri}"),
        )
        .await;

    Ok(Some(serde_json::json!({ "fixed": edit_count })))
}

/// Handle `basilisk.fixWorkspace`.
///
/// Iterates all files in the workspace index, collects fixable diagnostics,
/// and applies a single `WorkspaceEdit` so the user can undo in one step.
async fn execute_fix_workspace(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("fixWorkspace: starting");

    let file_edits: Option<
        Vec<(
            tower_lsp::lsp_types::Url,
            Vec<tower_lsp::lsp_types::TextEdit>,
        )>,
    > = server
        .with_index(|idx| {
            let mut all_edits = Vec::new();
            for entry in &idx.files {
                let path = entry.key();
                let file_entry = entry.value();

                let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(path) else {
                    continue;
                };

                if file_entry.diagnostics.is_empty() {
                    continue;
                }

                let lsp_diags: Vec<_> = file_entry
                    .diagnostics
                    .iter()
                    .map(|d| crate::workspace_analysis::bsk_to_lsp(d, &file_entry.text))
                    .collect();

                let Some(action) =
                    crate::code_actions::fix_all_in_file(&uri, &lsp_diags, &file_entry.text)
                else {
                    continue;
                };

                let Some(edit) = action.edit else {
                    continue;
                };

                let Some(changes) = edit.changes else {
                    continue;
                };

                for (change_uri, edits) in changes {
                    if !edits.is_empty() {
                        all_edits.push((change_uri, edits));
                    }
                }
            }
            Some(all_edits)
        })
        .await;

    let Some(file_edits) = file_edits else {
        warn!("fixWorkspace: workspace index not available");
        return Ok(Some(serde_json::json!({ "fixed": 0, "files": 0 })));
    };

    if file_edits.is_empty() {
        info!("fixWorkspace: no fixable diagnostics found");
        return Ok(Some(serde_json::json!({ "fixed": 0, "files": 0 })));
    }

    let file_count = file_edits.len();
    let total_edit_count: usize = file_edits.iter().map(|(_, edits)| edits.len()).sum();

    // Combine all per-file edits into a single WorkspaceEdit for atomic undo.
    let mut changes = std::collections::HashMap::new();
    for (uri, edits) in file_edits {
        changes.entry(uri).or_insert_with(Vec::new).extend(edits);
    }

    let workspace_edit = tower_lsp::lsp_types::WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    };

    match server.client.apply_edit(workspace_edit).await {
        Ok(response) => {
            if response.applied {
                info!(
                    total_edit_count,
                    file_count, "fixWorkspace: edits applied successfully"
                );
            } else {
                error!(
                    total_edit_count,
                    file_count,
                    failure_reason = response.failure_reason.as_deref().unwrap_or("unknown"),
                    "fixWorkspace: client REJECTED the workspace edit"
                );
            }
        }
        Err(err) => {
            error!(error = %err, "fixWorkspace: apply_edit request failed");
        }
    }

    server
        .client
        .log_message(
            tower_lsp::lsp_types::MessageType::INFO,
            format!("Basilisk: fixed {total_edit_count} issues across {file_count} file(s)"),
        )
        .await;

    Ok(Some(
        serde_json::json!({ "fixed": total_edit_count, "files": file_count }),
    ))
}
