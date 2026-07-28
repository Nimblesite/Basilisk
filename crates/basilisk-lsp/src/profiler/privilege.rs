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

#[cfg(target_os = "linux")]
use tracing::warn;
use tracing::{debug, info};

use super::sampler::{self, SamplerConfig, SamplerError, SamplerHandle};
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
/// Implements [PROFILE-PERMISSIONS]: a child PID yields `Allowed` (in-process
/// py-spy), an external PID yields `ElevationRequired` (macOS helper socket), and
/// a missing PID yields `Denied`. This is the authoritative attach-time check
/// that the panel's `requiresElevation` hint defers to.
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

/// Create a sampler for the target PID, choosing the strategy by privilege.
///
/// - [`PermissionStatus::Allowed`] → attach py-spy in-process.
/// - [`PermissionStatus::ElevationRequired`] → spawn the elevated helper and
///   stream samples over a Unix socket (macOS — see [`super::helper_client`]).
/// - [`PermissionStatus::Denied`] → fail with a permission error.
///
/// This replaces the previously broken two-step "elevate, then also attach
/// in-process" flow (issue #61): for an external macOS process the in-process
/// attach could never succeed, so the elevated helper now *is* the sampler.
///
/// # Errors
///
/// Returns [`ProfileError`] if permissions are insufficient or the chosen
/// sampler cannot be started.
pub(crate) async fn create_sampler(config: &SamplerConfig) -> Result<SamplerHandle, ProfileError> {
    let status = check_profiling_permissions(config.pid).map_err(ProfileError::ExportFailed)?;

    match status {
        PermissionStatus::Allowed => sampler::start_sampler(config)
            .map_err(|err| ProfileError::Sampler(with_platform_guidance(err))),
        PermissionStatus::ElevationRequired(reason) => {
            info!(pid = config.pid, %reason, "elevation required; sampling via elevated helper");
            start_elevated_sampler(config)
                .await
                .map_err(ProfileError::Sampler)
        }
        PermissionStatus::Denied(reason) => Err(ProfileError::Sampler(
            SamplerError::PermissionDenied(reason),
        )),
    }
}

/// Append platform remedies to a permission failure so a denied attach stays
/// actionable (issue #81): on Linux a real `EPERM` from py-spy gets the
/// `ptrace_scope` remedies the upfront check can no longer assert (see
/// [`classify_ptrace_scope`]). Other errors and platforms pass through.
#[cfg(target_os = "linux")]
fn with_platform_guidance(err: SamplerError) -> SamplerError {
    match err {
        SamplerError::PermissionDenied(msg) => SamplerError::PermissionDenied(format!(
            "{msg}. ptrace is restricted on this system: run \
             `sudo sysctl kernel.yama.ptrace_scope=0`, or \
             `sudo setcap cap_sys_ptrace+ep $(which basilisk)`, or \
             profile the process via a debug session."
        )),
        other => other,
    }
}

/// Non-Linux: no extra guidance to add — macOS routes through elevation and
/// Windows needs none for same-user targets.
#[cfg(not(target_os = "linux"))]
fn with_platform_guidance(err: SamplerError) -> SamplerError {
    err
}

/// macOS: stream samples from the elevated helper over a Unix socket.
#[cfg(target_os = "macos")]
async fn start_elevated_sampler(config: &SamplerConfig) -> Result<SamplerHandle, SamplerError> {
    super::helper_client::start_helper_sampler(config, super::helper_client::HelperSpawn::Elevated)
        .await
}

/// Non-macOS: there is no supported privilege-elevation path for profiling.
#[cfg(not(target_os = "macos"))]
async fn start_elevated_sampler(_config: &SamplerConfig) -> Result<SamplerHandle, SamplerError> {
    // Yield once to satisfy the `async` requirement (callers `.await` this).
    tokio::task::yield_now().await;
    Err(SamplerError::PermissionDenied(
        "Privilege elevation is only supported on macOS. \
         On Linux, adjust ptrace_scope or use setcap."
            .to_owned(),
    ))
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
///
/// The Result type is kept for cross-platform signature consistency.
#[expect(
    clippy::unnecessary_wraps,
    reason = "Cross-platform signature consistency with Linux variant"
)]
#[cfg(target_os = "windows")]
fn check_permissions_for_platform(_pid: u32) -> Result<PermissionStatus, String> {
    Ok(check_windows_permissions())
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
/// Implements [PROFILE-PERMISSIONS-MACOS]: a child Basilisk launched can be
/// traced by its parent with no elevation; any **external** process (even
/// same-user) needs root for `task_for_pid` / `vm_read`, so it returns
/// `ElevationRequired` (the LSP then spawns the elevated helper). There is no
/// "same-user, no-elevation" shortcut on macOS.
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
/// Implements [PROFILE-PERMISSIONS-LINUX]: works without root if
/// `ptrace_scope=0`; under the default `1` the precheck **attempts the attach**
/// rather than denying upfront (Yama still grants ancestors and `PR_SET_PTRACER`
/// opt-ins), with a real `EPERM` surfaced as a classified error + remedies;
/// scopes `2`/`3` are denied upfront.
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
        Ok(scope) => classify_ptrace_scope(scope),
        Err(err) => {
            warn!(%err, "could not read ptrace_scope, assuming allowed");
            PermissionStatus::Allowed
        }
    }
}

/// Map a Yama `ptrace_scope` value to a permission status (pure, so every
/// platform's test suite can pin the policy).
///
/// `1` (restricted) is deliberately **Allowed**: Yama still permits two
/// grants this precheck cannot see — *ancestor* relationships (a debug
/// session's debuggee is the LSP's grandchild via `debugpy.adapter`) and
/// targets that opted in via `PR_SET_PTRACER`. Denying upfront falsely
/// blocked the flagship "Run & Profile" flows on default Ubuntu; instead the
/// attach is attempted and a real `EPERM` is surfaced with remedies appended
/// (see `with_platform_guidance`). Scopes `2`/`3` are kernel-enforced
/// regardless of relationship, so they stay denied with the remedy.
#[cfg(any(target_os = "linux", test))]
fn classify_ptrace_scope(scope: u8) -> PermissionStatus {
    match scope {
        0 | 1 => PermissionStatus::Allowed,
        2 => PermissionStatus::Denied(
            "ptrace_scope is 2 (admin-only). Requires CAP_SYS_PTRACE. \
             Run: `sudo setcap cap_sys_ptrace+ep $(which basilisk)`"
                .to_owned(),
        ),
        3 => PermissionStatus::Denied(
            "ptrace_scope is 3 (no ptrace). Profiling external processes is disabled \
             by kernel policy. Profile child processes via debug sessions instead."
                .to_owned(),
        ),
        other => PermissionStatus::Denied(format!("Unknown ptrace_scope value: {other}")),
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
/// Implements [PROFILE-PERMISSIONS-WINDOWS]: `ReadProcessMemory` works without
/// elevation for same-user processes, so this is always `Allowed`.
#[cfg(target_os = "windows")]
fn check_windows_permissions() -> PermissionStatus {
    debug!("Windows: ReadProcessMemory works for same-user processes");
    PermissionStatus::Allowed
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
//
// The actual elevation + socket streaming lives in [`super::helper_client`].
// `create_sampler` (above) routes [`PermissionStatus::ElevationRequired`] there.

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

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only: a permission probe that errors must abort naming the probe"
)]
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
        let _permissions = result.expect("permission probing reports a verdict for any PID");
    }

    // [PROFILE-PERMISSIONS-MACOS] An external (non-child) process needs elevation
    // unless we're root.
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

    /// Restricted Yama (`ptrace_scope=1`) must attempt the attach instead of
    /// denying upfront: the kernel still grants ancestors (debug-session
    /// debuggees) and `PR_SET_PTRACER` opt-ins, which the precheck cannot see.
    /// [PROFILE-PERMISSIONS-LINUX]
    #[test]
    fn restricted_ptrace_scope_attempts_attach_instead_of_denying() {
        assert_eq!(classify_ptrace_scope(0), PermissionStatus::Allowed);
        assert_eq!(
            classify_ptrace_scope(1),
            PermissionStatus::Allowed,
            "scope=1 (default Ubuntu) must not falsely deny debuggee/opt-in targets"
        );
        assert!(
            matches!(classify_ptrace_scope(2), PermissionStatus::Denied(ref r) if r.contains("setcap")),
            "scope=2 stays denied with the setcap remedy"
        );
        assert!(
            matches!(classify_ptrace_scope(3), PermissionStatus::Denied(ref r) if r.contains("debug session")),
            "scope=3 stays denied, pointing at debug sessions"
        );
    }

    /// A real EPERM from py-spy must come back with the Linux remedies (#81).
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_permission_failures_carry_ptrace_remedies() {
        let err = with_platform_guidance(SamplerError::PermissionDenied(
            "Operation not permitted".to_owned(),
        ));
        let msg = err.to_string();
        assert!(
            msg.contains("ptrace") && msg.contains("setcap"),
            "guidance must name the remedies: {msg}"
        );
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

    // [PROFILE-PERMISSIONS] A child PID is Allowed (sampled in-process, no helper).
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn child_process_does_not_require_elevation() -> Result<(), String> {
        let child = std::process::Command::new("sleep").arg("10").spawn();

        if let Ok(mut child_proc) = child {
            let child_pid = child_proc.id();
            let status = check_profiling_permissions(child_pid)
                .map_err(|err| format!("check failed: {err}"))?;
            assert_eq!(
                status,
                PermissionStatus::Allowed,
                "a child process must be sampled in-process, with no elevation"
            );
            let _ = child_proc.kill();
            let _ = child_proc.wait();
        }
        Ok(())
    }

    /// Off macOS there is no elevation path; the helper-backed sampler must say so.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn elevated_sampler_unsupported_off_macos() -> Result<(), String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("build runtime: {err}"))?;
        let config = SamplerConfig {
            pid: std::process::id(),
            sample_rate: 100,
            include_native: false,
            include_idle: false,
            duration: None,
        };
        let result = runtime.block_on(start_elevated_sampler(&config));
        assert!(
            matches!(result, Err(SamplerError::PermissionDenied(ref msg)) if msg.contains("macOS")),
            "off macOS elevation must be reported as unsupported, got: {result:?}"
        );
        Ok(())
    }
}
