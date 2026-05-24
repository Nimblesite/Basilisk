//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Platform-specific privilege detection and escalation for profiling.
//!
//! py-spy reads target process memory via OS-specific calls:
//! - macOS: `vm_read` (requires root or parent-child relationship)
//! - Linux: `process_vm_readv` (requires `ptrace_scope`=0 or root)
//! - Windows: `ReadProcessMemory` (works for same-user processes)
//!
//! This module detects whether the current process has sufficient privileges
//! to profile a given PID, and on macOS spawns the elevated helper binary
//! via `osascript` when needed.

#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use tracing::warn;
use tracing::{debug, info};

use super::ProfileError;

/// Result of checking whether profiling privileges are sufficient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionStatus {
    /// Profiling is allowed without elevation.
    Allowed,
    /// Elevation is required (with a human-readable reason).
    ElevationRequired(String),
    /// Profiling is not possible (with a human-readable explanation).
    Denied(String),
}

/// Check whether the current process can profile the target PID.
///
/// Returns [`PermissionStatus::Allowed`] if no elevation is needed,
/// [`PermissionStatus::ElevationRequired`] if the helper must be spawned,
/// or [`PermissionStatus::Denied`] if profiling is not possible.
///
/// # Errors
///
/// Returns an error string if the platform check itself fails.
pub fn check_profiling_permissions(pid: u32) -> Result<PermissionStatus, String> {
    check_permissions_for_platform(pid)
}

/// Attempt to elevate privileges if needed for profiling the target PID.
///
/// On macOS, this spawns the `basilisk-profiler-helper` binary via `osascript`
/// with administrator privileges. On other platforms, it returns an error
/// with instructions for the user.
///
/// # Errors
///
/// Returns [`ProfileError::Sampler`] with a permission-denied error if
/// elevation fails or is not possible.
pub async fn elevate_if_needed(pid: u32) -> Result<(), ProfileError> {
    let status = check_profiling_permissions(pid).map_err(ProfileError::ExportFailed)?;

    match status {
        PermissionStatus::Allowed => Ok(()),
        PermissionStatus::ElevationRequired(reason) => {
            info!(pid, %reason, "elevation required for profiling");
            attempt_elevation(pid).await
        }
        PermissionStatus::Denied(reason) => Err(ProfileError::Sampler(
            super::sampler::SamplerError::PermissionDenied(reason),
        )),
    }
}

/// Platform-specific permission check dispatch.
///
/// On macOS this always succeeds; Linux may fail reading `ptrace_scope`.
/// The Result type is kept for cross-platform signature consistency.
#[expect(
    clippy::unnecessary_wraps,
    reason = "Cross-platform signature consistency with Linux variant"
)]
#[cfg(target_os = "macos")]
fn check_permissions_for_platform(pid: u32) -> Result<PermissionStatus, String> {
    Ok(check_macos_permissions(pid))
}

/// Platform-specific permission check dispatch.
///
/// The Result type is kept for cross-platform signature consistency.
#[expect(
    clippy::unnecessary_wraps,
    reason = "Cross-platform signature consistency with macOS variant"
)]
#[cfg(target_os = "linux")]
fn check_permissions_for_platform(pid: u32) -> Result<PermissionStatus, String> {
    Ok(check_linux_permissions(pid))
}

/// Platform-specific permission check dispatch.
#[cfg(target_os = "windows")]
fn check_permissions_for_platform(_pid: u32) -> Result<PermissionStatus, String> {
    check_windows_permissions()
}

/// Fallback for unsupported platforms.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn check_permissions_for_platform(_pid: u32) -> Result<PermissionStatus, String> {
    Ok(PermissionStatus::Denied(
        "Profiling is not supported on this platform".to_owned(),
    ))
}

// ── macOS ──────────────────────────────────────────────────────────────────

/// Check macOS profiling permissions for a target PID.
///
/// Parent processes can trace their children without elevation. For external
/// processes, root privileges are required for `task_for_pid` / `vm_read`.
#[cfg(target_os = "macos")]
fn check_macos_permissions(pid: u32) -> PermissionStatus {
    if !process_exists(pid) {
        return PermissionStatus::Denied(format!("Process {pid} not found"));
    }

    if is_child_process(pid) {
        debug!(pid, "target is a child process, no elevation needed");
        return PermissionStatus::Allowed;
    }

    if is_running_as_root() {
        return PermissionStatus::Allowed;
    }

    PermissionStatus::ElevationRequired(
        "macOS requires elevated privileges (vm_read) to profile external processes".to_owned(),
    )
}

// ── Linux ──────────────────────────────────────────────────────────────────

/// Check Linux profiling permissions by reading `ptrace_scope`.
///
/// - `ptrace_scope` = 0: classic, any process can trace any other.
/// - `ptrace_scope` = 1: restricted, only parent can trace child.
/// - `ptrace_scope` = 2: admin-only, requires `CAP_SYS_PTRACE`.
/// - `ptrace_scope` = 3: no ptrace at all.
#[cfg(target_os = "linux")]
fn check_linux_permissions(pid: u32) -> PermissionStatus {
    if !process_exists(pid) {
        return PermissionStatus::Denied(format!("Process {pid} not found"));
    }

    if is_child_process(pid) {
        debug!(pid, "target is a child process, no elevation needed");
        return PermissionStatus::Allowed;
    }

    if is_running_as_root() {
        return PermissionStatus::Allowed;
    }

    match read_ptrace_scope() {
        Ok(0) => PermissionStatus::Allowed,
        Ok(1) => PermissionStatus::Denied(
            "ptrace_scope is 1 (restricted). Only parent processes can trace children. \
             Options: run `sudo sysctl kernel.yama.ptrace_scope=0` or \
             `sudo setcap cap_sys_ptrace+ep $(which basilisk)` or \
             profile child processes via debug sessions."
                .to_owned(),
        ),
        Ok(2) => PermissionStatus::Denied(
            "ptrace_scope is 2 (admin-only). Requires CAP_SYS_PTRACE. \
             Run: `sudo setcap cap_sys_ptrace+ep $(which basilisk)`"
                .to_owned(),
        ),
        Ok(3) => PermissionStatus::Denied(
            "ptrace_scope is 3 (no ptrace). Profiling external processes is disabled \
             by kernel policy. Profile child processes via debug sessions instead."
                .to_owned(),
        ),
        Ok(scope) => PermissionStatus::Denied(format!("Unknown ptrace_scope value: {scope}")),
        Err(err) => {
            warn!(%err, "could not read ptrace_scope, assuming allowed");
            PermissionStatus::Allowed
        }
    }
}

/// Read the Linux `ptrace_scope` sysctl value.
#[cfg(target_os = "linux")]
fn read_ptrace_scope() -> Result<u8, String> {
    let content = std::fs::read_to_string("/proc/sys/kernel/yama/ptrace_scope")
        .map_err(|err| format!("failed to read ptrace_scope: {err}"))?;

    content
        .trim()
        .parse::<u8>()
        .map_err(|err| format!("failed to parse ptrace_scope: {err}"))
}

// ── Windows ────────────────────────────────────────────────────────────────

/// Check Windows profiling permissions.
///
/// `ReadProcessMemory` works without elevation for processes owned by the
/// same user, so no special handling is needed.
#[cfg(target_os = "windows")]
fn check_windows_permissions() -> Result<PermissionStatus, String> {
    debug!("Windows: ReadProcessMemory works for same-user processes");
    Ok(PermissionStatus::Allowed)
}

// ── Shared helpers ─────────────────────────────────────────────────────────

/// Check whether the target PID is a child of the current process.
///
/// If the target was spawned by the Basilisk debug session manager, the LSP
/// is the parent and can trace it without elevation on both macOS and Linux.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn is_child_process(pid: u32) -> bool {
    let our_pid = std::process::id();
    parent_pid_of(pid).is_some_and(|ppid| ppid == our_pid)
}

/// Check whether the target PID exists.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn process_exists(pid: u32) -> bool {
    pid != 0 && parent_pid_of(pid).is_some()
}

/// Get the parent PID of a process (platform-specific).
#[cfg(target_os = "macos")]
fn parent_pid_of(pid: u32) -> Option<u32> {
    read_ppid_via_ps(pid)
}

/// Get the parent PID of a process (platform-specific).
#[cfg(target_os = "linux")]
fn parent_pid_of(pid: u32) -> Option<u32> {
    read_ppid_from_proc(pid).or_else(|| read_ppid_via_ps(pid))
}

/// Read parent PID from `/proc/<pid>/status` on Linux.
#[cfg(target_os = "linux")]
fn read_ppid_from_proc(pid: u32) -> Option<u32> {
    let path = format!("/proc/{pid}/status");
    let content = std::fs::read_to_string(path).ok()?;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("PPid:\t") {
            return value.trim().parse::<u32>().ok();
        }
    }

    None
}

/// Read parent PID via `ps` command (works on macOS and Linux).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn read_ppid_via_ps(pid: u32) -> Option<u32> {
    let output = std::process::Command::new("ps")
        .args(["-o", "ppid=", "-p"])
        .arg(pid.to_string())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim().parse::<u32>().ok()
}

/// Check whether the current process is running as root (UID 0).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn is_running_as_root() -> bool {
    // SAFETY: not using unsafe — std::process::Command is fine here.
    // We check the effective UID via the `id` command to avoid libc calls.
    std::process::Command::new("id")
        .args(["-u"])
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u32>()
                .ok()
        })
        .is_some_and(|uid| uid == 0)
}

// ── Elevation ──────────────────────────────────────────────────────────────

/// macOS elevation: spawn the helper via `osascript` with admin privileges.
#[cfg(target_os = "macos")]
async fn attempt_elevation(pid: u32) -> Result<(), ProfileError> {
    let helper_path = find_helper_binary()?;
    info!(
        pid,
        helper = %helper_path.display(),
        "spawning elevated profiler helper via osascript"
    );
    spawn_elevated_helper(&helper_path, pid).await
}

/// Linux and Windows do not support elevation — returns an error immediately.
#[cfg(not(target_os = "macos"))]
async fn attempt_elevation(_pid: u32) -> Result<(), ProfileError> {
    // Yield once to satisfy the `async` requirement (callers `.await` this).
    tokio::task::yield_now().await;
    Err(ProfileError::Sampler(
        super::sampler::SamplerError::PermissionDenied(
            "Privilege elevation is only supported on macOS. \
             On Linux, adjust ptrace_scope or use setcap."
                .to_owned(),
        ),
    ))
}

/// Locate the `basilisk-profiler-helper` binary.
///
/// Searches next to the current executable first, then falls back to PATH.
#[cfg(target_os = "macos")]
fn find_helper_binary() -> Result<PathBuf, ProfileError> {
    let helper_name = "basilisk-profiler-helper";

    // Check adjacent to the current executable.
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let adjacent = exe_dir.join(helper_name);
            if adjacent.exists() {
                return Ok(adjacent);
            }
        }
    }

    // Fall back to PATH lookup via `which`.
    let which_result = std::process::Command::new("which")
        .arg(helper_name)
        .output()
        .map_err(|err| {
            ProfileError::Sampler(super::sampler::SamplerError::PermissionDenied(format!(
                "Failed to locate {helper_name}: {err}"
            )))
        })?;

    if which_result.status.success() {
        let path_str = String::from_utf8_lossy(&which_result.stdout);
        return Ok(PathBuf::from(path_str.trim()));
    }

    Err(ProfileError::Sampler(
        super::sampler::SamplerError::PermissionDenied(format!(
            "Could not find {helper_name} binary. \
             Ensure it is installed alongside the Basilisk LSP."
        )),
    ))
}

/// Spawn the helper binary with elevated privileges via `osascript`.
///
/// Uses the macOS `do shell script ... with administrator privileges`
/// `AppleScript` command, which shows the system password prompt.
#[cfg(target_os = "macos")]
async fn spawn_elevated_helper(
    helper_path: &std::path::Path,
    pid: u32,
) -> Result<(), ProfileError> {
    let socket_path = create_socket_path(pid);
    let helper_str = helper_path.display().to_string();

    let script =
        format!("do shell script \"{helper_str} {socket_path}\" with administrator privileges");

    let result = tokio::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await
        .map_err(|err| {
            ProfileError::Sampler(super::sampler::SamplerError::PermissionDenied(format!(
                "Failed to spawn osascript for privilege elevation: {err}"
            )))
        })?;

    if result.status.success() {
        info!(pid, "elevated helper completed successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        Err(ProfileError::Sampler(
            super::sampler::SamplerError::PermissionDenied(format!(
                "Privilege elevation failed (user may have cancelled): {stderr}"
            )),
        ))
    }
}

/// Return platform-specific help text for permission errors.
///
/// Provides actionable guidance when the user encounters a privilege error
/// while trying to profile an external process.
#[must_use]
pub fn platform_permission_message() -> &'static str {
    platform_permission_message_impl()
}

/// macOS-specific permission help text.
#[cfg(target_os = "macos")]
fn platform_permission_message_impl() -> &'static str {
    "macOS requires elevated privileges to profile external processes. \
     The Basilisk profiler helper will prompt for your password via the \
     system dialog. To avoid this, profile processes launched from a \
     debug session instead."
}

/// Linux-specific permission help text.
#[cfg(target_os = "linux")]
fn platform_permission_message_impl() -> &'static str {
    "Linux profiling requires ptrace access. Options:\n\
     1. Set ptrace_scope to 0: sudo sysctl kernel.yama.ptrace_scope=0\n\
     2. Grant capability: sudo setcap cap_sys_ptrace+ep $(which basilisk)\n\
     3. Profile child processes via debug sessions (no elevation needed)"
}

/// Windows-specific permission help text.
#[cfg(target_os = "windows")]
fn platform_permission_message_impl() -> &'static str {
    "Windows allows profiling processes owned by the same user without \
     elevation. If you encounter errors, ensure the target process is \
     running under your user account."
}

/// Fallback permission help text.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_permission_message_impl() -> &'static str {
    "Profiling is not supported on this platform."
}

/// Generate a unique Unix socket path for helper communication.
#[cfg(target_os = "macos")]
fn create_socket_path(pid: u32) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    format!("/tmp/basilisk-profiler-{pid}-{timestamp}.sock")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_status_equality() {
        assert_eq!(PermissionStatus::Allowed, PermissionStatus::Allowed);
        assert_ne!(
            PermissionStatus::Allowed,
            PermissionStatus::Denied("test".to_owned())
        );
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn child_process_check_returns_false_for_nonexistent_pid() {
        // PID 0 is the kernel, not our child.
        assert!(!is_child_process(0));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn child_process_check_returns_false_for_random_pid() {
        // Very unlikely to be our child.
        assert!(!is_child_process(999_999));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn nonexistent_pid_is_denied_before_elevation() -> Result<(), String> {
        let status = check_profiling_permissions(999_999_999)?;
        assert!(
            matches!(status, PermissionStatus::Denied(ref reason) if reason.contains("not found")),
            "nonexistent PID should be denied before elevation, got: {status:?}"
        );
        Ok(())
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn pid_zero_is_denied_before_elevation() -> Result<(), String> {
        let status = check_profiling_permissions(0)?;
        assert!(
            matches!(status, PermissionStatus::Denied(ref reason) if reason.contains("not found")),
            "PID 0 should be denied before elevation, got: {status:?}"
        );
        Ok(())
    }

    #[test]
    fn check_permissions_returns_result() {
        // Should not panic for any PID.
        let result = check_profiling_permissions(1);
        assert!(result.is_ok());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_non_child_requires_elevation_when_not_root() {
        // PID 1 (launchd) is not our child and we are not root in tests.
        let status = check_macos_permissions(1);
        // If running as root in CI, this will be Allowed; otherwise ElevationRequired.
        assert!(
            status == PermissionStatus::Allowed
                || matches!(status, PermissionStatus::ElevationRequired(_))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_reads_ptrace_scope_without_panic() {
        // Should return a valid status regardless of the actual scope value.
        let _result = check_linux_permissions(1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn socket_path_contains_pid() {
        let path = create_socket_path(12345);
        assert!(path.contains("12345"));
        assert!(path.starts_with("/tmp/basilisk-profiler-"));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn is_child_process_true_for_spawned_child() {
        // Spawn a short-lived child and verify detection.
        let child = std::process::Command::new("sleep").arg("10").spawn();

        if let Ok(mut child_proc) = child {
            let child_pid = child_proc.id();
            assert!(is_child_process(child_pid));
            let _ = child_proc.kill();
            let _ = child_proc.wait();
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn check_ptrace_scope_parses_values() {
        let result = read_ptrace_scope();
        match result {
            Ok(scope) => assert!(scope <= 3, "unexpected ptrace_scope: {scope}"),
            Err(err) => assert!(
                err.contains("ptrace_scope"),
                "error should mention ptrace_scope: {err}"
            ),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_permissions_child_process_allowed() {
        let child = std::process::Command::new("sleep").arg("10").spawn();

        if let Ok(mut child_proc) = child {
            let child_pid = child_proc.id();
            let result = check_linux_permissions(child_pid);
            assert_eq!(result, PermissionStatus::Allowed);
            let _ = child_proc.kill();
            let _ = child_proc.wait();
        }
    }

    #[test]
    fn platform_permission_message_is_not_empty() {
        let msg = platform_permission_message();
        assert!(!msg.is_empty());
        assert!(msg.len() > 20, "message should be descriptive");
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn elevate_if_needed_allowed_for_child() -> Result<(), String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to build tokio runtime: {err}"))?;

        let child = std::process::Command::new("sleep").arg("10").spawn();

        if let Ok(mut child_proc) = child {
            let child_pid = child_proc.id();
            let result = rt.block_on(elevate_if_needed(child_pid));
            assert!(result.is_ok(), "child process should not need elevation");
            let _ = child_proc.kill();
            let _ = child_proc.wait();
        }
        Ok(())
    }
}
