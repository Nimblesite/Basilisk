//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! py-spy sampling wrapper.
//!
//! Spawns a dedicated OS thread that calls `py_spy::PythonSpy::get_stack_traces()`
//! in a loop and sends samples to the aggregator via an mpsc channel. The LSP
//! thread controls the sampler via `SamplerHandle`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use basilisk_profiler_protocol::{classify_attach_error, AttachErrorKind};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// A batch of stack traces from a single sampling tick.
#[derive(Debug)]
pub struct SampleBatch {
    /// The stack traces collected in this tick.
    pub traces: Vec<py_spy::StackTrace>,
}

/// Configuration for the sampler thread.
#[derive(Debug, Clone)]
pub struct SamplerConfig {
    /// Target process ID.
    pub pid: u32,
    /// Samples per second (default 100).
    pub sample_rate: u64,
    /// Include native (C extension) frames.
    pub include_native: bool,
    /// Include idle threads in samples.
    pub include_idle: bool,
    /// Optional duration limit — sampler stops after this.
    pub duration: Option<Duration>,
}

impl Default for SamplerConfig {
    fn default() -> Self {
        Self {
            pid: 0,
            sample_rate: 100,
            include_native: false,
            include_idle: false,
            duration: None,
        }
    }
}

/// Handle for controlling a running sampler from the LSP thread.
pub struct SamplerHandle {
    /// Signal to stop the sampler thread.
    stop_flag: Arc<AtomicBool>,
    /// Join handle for the sampler OS thread.
    join_handle: Option<std::thread::JoinHandle<()>>,
    /// Receiver for sample batches.
    pub receiver: mpsc::Receiver<SampleBatch>,
    /// The Python version detected by py-spy.
    pub python_version: String,
    /// The PID being profiled.
    pub pid: u32,
}

impl std::fmt::Debug for SamplerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamplerHandle")
            .field("python_version", &self.python_version)
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}

impl SamplerHandle {
    /// Construct a handle around an arbitrary sample-producing thread.
    ///
    /// Used by both the in-process py-spy sampler and the elevated helper-socket
    /// sampler ([`super::helper_client`]) so the rest of the pipeline treats both
    /// sources identically.
    pub(crate) fn from_parts(
        stop_flag: Arc<AtomicBool>,
        join_handle: std::thread::JoinHandle<()>,
        receiver: mpsc::Receiver<SampleBatch>,
        python_version: String,
        pid: u32,
    ) -> Self {
        Self {
            stop_flag,
            join_handle: Some(join_handle),
            receiver,
            python_version,
            pid,
        }
    }

    /// Signal the sampler to stop.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    /// Wait for the sampler thread to finish.
    pub fn join(mut self) {
        self.stop();
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SamplerHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        // Don't block in Drop — the thread will see the flag and exit.
    }
}

/// Errors that can occur during sampler creation.
#[derive(Debug)]
pub enum SamplerError {
    /// The target PID was not found.
    ProcessNotFound(u32),
    /// The target is not a Python process.
    NotPython(u32),
    /// Permission denied (macOS/Linux privilege issue).
    PermissionDenied(String),
    /// py-spy failed to attach.
    AttachFailed(String),
}

impl std::fmt::Display for SamplerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProcessNotFound(pid) => write!(f, "Process not found: PID {pid}"),
            Self::NotPython(pid) => write!(f, "PID {pid} is not a Python process"),
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            Self::AttachFailed(msg) => write!(f, "Failed to attach to process: {msg}"),
        }
    }
}

impl std::error::Error for SamplerError {}

/// Map a classified attach failure onto the matching [`SamplerError`] variant.
///
/// Shared by the in-process sampler and the elevated-helper client so both
/// attach paths report identical, distinct failure modes (issue #81).
#[must_use]
pub fn sampler_error_from_kind(kind: AttachErrorKind, pid: u32, message: String) -> SamplerError {
    match kind {
        AttachErrorKind::ProcessNotFound => SamplerError::ProcessNotFound(pid),
        AttachErrorKind::NotPython => SamplerError::NotPython(pid),
        AttachErrorKind::PermissionDenied => SamplerError::PermissionDenied(message),
        AttachErrorKind::AttachFailed => SamplerError::AttachFailed(message),
    }
}

/// Convert a u32 PID to the platform-specific type py-spy expects.
///
/// py-spy uses `remoteprocess::Pid` which is `i32` on Unix and `u32` on Windows.
fn to_pyspy_pid(pid: u32) -> py_spy::Pid {
    #[cfg(target_os = "windows")]
    {
        pid
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Safe: PIDs on Unix fit in i32 (max PID is typically 2^22).
        i32::try_from(pid).unwrap_or(0)
    }
}

/// Start a sampler thread for the given configuration.
///
/// Returns a handle that can be used to receive samples and stop the sampler.
///
/// # Errors
///
/// Returns `SamplerError` if py-spy cannot attach to the target process.
pub fn start_sampler(config: &SamplerConfig) -> Result<SamplerHandle, SamplerError> {
    let pid = to_pyspy_pid(config.pid);

    let spy_config = py_spy::Config {
        sampling_rate: config.sample_rate,
        include_idle: config.include_idle,
        native: config.include_native,
        ..Default::default()
    };

    info!(
        pid = config.pid,
        sample_rate = config.sample_rate,
        "attaching py-spy to process"
    );

    let spy = py_spy::PythonSpy::new(pid, &spy_config).map_err(|err| {
        let msg = err.to_string();
        sampler_error_from_kind(classify_attach_error(&msg), config.pid, msg)
    })?;

    let python_version = format!(
        "{}.{}.{}",
        spy.version.major, spy.version.minor, spy.version.patch
    );

    info!(
        pid = config.pid,
        python_version = %python_version,
        "attached to Python process"
    );

    let (sender, receiver) = mpsc::channel::<SampleBatch>(256);
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop_flag);
    let sample_interval = Duration::from_micros(1_000_000 / config.sample_rate.max(1));
    let duration_limit = config.duration;
    let target_pid = config.pid;

    let join_handle = std::thread::Builder::new()
        .name(format!("profiler-pid-{target_pid}"))
        .spawn(move || {
            sampler_loop(spy, &sender, &stop_clone, sample_interval, duration_limit);
        })
        .map_err(|err| {
            SamplerError::AttachFailed(format!("Failed to spawn sampler thread: {err}"))
        })?;

    Ok(SamplerHandle::from_parts(
        stop_flag,
        join_handle,
        receiver,
        python_version,
        config.pid,
    ))
}

/// The main sampling loop, running on a dedicated OS thread.
fn sampler_loop(
    mut spy: py_spy::PythonSpy,
    sender: &mpsc::Sender<SampleBatch>,
    stop_flag: &Arc<AtomicBool>,
    sample_interval: Duration,
    duration_limit: Option<Duration>,
) {
    let start = std::time::Instant::now();

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            debug!("sampler: stop flag set, exiting");
            break;
        }

        if let Some(limit) = duration_limit {
            if start.elapsed() > limit {
                info!("sampler: duration limit reached, stopping");
                break;
            }
        }

        match spy.get_stack_traces() {
            Ok(traces) => {
                if sender.blocking_send(SampleBatch { traces }).is_err() {
                    warn!("sampler: receiver dropped, exiting");
                    break;
                }
            }
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("No such process") || msg.contains("exited") {
                    info!("sampler: target process exited, stopping");
                    break;
                }
                error!(error = %msg, "sampler: failed to get stack traces");
            }
        }

        std::thread::sleep(sample_interval);
    }

    info!("sampler thread exiting");
}
