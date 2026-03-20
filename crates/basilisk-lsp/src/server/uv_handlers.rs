//! LSP command handlers for `basilisk.uv.*` package management commands.
//!
//! Each handler extracts arguments, calls the corresponding function in
//! [`crate::uv_commands`], logs the outcome, and returns a JSON response.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::MessageType;
use tracing::{error, info, warn};

use super::LspServer;

/// Extract the first workspace root, returning an LSP error if none is available.
async fn get_workspace_root(server: &LspServer) -> LspResult<std::path::PathBuf> {
    let roots = server.workspace_roots.read().await;
    roots
        .first()
        .cloned()
        .ok_or_else(|| tower_lsp::jsonrpc::Error {
            code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32010),
            message: "No workspace root available".into(),
            data: None,
        })
}

/// Convert a [`crate::uv_commands::UvCommandResult`] into a JSON response value.
fn uv_result_to_json(result: &crate::uv_commands::UvCommandResult) -> serde_json::Value {
    serde_json::json!({
        "success": result.success,
        "stdout": result.stdout,
        "stderr": result.stderr,
    })
}

/// Extract a package name string from the first command argument.
///
/// Accepts either a bare string `"requests"` or an object `{"package": "requests"}`.
fn extract_package_arg(args: &[serde_json::Value]) -> Option<String> {
    args.first().and_then(|v| {
        v.as_str()
            .map(String::from)
            .or_else(|| v.get("package").and_then(|p| p.as_str()).map(String::from))
    })
}

/// Construct a standard LSP spawn-failure error.
fn spawn_error(label: &str, err: &std::io::Error) -> tower_lsp::jsonrpc::Error {
    tower_lsp::jsonrpc::Error {
        code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32011),
        message: format!("{label} failed: {err}").into(),
        data: None,
    }
}

/// Handle `basilisk.uv.sync`.
pub(super) async fn execute_uv_sync(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let root = get_workspace_root(server).await?;
    info!("uv sync requested");
    match crate::uv_commands::uv_sync(&root).await {
        Ok(result) => {
            let msg = if result.success {
                "uv sync completed successfully"
            } else {
                "uv sync failed"
            };
            server
                .client
                .log_message(MessageType::INFO, format!("Basilisk: {msg}"))
                .await;
            Ok(Some(uv_result_to_json(&result)))
        }
        Err(err) => {
            error!(%err, "uv sync failed to spawn");
            Err(spawn_error("uv sync", &err))
        }
    }
}

/// Handle `basilisk.uv.add`.
pub(super) async fn execute_uv_add(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let root = get_workspace_root(server).await?;
    let Some(package) = extract_package_arg(args) else {
        warn!("uv add: missing package argument");
        return Ok(None);
    };
    info!(package = %package, "uv add requested");
    match crate::uv_commands::uv_add(&root, &package).await {
        Ok(result) => {
            let msg = if result.success {
                format!("uv add {package} completed successfully")
            } else {
                format!("uv add {package} failed")
            };
            server
                .client
                .log_message(MessageType::INFO, format!("Basilisk: {msg}"))
                .await;
            Ok(Some(uv_result_to_json(&result)))
        }
        Err(err) => {
            error!(%err, "uv add failed to spawn");
            Err(spawn_error("uv add", &err))
        }
    }
}

/// Handle `basilisk.uv.addDev`.
pub(super) async fn execute_uv_add_dev(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let root = get_workspace_root(server).await?;
    let Some(package) = extract_package_arg(args) else {
        warn!("uv addDev: missing package argument");
        return Ok(None);
    };
    info!(package = %package, "uv add --dev requested");
    match crate::uv_commands::uv_add_dev(&root, &package).await {
        Ok(result) => {
            let msg = if result.success {
                format!("uv add --dev {package} completed successfully")
            } else {
                format!("uv add --dev {package} failed")
            };
            server
                .client
                .log_message(MessageType::INFO, format!("Basilisk: {msg}"))
                .await;
            Ok(Some(uv_result_to_json(&result)))
        }
        Err(err) => {
            error!(%err, "uv add --dev failed to spawn");
            Err(spawn_error("uv add --dev", &err))
        }
    }
}

/// Handle `basilisk.uv.remove`.
pub(super) async fn execute_uv_remove(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let root = get_workspace_root(server).await?;
    let Some(package) = extract_package_arg(args) else {
        warn!("uv remove: missing package argument");
        return Ok(None);
    };
    info!(package = %package, "uv remove requested");
    match crate::uv_commands::uv_remove(&root, &package).await {
        Ok(result) => {
            let msg = if result.success {
                format!("uv remove {package} completed successfully")
            } else {
                format!("uv remove {package} failed")
            };
            server
                .client
                .log_message(MessageType::INFO, format!("Basilisk: {msg}"))
                .await;
            Ok(Some(uv_result_to_json(&result)))
        }
        Err(err) => {
            error!(%err, "uv remove failed to spawn");
            Err(spawn_error("uv remove", &err))
        }
    }
}

/// Handle `basilisk.uv.lock`.
pub(super) async fn execute_uv_lock(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let root = get_workspace_root(server).await?;
    info!("uv lock requested");
    match crate::uv_commands::uv_lock(&root).await {
        Ok(result) => {
            let msg = if result.success {
                "uv lock completed successfully"
            } else {
                "uv lock failed"
            };
            server
                .client
                .log_message(MessageType::INFO, format!("Basilisk: {msg}"))
                .await;
            Ok(Some(uv_result_to_json(&result)))
        }
        Err(err) => {
            error!(%err, "uv lock failed to spawn");
            Err(spawn_error("uv lock", &err))
        }
    }
}

/// Handle `basilisk.uv.createEnv`.
pub(super) async fn execute_uv_create_env(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let root = get_workspace_root(server).await?;
    let python_version = args.first().and_then(|v| {
        v.as_str()
            .or_else(|| v.get("python").and_then(|p| p.as_str()))
    });
    info!(python_version = ?python_version, "uv venv requested");
    match crate::uv_commands::uv_create_env(&root, python_version).await {
        Ok(result) => {
            let msg = if result.success {
                "uv venv completed successfully"
            } else {
                "uv venv failed"
            };
            server
                .client
                .log_message(MessageType::INFO, format!("Basilisk: {msg}"))
                .await;
            Ok(Some(uv_result_to_json(&result)))
        }
        Err(err) => {
            error!(%err, "uv venv failed to spawn");
            Err(spawn_error("uv venv", &err))
        }
    }
}
