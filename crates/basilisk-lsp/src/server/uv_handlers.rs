//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! LSP command handlers for `basilisk.uv.*` package management commands.
//!
//! Each handler extracts arguments, calls the corresponding function in
//! [`crate::uv_commands`], logs the outcome, and returns a JSON response.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::MessageType;
use tracing::{error, info, warn};

use super::LspServer;

/// Find a uv project root suitable for running `uv` commands.
///
/// Returns `Some(root)` only when a `pyproject.toml` is actually found at one
/// of the workspace roots, or one level deep inside one. Returns `None` when
/// no `pyproject.toml` exists anywhere — the caller MUST treat this as
/// "no uv project" and no-op gracefully rather than running `uv sync` from a
/// directory without a project file (which fails with the misleading
/// "No `pyproject.toml` found" error reported in issue #23).
fn find_uv_project_root(roots: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    // Direct hit at any workspace root.
    for root in roots {
        if root.join("pyproject.toml").is_file() {
            return Some(root.clone());
        }
    }

    // One level deep — common in monorepo layouts (e.g. `ai_cms/agent-backend/`).
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("pyproject.toml").is_file() {
                info!(
                    project_root = %path.display(),
                    "found pyproject.toml in subdirectory"
                );
                return Some(path);
            }
        }
    }

    None
}

/// Resolve the uv project root for a uv handler.
///
/// Returns `Ok(Some(root))` when a uv project is detected, `Ok(None)` when
/// the workspace contains no `pyproject.toml` (signal to the caller to no-op
/// cleanly), or `Err` only when there is no workspace root at all.
async fn get_workspace_root(server: &LspServer) -> LspResult<Option<std::path::PathBuf>> {
    let roots = server.workspace_roots.read().await;
    if roots.is_empty() {
        return Err(super::no_workspace_root_error());
    }
    Ok(find_uv_project_root(&roots))
}

/// Strip ANSI escape sequences (CSI, OSC) from a string.
///
/// uv emits coloured error output and tracing wraps log lines in colour
/// escapes. The VS Code output channel renders them as raw garbage (see
/// issue #23), so we clean them before showing messages to the user.
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.peek() {
                Some('[') => {
                    // CSI: ESC [ ... <final byte 0x40..=0x7e>
                    let _ = chars.next();
                    while let Some(&inner) = chars.peek() {
                        let _ = chars.next();
                        if matches!(inner, '\u{40}'..='\u{7e}') {
                            break;
                        }
                    }
                    continue;
                }
                Some(']') => {
                    // OSC: ESC ] ... terminated by BEL (0x07) or ESC \\
                    let _ = chars.next();
                    while let Some(&inner) = chars.peek() {
                        let _ = chars.next();
                        if inner == '\u{07}' {
                            break;
                        }
                        if inner == '\u{1b}' && chars.peek() == Some(&'\\') {
                            let _ = chars.next();
                            break;
                        }
                    }
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

/// Convert a [`crate::uv_commands::UvCommandResult`] into a JSON response value.
fn uv_result_to_json(result: &crate::uv_commands::UvCommandResult) -> serde_json::Value {
    serde_json::json!({
        "success": result.success,
        "stdout": result.stdout,
        "stderr": result.stderr,
    })
}

/// Extract a package/module name string from the first command argument.
///
/// Accepts either a bare string `"requests"` or an object `{"package": "requests"}`.
/// Shared with [`super::stub_handlers`] so both `uv.*` and `stubs.*` commands
/// parse their single string argument identically.
pub(super) fn extract_package_arg(args: &[serde_json::Value]) -> Option<String> {
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
///
/// Failures are classified per [LSPUV-COMMAND-FAILURE-UX] (issue #94): the
/// toast carries a plain-language headline + remediation, while the full uv
/// stderr stays in the Output channel via `log_message`.
async fn run_uv_and_refresh<F, Fut>(
    server: &LspServer,
    label: &str,
    package: Option<&str>,
    command: F,
) -> LspResult<Option<serde_json::Value>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = std::io::Result<crate::uv_commands::UvCommandResult>>,
{
    let started = std::time::Instant::now();
    match command().await {
        Ok(result) => {
            let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            if result.success {
                info!(
                    command = label,
                    package = package.unwrap_or(""),
                    duration_ms,
                    "uv command succeeded"
                );
                server
                    .client
                    .log_message(
                        MessageType::INFO,
                        format!("Basilisk: {label} completed successfully"),
                    )
                    .await;
                // Rebuild registry and re-resolve imports on success.
                super::init::rebuild_registry_and_resolve(server).await;
                return Ok(Some(uv_result_to_json(&result)));
            }

            // Strip ANSI escapes — uv emits coloured errors and the
            // VS Code output channel renders the raw codes (issue #23).
            let stderr_clean = strip_ansi(result.stderr.trim());
            let failure = crate::uv_failure::classify_uv_failure(label, package, &stderr_clean);
            warn!(
                command = label,
                package = package.unwrap_or(""),
                exit_code = result.exit_code.unwrap_or(-1),
                failure_category = failure.category.as_str(),
                duration_ms,
                "uv command failed"
            );

            // Output channel keeps the FULL uv stderr — power users lose nothing.
            server
                .client
                .log_message(
                    MessageType::ERROR,
                    format!("Basilisk: {label} failed:\n{stderr_clean}"),
                )
                .await;

            // The toast is the classified headline + remediation, never the
            // raw resolver dump.
            server
                .client
                .show_message(MessageType::ERROR, format!("Basilisk: {}", failure.message))
                .await;

            Ok(Some(uv_result_to_json(&result)))
        }
        Err(err) => {
            let failure = crate::uv_failure::classify_uv_spawn_failure(&err);
            error!(
                command = label,
                package = package.unwrap_or(""),
                failure_category = failure.category.as_str(),
                %err,
                "uv command failed to spawn"
            );
            server
                .client
                .show_message(MessageType::ERROR, format!("Basilisk: {}", failure.message))
                .await;
            Err(spawn_error(label, &err))
        }
    }
}

/// Build a no-op JSON response for uv commands that require a project file
/// when none exists. The handler logs a clear non-error message and returns
/// success=false with a stderr explaining why nothing happened, instead of
/// running the uv subprocess from a directory without a `pyproject.toml`
/// (which would surface a confusing parse-error to the user — issue #23).
async fn no_uv_project_response(
    server: &LspServer,
    label: &str,
) -> LspResult<Option<serde_json::Value>> {
    let msg = format!("Basilisk: {label} skipped — no `pyproject.toml` found in this workspace");
    info!("{msg}");
    server
        .client
        .log_message(MessageType::INFO, msg.clone())
        .await;
    Ok(Some(serde_json::json!({
        "success": false,
        "stdout": "",
        "stderr": format!("{label} skipped — no `pyproject.toml` found in this workspace"),
        "skipped": true,
    })))
}

/// Handle `basilisk.uv.sync`.
pub(super) async fn execute_uv_sync(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(root) = get_workspace_root(server).await? else {
        return no_uv_project_response(server, "uv sync").await;
    };
    info!("uv sync requested");
    run_uv_and_refresh(server, "uv sync", None, || {
        crate::uv_commands::uv_sync(&root)
    })
    .await
}

/// Handle `basilisk.uv.add`.
pub(super) async fn execute_uv_add(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(root) = get_workspace_root(server).await? else {
        return no_uv_project_response(server, "uv add").await;
    };
    let Some(package) = extract_package_arg(args) else {
        warn!("uv add: missing package argument");
        return Ok(None);
    };
    info!(package = %package, "uv add requested");
    run_uv_and_refresh(server, &format!("uv add {package}"), Some(&package), || {
        crate::uv_commands::uv_add(&root, &package)
    })
    .await
}

/// Handle `basilisk.uv.addDev`.
pub(super) async fn execute_uv_add_dev(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(root) = get_workspace_root(server).await? else {
        return no_uv_project_response(server, "uv add --dev").await;
    };
    let Some(package) = extract_package_arg(args) else {
        warn!("uv addDev: missing package argument");
        return Ok(None);
    };
    info!(package = %package, "uv add --dev requested");
    run_uv_and_refresh(
        server,
        &format!("uv add --dev {package}"),
        Some(&package),
        || crate::uv_commands::uv_add_dev(&root, &package),
    )
    .await
}

/// Handle `basilisk.uv.remove`.
pub(super) async fn execute_uv_remove(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(root) = get_workspace_root(server).await? else {
        return no_uv_project_response(server, "uv remove").await;
    };
    let Some(package) = extract_package_arg(args) else {
        warn!("uv remove: missing package argument");
        return Ok(None);
    };
    info!(package = %package, "uv remove requested");
    run_uv_and_refresh(
        server,
        &format!("uv remove {package}"),
        Some(&package),
        || crate::uv_commands::uv_remove(&root, &package),
    )
    .await
}

/// Handle `basilisk.uv.lock`.
pub(super) async fn execute_uv_lock(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(root) = get_workspace_root(server).await? else {
        return no_uv_project_response(server, "uv lock").await;
    };
    info!("uv lock requested");
    run_uv_and_refresh(server, "uv lock", None, || {
        crate::uv_commands::uv_lock(&root)
    })
    .await
}

/// Handle `basilisk.uv.createEnv`.
///
/// `uv venv` does not require a `pyproject.toml`, so this falls back to the
/// first workspace root when no project is detected.
pub(super) async fn execute_uv_create_env(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let root = if let Some(r) = get_workspace_root(server).await? {
        r
    } else {
        let roots = server.workspace_roots.read().await;
        let Some(first) = roots.first().cloned() else {
            return Err(super::no_workspace_root_error());
        };
        first
    };
    let python_version = args.first().and_then(|v| {
        v.as_str()
            .or_else(|| v.get("python").and_then(|p| p.as_str()))
    });
    let pv = python_version.map(String::from);
    info!(python_version = ?pv, "uv venv requested");
    run_uv_and_refresh(server, "uv venv", None, || {
        crate::uv_commands::uv_create_env(&root, pv.as_deref())
    })
    .await
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test-only code: indexing/unwrap acceptable in unit tests"
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
            exit_code: Some(0),
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

    /// Generate a unique temp dir path to avoid races between parallel tests.
    fn unique_tmp(prefix: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CTR: AtomicU64 = AtomicU64::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "{prefix}_{n}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ))
    }

    /// Regression for issue #23: when no workspace root contains a
    /// `pyproject.toml` (and no uv project is detected anywhere in the tree),
    /// `find_uv_project_root` must return `None` so `basilisk.uv.sync` can
    /// no-op cleanly instead of running `uv sync` from a directory without a
    /// `pyproject.toml` (which produces the spurious
    /// "No `pyproject.toml` found in current directory or any parent directory"
    /// error reported in the bug).
    #[test]
    fn find_uv_project_root_returns_none_for_workspace_without_pyproject() {
        let dir = unique_tmp("bsk_uv23_no_proj");
        std::fs::create_dir_all(&dir).unwrap();
        // Workspace root with NO pyproject.toml, NO uv.lock, NO [tool.uv].
        std::fs::write(dir.join("README.md"), "# no python project\n").unwrap();

        let roots = vec![dir.clone()];
        let result = find_uv_project_root(&roots);

        assert!(
            result.is_none(),
            "no uv project must yield None so the handler can no-op; got {result:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `find_uv_project_root` must still return the project root when a
    /// `pyproject.toml` lives in a subdirectory (the existing one-level-deep
    /// search behaviour must keep working).
    #[test]
    fn find_uv_project_root_returns_subdir_with_pyproject() {
        let dir = unique_tmp("bsk_uv23_subdir");
        let sub = dir.join("agent-backend");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("pyproject.toml"),
            "[project]\nname = \"x\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let roots = vec![dir.clone()];
        let result = find_uv_project_root(&roots);

        assert_eq!(result.as_deref(), Some(sub.as_path()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ANSI escape sequences from tracing/uv output must be stripped before
    /// being shown to the user. Issue #23 reports raw escape codes appearing
    /// in the VS Code output channel.
    #[test]
    fn strip_ansi_removes_color_escapes() {
        let with_ansi = "\u{1b}[31merror:\u{1b}[0m oh no";
        let cleaned = strip_ansi(with_ansi);
        assert_eq!(cleaned, "error: oh no");
    }

    #[test]
    fn strip_ansi_passes_plain_text_through() {
        let plain = "no escapes here";
        assert_eq!(strip_ansi(plain), plain);
    }
}
