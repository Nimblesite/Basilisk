//! Debug session management for the Basilisk LSP.
//!
//! Spawns debugpy on a free TCP port and tracks active sessions.
//! The editor connects directly to debugpy via DAP over TCP — the
//! LSP just brokers the connection.

use std::collections::HashMap;
use std::fmt;
use std::net::TcpListener;
use std::path::Path;
use tracing::{debug, error, info, warn};

use tokio::process::Child;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

/// Errors that can occur during debug session management.
#[derive(Debug)]
pub enum DebugError {
    /// Failed to allocate a free TCP port.
    PortAllocation(std::io::Error),
    /// Failed to spawn the Python/debugpy process.
    SpawnFailed(std::io::Error),
    /// debugpy did not start accepting connections within the timeout.
    Timeout(u16),
    /// debugpy is not installed in the Python environment.
    DebugpyNotFound(String),
    /// Python interpreter not found.
    PythonNotFound(String),
}

impl fmt::Display for DebugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PortAllocation(err) => write!(f, "Failed to allocate TCP port: {err}"),
            Self::SpawnFailed(err) => write!(f, "Failed to spawn debugpy: {err}"),
            Self::Timeout(port) => write!(
                f,
                "debugpy did not start accepting connections on port {port} within 5 seconds"
            ),
            Self::DebugpyNotFound(python) => write!(
                f,
                "debugpy not found. Install it: {python} -m pip install debugpy"
            ),
            Self::PythonNotFound(searched) => write!(
                f,
                "No Python interpreter found. Checked: {searched}. \
                 Set BASILISK_PYTHON or create a virtualenv."
            ),
        }
    }
}

impl std::error::Error for DebugError {}

/// Tracks active debug sessions spawned by the LSP.
pub struct DebugSessionManager {
    /// Map from session ID to the spawned debugpy child process.
    sessions: Mutex<HashMap<String, Child>>,
}

impl Default for DebugSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugSessionManager {
    /// Create a new debug session manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Spawn debugpy on a free TCP port. Returns (host, port, `session_id`).
    ///
    /// Waits until debugpy is accepting connections before returning,
    /// so the editor can immediately connect its DAP client.
    ///
    /// # Errors
    ///
    /// Returns `DebugError` if port allocation, process spawning, or the
    /// readiness check fails.
    pub async fn start_session(
        &self,
        python_path: &str,
    ) -> Result<(String, u16, String), DebugError> {
        let port = find_free_port()?;
        let session_id = generate_session_id();

        info!(python = python_path, port, session_id = %session_id, "spawning debugpy adapter");

        let child = tokio::process::Command::new(python_path)
            .args(["-m", "debugpy.adapter", "--port", &port.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| {
                error!(python = python_path, %err, "failed to spawn debugpy");
                DebugError::SpawnFailed(err)
            })?;

        let _ = self.sessions.lock().await.insert(session_id.clone(), child);

        // Wait for debugpy to start accepting connections (up to 5s).
        debug!(port, "waiting for debugpy to accept connections");
        wait_for_port(port, Duration::from_secs(5)).await?;

        info!(port, session_id = %session_id, "debugpy ready on localhost:{port}");
        Ok(("localhost".to_owned(), port, session_id))
    }

    /// Kill a debug session and clean up.
    pub async fn stop_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.lock().await;
        if let Some(mut child) = sessions.remove(session_id) {
            info!(session_id, "stopping debug session");
            let _ = child.kill().await;
            return true;
        }
        warn!(session_id, "stop_session called for unknown session");
        false
    }

    /// Kill all active debug sessions. Called on LSP shutdown.
    pub async fn stop_all(&self) {
        let mut sessions = self.sessions.lock().await;
        let count = sessions.len();
        if count > 0 {
            info!(count, "stopping all debug sessions");
        }
        for (id, mut child) in sessions.drain() {
            debug!(session_id = %id, "killing debug session");
            let _ = child.kill().await;
        }
    }
}

/// Find a free TCP port by binding to port 0.
fn find_free_port() -> Result<u16, DebugError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(DebugError::PortAllocation)?;
    let port = listener
        .local_addr()
        .map_err(DebugError::PortAllocation)?
        .port();
    // Dropping the listener frees the port for debugpy to bind.
    Ok(port)
}

/// Poll until something is listening on `port` — without making a connection.
///
/// debugpy.adapter in debugServer mode accepts exactly ONE TCP connection.
/// A probe connection would consume that slot, leaving the real client
/// (VS Code) with ECONNREFUSED. Instead we check readiness by trying to
/// **bind** to the port: if binding fails with `AddrInUse`, something is
/// already listening — i.e. debugpy is ready.
async fn wait_for_port(port: u16, timeout: Duration) -> Result<(), DebugError> {
    let start = Instant::now();
    loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Err(ref err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                debug!(
                    port,
                    elapsed_ms = start.elapsed().as_millis(),
                    "port occupied — debugpy ready"
                );
                return Ok(());
            }
            Ok(_listener) => {
                // We could bind → port is free → debugpy hasn't started yet.
                // Dropping the listener frees it again; retry after a short sleep.
            }
            Err(err) => {
                warn!(port, %err, "unexpected bind error during port check");
            }
        }
        if start.elapsed() > timeout {
            error!(port, "debugpy did not bind within timeout");
            return Err(DebugError::Timeout(port));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Generate a short random session ID.
fn generate_session_id() -> String {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("dbg-{nanos:08x}")
}

// ── Python resolver ─────────────────────────────────────────────────────────

/// Resolve the Python interpreter for a workspace.
///
/// Checks (in order):
/// 1. `BASILISK_PYTHON` environment variable
/// 2. Workspace virtualenv (`.venv/bin/python`, `venv/bin/python`)
/// 3. System `python3` (or `python` on Windows)
#[must_use]
pub fn resolve_python(workspace_root: &Path) -> String {
    // 1. Explicit override via env var.
    if let Ok(p) = std::env::var("BASILISK_PYTHON") {
        info!(python = %p, "using BASILISK_PYTHON env var");
        return p;
    }

    // 2. Workspace venv.
    for venv_dir in &[".venv", "venv"] {
        let bin = if cfg!(windows) {
            workspace_root
                .join(venv_dir)
                .join("Scripts")
                .join("python.exe")
        } else {
            workspace_root.join(venv_dir).join("bin").join("python")
        };
        if bin.exists() {
            let resolved = bin.to_string_lossy().into_owned();
            info!(python = %resolved, "found workspace venv");
            return resolved;
        }
    }

    // 3. System fallback.
    let fallback = if cfg!(windows) { "python" } else { "python3" };
    info!(python = fallback, "falling back to system python");
    fallback.into()
}

/// Check if debugpy is importable by the given Python interpreter.
///
/// # Errors
///
/// Returns `DebugError::SpawnFailed` if the Python process cannot be started,
/// or `DebugError::DebugpyNotFound` if the import fails.
pub async fn check_debugpy(python: &str) -> Result<(), DebugError> {
    debug!(python, "checking if debugpy is installed");
    let output = tokio::process::Command::new(python)
        .args(["-c", "import debugpy; print(debugpy.__version__)"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .map_err(DebugError::SpawnFailed)?;

    if output.status.success() {
        info!(python, "debugpy is available");
        Ok(())
    } else {
        warn!(python, "debugpy not found");
        Err(DebugError::DebugpyNotFound(python.to_owned()))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;

    #[test]
    fn find_free_port_returns_nonzero() {
        let port = find_free_port().expect("should allocate a port");
        assert!(port > 0, "port must be nonzero");
    }

    #[test]
    fn generate_session_id_is_nonempty() {
        let id = generate_session_id();
        assert!(id.starts_with("dbg-"), "id must start with dbg- prefix");
        assert!(id.len() > 4, "id must have content after prefix");
    }

    #[test]
    fn resolve_python_uses_env_var() {
        // Temporarily set the env var.
        let key = "BASILISK_PYTHON";
        let prev = std::env::var(key).ok();
        std::env::set_var(key, "/custom/python3.12");

        let result = resolve_python(Path::new("/nonexistent"));
        assert_eq!(result, "/custom/python3.12");

        // Restore.
        match prev {
            Some(val) => std::env::set_var(key, val),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn resolve_python_falls_back_to_system() {
        let key = "BASILISK_PYTHON";
        let prev = std::env::var(key).ok();
        std::env::remove_var(key);

        // Use a nonexistent workspace root so no venv is found.
        let result = resolve_python(Path::new("/nonexistent/workspace"));
        let expected = if cfg!(windows) { "python" } else { "python3" };
        assert_eq!(result, expected);

        if let Some(val) = prev {
            std::env::set_var(key, val);
        }
    }

    #[tokio::test]
    async fn check_debugpy_with_nonexistent_python_returns_err() {
        let result = check_debugpy("/nonexistent/python").await;
        assert!(result.is_err(), "nonexistent python must fail");
    }

    /// Regression test: debugpy.adapter in debugServer mode accepts exactly
    /// ONE TCP connection. The old `wait_for_port` made a probe connection to
    /// check readiness, which consumed that single slot. debugpy starts a DAP
    /// session on the probe, the probe immediately disconnects → debugpy exits
    /// → port is dead → VS Code gets ECONNREFUSED.
    ///
    /// This test simulates debugpy accurately:
    ///   1. Accept one connection (the probe from `wait_for_port`)
    ///   2. When the probe disconnects (EOF), the "adapter" exits → listener drops
    ///   3. The real client (VS Code) tries to connect → ECONNREFUSED
    ///
    /// The test MUST FAIL with a buggy `wait_for_port` that makes TCP probes.
    #[tokio::test]
    async fn wait_for_port_does_not_consume_the_connection() {
        use std::io::Read as _;

        // Start a TCP listener simulating debugpy.adapter in debugServer mode.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();

        // Simulate debugpy: accept ONE connection, start a "DAP session".
        // When that connection drops (probe disconnects), the adapter "exits"
        // and closes the listening socket — exactly like real debugpy.
        let accept_handle = tokio::task::spawn_blocking(move || {
            let (mut conn, _) = listener.accept().expect("accept");
            // Drop the listener immediately — debugpy stops accepting after one.
            drop(listener);
            // Block until the client disconnects (EOF). This simulates
            // debugpy waiting for DAP messages from its single client.
            let mut buf = [0u8; 1];
            let _ = conn.read(&mut buf); // returns Ok(0) on EOF
                                         // Adapter "exits" — drop the connection too.
            drop(conn);
        });

        // wait_for_port makes a TCP probe — debugpy accepts it as "the client".
        // The probe succeeds, wait_for_port returns Ok. But the probe's
        // TcpStream is dropped immediately → debugpy sees EOF → exits.
        wait_for_port(port, Duration::from_secs(3))
            .await
            .expect("wait_for_port should succeed");

        // Give the simulated debugpy time to process EOF and exit.
        tokio::time::sleep(Duration::from_millis(200)).await;

        // The REAL client (VS Code) tries to connect AFTER wait_for_port.
        // With the buggy implementation, the port is DEAD → ECONNREFUSED.
        let real_client = TcpStream::connect(("127.0.0.1", port)).await;
        assert!(
            real_client.is_ok(),
            "VS Code's connection must succeed after wait_for_port, \
             but got ECONNREFUSED — wait_for_port consumed the only slot"
        );
        // Drop real_client first — this sends EOF to accept_handle's conn.read(),
        // allowing the simulated debugpy thread to unblock and finish.
        drop(real_client);
        let _ = accept_handle.await;
    }
}
