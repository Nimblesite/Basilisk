//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Execute-command handlers for the Basilisk LSP server.
//!
//! Covers `workspace/executeCommand` dispatch and the individual command
//! implementations: `basilisk.organizeImports`, `basilisk.startDebugSession`,
//! `basilisk.stopDebugSession`, `basilisk.disableRule`, `basilisk.fixFile`,
//! `basilisk.fixFileAll`, `basilisk.fixWorkspace`, `basilisk.fixWorkspaceAll`,
//! and `basilisk.uv.*` package management commands.

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

/// Whether `cmd` is one of the `basilisk.stubs.*` commands.
fn is_stub_command(cmd: &str) -> bool {
    matches!(
        cmd,
        basilisk_common::commands::STUBS_CREATE_LOCAL | basilisk_common::commands::STUBS_ADD_MEMBER
    )
}

/// Dispatch the `basilisk.stubs.*` family (create local stub, add member).
async fn dispatch_stub_command(
    server: &LspServer,
    cmd: &str,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    match cmd {
        basilisk_common::commands::STUBS_CREATE_LOCAL => {
            super::stub_handlers::execute_create_local_stub(server, args).await
        }
        basilisk_common::commands::STUBS_ADD_MEMBER => {
            super::stub_handlers::execute_add_stub_member(server, args).await
        }
        _ => Ok(None),
    }
}

// Implements [LSPARCH-CMDS]
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
        // Implements [LSPDEBUG-WIRE] (handle debug commands in execute_command dispatch)
        basilisk_common::commands::START_DEBUG_SESSION => {
            execute_start_debug_session(server, &params.arguments).await
        }
        basilisk_common::commands::STOP_DEBUG_SESSION => {
            execute_stop_debug_session(server, &params.arguments).await
        }
        basilisk_common::commands::DISABLE_RULE => {
            super::command_configuration::execute_disable_rule(server, &params.arguments).await
        }
        cmd if super::command_fixes::is_fix_command(cmd) => {
            super::command_fixes::dispatch(server, cmd, &params.arguments).await
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
        cmd if is_stub_command(cmd) => dispatch_stub_command(server, cmd, &params.arguments).await,
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
        | basilisk_common::commands::PROFILER_COOPERATIVE_SCRIPT
        | basilisk_common::commands::PROFILER_COOPERATIVE_ATTACH
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
        basilisk_common::commands::PROFILER_COOPERATIVE_SCRIPT => {
            super::profiler_handlers::execute_profiler_cooperative_script(args)
        }
        basilisk_common::commands::PROFILER_COOPERATIVE_ATTACH => {
            super::profiler_handlers::execute_profiler_cooperative_attach(server, args).await
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

// Implements [LSPARCH-FEATURES-EXECCMD]
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
    if let Err(err) = server.debug_manager.ensure_debugpy(&python).await {
        error!(python = %python, %err, "debugpy check failed");
        server
            .client
            .log_message(MessageType::ERROR, err.to_string())
            .await;
        return Err(tower_lsp::jsonrpc::Error {
            code: tower_lsp::jsonrpc::ErrorCode::ServerError(err.jsonrpc_code()),
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
                code: tower_lsp::jsonrpc::ErrorCode::ServerError(err.jsonrpc_code()),
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
