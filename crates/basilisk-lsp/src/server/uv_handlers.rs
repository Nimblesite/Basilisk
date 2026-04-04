//! LSP command handlers for `basilisk.uv.*` package management commands.
//!
//! Each handler extracts arguments, calls the corresponding function in
//! [`crate::uv_commands`], logs the outcome, and returns a JSON response.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::MessageType;
use tracing::{error, info, warn};

use super::LspServer;

/// Find the project root that contains `pyproject.toml`.
///
/// In monorepo layouts the workspace root (e.g. `ai_cms/`) may not itself
/// contain a `pyproject.toml` — it could be in a subdirectory like
/// `ai_cms/agent-backend/`. We check each workspace root directly, then
/// search one level of subdirectories.
async fn get_workspace_root(server: &LspServer) -> LspResult<std::path::PathBuf> {
    let roots = server.workspace_roots.read().await;
    if roots.is_empty() {
        return Err(tower_lsp::jsonrpc::Error {
            code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32010),
            message: "No workspace root available".into(),
            data: None,
        });
    }

    // Check each workspace root directly.
    for root in roots.iter() {
        if root.join("pyproject.toml").is_file() {
            return Ok(root.clone());
        }
    }

    // Search one level deep for a subdirectory with pyproject.toml.
    for root in roots.iter() {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("pyproject.toml").is_file() {
                    info!(
                        project_root = %path.display(),
                        "found pyproject.toml in subdirectory"
                    );
                    return Ok(path);
                }
            }
        }
    }

    // Fall back to the first workspace root even without pyproject.toml.
    Ok(roots[0].clone())
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

/// Run a uv command and trigger registry rebuild on success.
///
/// After any successful uv command, the lock file may have changed,
/// so we rebuild the package registry and re-resolve all imports.
async fn run_uv_and_refresh<F, Fut>(
    server: &LspServer,
    label: &str,
    command: F,
) -> LspResult<Option<serde_json::Value>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = std::io::Result<crate::uv_commands::UvCommandResult>>,
{
    match command().await {
        Ok(result) => {
            let msg = if result.success {
                format!("{label} completed successfully")
            } else {
                let stderr = result.stderr.trim();
                if stderr.is_empty() {
                    format!("{label} failed")
                } else {
                    format!("{label} failed: {stderr}")
                }
            };
            let msg_type = if result.success {
                MessageType::INFO
            } else {
                MessageType::ERROR
            };
            server
                .client
                .log_message(msg_type, format!("Basilisk: {msg}"))
                .await;

            // Show error to user in the UI (not just the output panel).
            if !result.success {
                server
                    .client
                    .show_message(MessageType::ERROR, format!("Basilisk: {msg}"))
                    .await;
            }

            // Rebuild registry and re-resolve imports on success.
            if result.success {
                super::init::rebuild_registry_and_resolve(server).await;
            }

            Ok(Some(uv_result_to_json(&result)))
        }
        Err(err) => {
            error!(%err, "{label} failed to spawn");
            Err(spawn_error(label, &err))
        }
    }
}

/// Handle `basilisk.uv.sync`.
pub(super) async fn execute_uv_sync(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let root = get_workspace_root(server).await?;
    info!("uv sync requested");
    run_uv_and_refresh(server, "uv sync", || crate::uv_commands::uv_sync(&root)).await
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
    run_uv_and_refresh(server, &format!("uv add {package}"), || {
        crate::uv_commands::uv_add(&root, &package)
    })
    .await
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
    run_uv_and_refresh(server, &format!("uv add --dev {package}"), || {
        crate::uv_commands::uv_add_dev(&root, &package)
    })
    .await
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
    run_uv_and_refresh(server, &format!("uv remove {package}"), || {
        crate::uv_commands::uv_remove(&root, &package)
    })
    .await
}

/// Handle `basilisk.uv.lock`.
pub(super) async fn execute_uv_lock(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let root = get_workspace_root(server).await?;
    info!("uv lock requested");
    run_uv_and_refresh(server, "uv lock", || crate::uv_commands::uv_lock(&root)).await
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
    let pv = python_version.map(String::from);
    info!(python_version = ?pv, "uv venv requested");
    run_uv_and_refresh(server, "uv venv", || {
        crate::uv_commands::uv_create_env(&root, pv.as_deref())
    })
    .await
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test-only code: indexing acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn extract_package_arg_bare_string() {
        let args = vec![serde_json::json!("requests")];
        assert_eq!(extract_package_arg(&args), Some("requests".to_owned()));
    }

    #[test]
    fn extract_package_arg_object_with_package_field() {
        let args = vec![serde_json::json!({"package": "flask"})];
        assert_eq!(extract_package_arg(&args), Some("flask".to_owned()));
    }

    #[test]
    fn extract_package_arg_empty_args() {
        assert_eq!(extract_package_arg(&[]), None);
    }

    #[test]
    fn extract_package_arg_wrong_type() {
        let args = vec![serde_json::json!(42)];
        assert_eq!(extract_package_arg(&args), None);
    }

    #[test]
    fn uv_result_to_json_captures_all_fields() {
        let result = crate::uv_commands::UvCommandResult {
            success: true,
            stdout: "ok".to_owned(),
            stderr: String::new(),
        };
        let json = uv_result_to_json(&result);
        assert_eq!(json["success"], true);
        assert_eq!(json["stdout"], "ok");
        assert_eq!(json["stderr"], "");
    }

    #[test]
    fn spawn_error_includes_label_and_message() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let lsp_err = spawn_error("uv sync", &io_err);
        assert!(lsp_err.message.contains("uv sync"));
        assert!(lsp_err.message.contains("not found"));
    }
}
