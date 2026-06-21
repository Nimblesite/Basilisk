//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
//! Shared test utilities for basilisk-lsp integration tests.

use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Maximum time a test Python process is allowed to live (30 seconds).
/// After this, the process is forcefully killed regardless of test outcome.
const MAX_PYTHON_LIFETIME: Duration = Duration::from_secs(30);

/// RAII guard that kills a child process on drop.
///
/// This prevents orphaned Python processes when tests panic or fail before
/// reaching their cleanup code. Every test that spawns a child process
/// MUST use this guard.
pub struct ProcessGuard {
    child: Child,
    spawned_at: Instant,
}

impl ProcessGuard {
    /// Wrap a child process in a guard that kills it on drop.
    #[must_use]
    pub fn new(child: Child) -> Self {
        Self {
            child,
            spawned_at: Instant::now(),
        }
    }

    /// Access the underlying child process mutably (e.g., to read stdout).
    pub fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    /// Get the child's PID.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Check if the process has exceeded its maximum lifetime.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.spawned_at.elapsed() > MAX_PYTHON_LIFETIME
    }

    /// Kill the child process explicitly and wait for it.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── Real-Python helpers ──────────────────────────────────────────────────────
//
// Shared by the profiling e2e suites that run the ACTUAL generated injection
// scripts inside a real interpreter (no mocks). Written without
// `unwrap`/`expect`/`panic` so the module stays clippy-clean under every
// consuming binary's lint configuration; failures degrade to a `None`/`Vec`
// the caller treats as a SKIP rather than a hard error.

/// Path to the Python 3 interpreter, overridable via `PYTHON` / `BASILISK_PYTHON`.
#[must_use]
pub fn python_path() -> String {
    std::env::var("PYTHON")
        .or_else(|_| std::env::var("BASILISK_PYTHON"))
        .unwrap_or_else(|_| "python3".to_owned())
}

/// A collision-free temp path (`prefix_pid_nanos_seq.ext`).
fn unique_temp_path(prefix: &str, ext: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let pid = std::process::id();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{pid}_{nanos}_{seq}.{ext}"))
}

/// Mint a unique temp file path (deleted by the caller) for ad-hoc fixtures
/// such as the memory profiler's at-exit final-snapshot file.
#[must_use]
pub fn unique_temp_file(prefix: &str, ext: &str) -> PathBuf {
    unique_temp_path(prefix, ext)
}

/// The captured result of running a Python program to completion.
pub struct PythonRun {
    /// The real `.py` file the program ran as (its frames carry this path).
    pub script_path: PathBuf,
    /// Captured stdout, UTF-8 lossy.
    pub stdout: String,
    /// Captured stderr, UTF-8 lossy.
    pub stderr: String,
    /// Whether the interpreter exited 0.
    pub success: bool,
}

impl PythonRun {
    /// The base file name the interpreter saw (what `tracemalloc` records).
    #[must_use]
    pub fn script_file_name(&self) -> String {
        self.script_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned()
    }
}

impl Drop for PythonRun {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.script_path);
    }
}

/// Write `src` to a real temp `.py` file and run it to completion, capturing
/// its output. Returns `None` (a SKIP) if the file cannot be written or the
/// interpreter cannot be spawned.
///
/// A real file — not `python -c` — is used so `tracemalloc` frames carry a
/// genuine source path: the snapshot scripts drop synthetic `<string>` frames,
/// so `-c` code would be filtered out of every allocation.
#[must_use]
pub fn run_python_program(src: &str) -> Option<PythonRun> {
    let script_path = unique_temp_path("basilisk_pyrun", "py");
    if std::fs::write(&script_path, src).is_err() {
        return None;
    }
    let output = std::process::Command::new(python_path())
        .arg(&script_path)
        .output()
        .ok()?;
    Some(PythonRun {
        script_path,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Marker a memory script prints when it couriers a payload back via a file.
const MEM_FILE_MARKER: &str = "__BASILISK_MEM_FILE__";

/// Recover every memory-script payload from a program's stdout, playing the
/// editor's courier role.
///
/// Every JSON-emitting script in `profiler::memory::scripts` writes its
/// `marker + json` to a temp file and prints only `__BASILISK_MEM_FILE__<path>`
/// (a short line debugpy never truncates). This reads each referenced file back
/// — in emission order — and returns the payloads, deleting the temp files.
/// Bare `__BASILISK_MEM_OK__` acks (which are not couriered via a file) are
/// intentionally ignored so callers see exactly the JSON payloads, in order.
#[must_use]
pub fn harvest_mem_payloads(stdout: &str) -> Vec<String> {
    let mut payloads = Vec::new();
    for line in stdout.lines() {
        if let Some(idx) = line.find(MEM_FILE_MARKER) {
            let path = line[idx + MEM_FILE_MARKER.len()..].trim();
            if let Ok(contents) = std::fs::read_to_string(path) {
                payloads.push(contents);
                let _ = std::fs::remove_file(path);
            }
        }
    }
    payloads
}
