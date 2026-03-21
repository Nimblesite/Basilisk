//! uv command execution — thin subprocess wrapper.
//!
//! Delegates all package management to the `uv` CLI. We don't reinvent
//! dependency resolution — we just invoke `uv` and refresh our state.

use std::path::Path;
use std::time::Duration;
use tracing::{error, info};

/// Timeout for uv subprocess calls.
const UV_TIMEOUT: Duration = Duration::from_secs(30);

/// Result of a uv command execution.
#[derive(Debug)]
pub struct UvCommandResult {
    /// Whether the command succeeded (exit code 0).
    pub success: bool,
    /// Combined stdout output.
    pub stdout: String,
    /// Combined stderr output.
    pub stderr: String,
}

/// Check if `uv` is available on `PATH`.
pub async fn is_uv_available() -> bool {
    let result = tokio::process::Command::new("uv")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    match result {
        Ok(status) => status.success(),
        Err(err) => {
            info!(%err, "uv not found on PATH");
            false
        }
    }
}

/// Run `uv sync` in the given project root.
///
/// # Errors
///
/// Returns an `io::Error` if the subprocess cannot be spawned.
pub async fn uv_sync(project_root: &Path) -> Result<UvCommandResult, std::io::Error> {
    run_uv(project_root, &["sync"]).await
}

/// Run `uv add <package>` in the given project root.
///
/// # Errors
///
/// Returns an `io::Error` if the subprocess cannot be spawned.
pub async fn uv_add(project_root: &Path, package: &str) -> Result<UvCommandResult, std::io::Error> {
    run_uv(project_root, &["add", package]).await
}

/// Run `uv add --dev <package>` in the given project root.
///
/// # Errors
///
/// Returns an `io::Error` if the subprocess cannot be spawned.
pub async fn uv_add_dev(
    project_root: &Path,
    package: &str,
) -> Result<UvCommandResult, std::io::Error> {
    run_uv(project_root, &["add", "--dev", package]).await
}

/// Run `uv remove <package>` in the given project root.
///
/// # Errors
///
/// Returns an `io::Error` if the subprocess cannot be spawned.
pub async fn uv_remove(
    project_root: &Path,
    package: &str,
) -> Result<UvCommandResult, std::io::Error> {
    run_uv(project_root, &["remove", package]).await
}

/// Run `uv lock` in the given project root.
///
/// # Errors
///
/// Returns an `io::Error` if the subprocess cannot be spawned.
pub async fn uv_lock(project_root: &Path) -> Result<UvCommandResult, std::io::Error> {
    run_uv(project_root, &["lock"]).await
}

/// Run `uv venv` in the given project root, optionally with a Python version.
///
/// # Errors
///
/// Returns an `io::Error` if the subprocess cannot be spawned.
pub async fn uv_create_env(
    project_root: &Path,
    python_version: Option<&str>,
) -> Result<UvCommandResult, std::io::Error> {
    match python_version {
        Some(version) => run_uv(project_root, &["venv", "--python", version]).await,
        None => run_uv(project_root, &["venv"]).await,
    }
}

/// Shared helper: spawn `uv` with the given arguments, capture output, apply
/// the 30-second timeout, and return a structured result.
async fn run_uv(project_root: &Path, args: &[&str]) -> Result<UvCommandResult, std::io::Error> {
    info!(project_root = %project_root.display(), args = ?args, "running uv command");

    let child = tokio::process::Command::new("uv")
        .args(args)
        .current_dir(project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let output =
        if let Ok(result) = tokio::time::timeout(UV_TIMEOUT, child.wait_with_output()).await {
            result?
        } else {
            error!(
                project_root = %project_root.display(),
                args = ?args,
                "uv command timed out after {UV_TIMEOUT:?}"
            );
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("uv command timed out after {UV_TIMEOUT:?}"),
            ));
        };

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let success = output.status.success();

    if success {
        info!(args = ?args, "uv command succeeded");
    } else {
        error!(args = ?args, %stderr, "uv command failed");
    }

    Ok(UvCommandResult {
        success,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn is_uv_available_returns_bool() {
        // Should not panic regardless of whether uv is installed.
        let _available = is_uv_available().await;
    }

    #[test]
    fn uv_command_result_debug_format() {
        let result = UvCommandResult {
            success: true,
            stdout: "ok".to_owned(),
            stderr: String::new(),
        };
        let debug_output = format!("{result:?}");
        assert!(debug_output.contains("success: true"));
        assert!(debug_output.contains("ok"));
    }
}
