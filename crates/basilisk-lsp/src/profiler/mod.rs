//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! CPU profiling engine for the Basilisk LSP.
//!
//! Embeds py-spy as a Rust library to sample Python process call stacks
//! and surfaces hotspots as LSP diagnostics, speedscope JSON, and flamegraph SVG.
//!
//! # Architecture
//!
//! - [`ProfileSessionManager`] owns active profiling sessions (one per PID).
//! - Each session spawns a [`sampler::SamplerHandle`] on a dedicated OS thread.
//! - Samples flow through an mpsc channel into [`aggregator::ProfileData`].
//! - On stop/snapshot, data is exported via [`export`] and [`diagnostics`].

pub mod aggregator;
pub mod commands;
pub mod cpuprofile;
pub mod diagnostics;
pub mod export;
/// Elevated-helper-over-Unix-socket sampling path (Unix only). See [`helper_client`].
#[cfg(unix)]
pub mod helper_client;
pub mod memory;
pub mod presets;
pub mod privilege;
pub mod processes;
pub mod sampler;

#[cfg(test)]
mod benchmarks;
#[cfg(test)]
mod pipeline_tests;
#[cfg(test)]
mod scenario_tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use tokio::sync::Mutex;
use tracing::{debug, info};

use aggregator::{HotspotConfig, ProfileData};
use sampler::{SamplerConfig, SamplerError, SamplerHandle};

/// Errors that can occur during profiling.
#[derive(Debug)]
pub enum ProfileError {
    /// py-spy sampler failed to attach.
    Sampler(SamplerError),
    /// Already profiling this PID.
    AlreadyProfiling {
        /// The PID.
        pid: u32,
        /// Existing session ID.
        session_id: String,
    },
    /// No active session with this ID.
    SessionNotFound(String),
    /// Export or IO error.
    ExportFailed(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sampler(err) => write!(f, "{err}"),
            Self::AlreadyProfiling { pid, session_id } => {
                write!(f, "Already profiling PID {pid} (session {session_id})")
            }
            Self::SessionNotFound(id) => write!(f, "No active profiling session: {id}"),
            Self::ExportFailed(msg) => write!(f, "Export failed: {msg}"),
        }
    }
}

impl std::error::Error for ProfileError {}

/// Map sampler errors to LSP JSON-RPC error codes per spec.
impl ProfileError {
    /// Return the JSON-RPC error code for this error.
    #[must_use]
    pub fn error_code(&self) -> i64 {
        match self {
            Self::Sampler(SamplerError::ProcessNotFound(_)) => -32001,
            Self::Sampler(SamplerError::NotPython(_)) => -32002,
            Self::Sampler(SamplerError::PermissionDenied(_)) => -32003,
            Self::AlreadyProfiling { .. } => -32004,
            Self::Sampler(SamplerError::AttachFailed(_)) => -32005,
            Self::SessionNotFound(_) | Self::ExportFailed(_) => -32000,
        }
    }
}

/// An active profiling session with its sampler thread handle.
struct ProfileSession {
    /// Unique session identifier.
    session_id: String,
    /// Target process ID.
    pid: u32,
    /// Python version detected by py-spy.
    python_version: String,
    /// When profiling started (monotonic).
    started_at: Instant,
    /// ISO 8601 start timestamp.
    started_at_iso: String,
    /// Accumulated profiling data.
    data: ProfileData,
    /// Handle to the sampler thread (owns the mpsc receiver).
    sampler: SamplerHandle,
    /// Hotspot configuration.
    hotspot_config: HotspotConfig,
    /// Seconds per sample (1.0 / `sample_rate`).
    sample_weight: f64,
    /// Samples per second (used to emit integer microsecond timeDeltas).
    sample_rate: u64,
    /// Whether idle threads are included.
    include_idle: bool,
}

/// Information about an active profiling session (returned by `list`).
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Unique session identifier.
    pub session_id: String,
    /// Target process ID.
    pub pid: u32,
    /// Python version string.
    pub python_version: String,
    /// ISO 8601 start timestamp.
    pub started_at: String,
    /// Samples collected so far.
    pub sample_count: u64,
    /// Duration in seconds.
    pub duration: f64,
}

/// A live progress sample for [PROFILE-NOTIFICATIONS-PROGRESS].
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    /// Session being reported.
    pub session_id: String,
    /// Samples collected so far.
    pub sample_count: u64,
    /// Seconds since the session started.
    pub duration: f64,
    /// Hottest function seen so far (empty until data crosses thresholds).
    pub top_function: String,
}

/// Result of starting a profiling session.
#[derive(Debug, Clone)]
pub struct StartResult {
    /// Unique session ID.
    pub session_id: String,
    /// Target PID.
    pub pid: u32,
    /// Python version string.
    pub python_version: String,
    /// ISO 8601 start timestamp.
    pub started_at: String,
}

/// Result of stopping or snapshotting a profiling session.
#[derive(Debug, Clone)]
pub struct StopResult {
    /// Session ID.
    pub session_id: String,
    /// Duration in seconds.
    pub duration: f64,
    /// Total samples collected.
    pub total_samples: u64,
    /// Full aggregated profile data.
    pub data: ProfileData,
    /// Hot functions above threshold.
    pub hot_functions: Vec<aggregator::HotFunction>,
    /// Hot lines above threshold.
    pub hot_lines: Vec<aggregator::HotLine>,
    /// Hotspot config used.
    pub hotspot_config: HotspotConfig,
    /// Seconds per sample.
    pub sample_weight: f64,
    /// Samples per second.
    pub sample_rate: u64,
}

/// Manages active profiling sessions for the LSP.
///
/// One session per PID. Lives on `LspServer` alongside `DebugSessionManager`.
/// Cloning shares the session table, so a clone can drive the progress
/// notifier task ([PROFILE-NOTIFICATIONS-PROGRESS]).
#[derive(Clone)]
pub struct ProfileSessionManager {
    /// Active sessions keyed by session ID (shared across clones).
    sessions: Arc<Mutex<HashMap<String, ProfileSession>>>,
}

impl std::fmt::Debug for ProfileSessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileSessionManager")
            .finish_non_exhaustive()
    }
}

impl Default for ProfileSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileSessionManager {
    /// Create a new profiler session manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start profiling a Python process.
    ///
    /// # Errors
    ///
    /// Returns `ProfileError` if py-spy cannot attach or the PID is already
    /// being profiled.
    pub async fn start(
        &self,
        pid: u32,
        sample_rate: Option<u64>,
        include_native: Option<bool>,
        duration: Option<std::time::Duration>,
    ) -> Result<StartResult, ProfileError> {
        if pid == 0 {
            return Err(ProfileError::Sampler(SamplerError::ProcessNotFound(pid)));
        }

        let mut sessions = self.sessions.lock().await;

        // Reject duplicate PID.
        for session in sessions.values() {
            if session.pid == pid {
                return Err(ProfileError::AlreadyProfiling {
                    pid,
                    session_id: session.session_id.clone(),
                });
            }
        }

        let rate = sample_rate.unwrap_or(100);
        let config = SamplerConfig {
            pid,
            sample_rate: rate,
            include_native: include_native.unwrap_or(false),
            include_idle: false,
            duration,
        };

        // Pick the right sampler: in-process when we have access, or the
        // elevated helper over a Unix socket when macOS requires `vm_read`.
        let sampler = privilege::create_sampler(&config).await?;
        let python_version = sampler.python_version.clone();
        let session_id = generate_session_id();
        let started_at_iso = iso_now();

        info!(
            session_id = %session_id,
            pid,
            python_version = %python_version,
            sample_rate = rate,
            "profiling session started"
        );

        let result = StartResult {
            session_id: session_id.clone(),
            pid,
            python_version: python_version.clone(),
            started_at: started_at_iso.clone(),
        };

        let sample_weight = 1.0 / f64::from(u32::try_from(rate).unwrap_or(100));

        let _ = sessions.insert(
            session_id.clone(),
            ProfileSession {
                session_id,
                pid,
                python_version,
                started_at: Instant::now(),
                started_at_iso,
                data: ProfileData::default(),
                sampler,
                hotspot_config: HotspotConfig::default(),
                sample_weight,
                sample_rate: rate,
                include_idle: false,
            },
        );

        Ok(result)
    }

    /// Stop a profiling session and return the results.
    ///
    /// # Errors
    ///
    /// Returns `ProfileError::SessionNotFound` if the session ID is unknown.
    pub async fn stop(&self, session_id: &str) -> Result<StopResult, ProfileError> {
        let mut sessions = self.sessions.lock().await;
        let mut session = sessions
            .remove(session_id)
            .ok_or_else(|| ProfileError::SessionNotFound(session_id.to_owned()))?;

        // Signal sampler to stop and drain remaining samples.
        session.sampler.stop();
        drain_samples(&mut session);

        let duration = session.started_at.elapsed().as_secs_f64();
        let hot_functions = session.data.hot_functions(&session.hotspot_config);
        let hot_lines = session.data.hot_lines(&session.hotspot_config);

        info!(
            session_id,
            duration,
            total_samples = session.data.total_samples,
            hot_functions = hot_functions.len(),
            hot_lines = hot_lines.len(),
            "profiling session stopped"
        );

        Ok(StopResult {
            session_id: session_id.to_owned(),
            duration,
            total_samples: session.data.total_samples,
            data: session.data,
            hot_functions,
            hot_lines,
            hotspot_config: session.hotspot_config,
            sample_weight: session.sample_weight,
            sample_rate: session.sample_rate,
        })
    }

    /// Take a snapshot without stopping the session.
    ///
    /// # Errors
    ///
    /// Returns `ProfileError::SessionNotFound` if the session ID is unknown.
    pub async fn snapshot(&self, session_id: &str) -> Result<StopResult, ProfileError> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| ProfileError::SessionNotFound(session_id.to_owned()))?;

        drain_samples(session);

        let duration = session.started_at.elapsed().as_secs_f64();
        let hot_functions = session.data.hot_functions(&session.hotspot_config);
        let hot_lines = session.data.hot_lines(&session.hotspot_config);

        debug!(
            session_id,
            total_samples = session.data.total_samples,
            "snapshot taken"
        );

        Ok(StopResult {
            session_id: session_id.to_owned(),
            duration,
            total_samples: session.data.total_samples,
            data: session.data.clone(),
            hot_functions,
            hot_lines,
            hotspot_config: session.hotspot_config.clone(),
            sample_weight: session.sample_weight,
            sample_rate: session.sample_rate,
        })
    }

    /// Drain pending samples and report live progress for one session, or
    /// `None` once the session has ended (which stops the notifier task).
    /// Implements [PROFILE-NOTIFICATIONS-PROGRESS].
    pub async fn progress(&self, session_id: &str) -> Option<ProgressInfo> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions.get_mut(session_id)?;
        drain_samples(session);
        let top_function = session
            .data
            .hot_functions(&session.hotspot_config)
            .first()
            .map(|hot| hot.name.clone())
            .unwrap_or_default();
        Some(ProgressInfo {
            session_id: session.session_id.clone(),
            sample_count: session.data.total_samples,
            duration: session.started_at.elapsed().as_secs_f64(),
            top_function,
        })
    }

    /// List all active profiling sessions.
    pub async fn list(&self) -> Vec<SessionInfo> {
        let mut sessions = self.sessions.lock().await;
        sessions
            .values_mut()
            .map(|session| {
                drain_samples(session);
                SessionInfo {
                    session_id: session.session_id.clone(),
                    pid: session.pid,
                    python_version: session.python_version.clone(),
                    started_at: session.started_at_iso.clone(),
                    sample_count: session.data.total_samples,
                    duration: session.started_at.elapsed().as_secs_f64(),
                }
            })
            .collect()
    }

    /// Stop all active profiling sessions. Called on LSP shutdown.
    pub async fn stop_all(&self) {
        let mut sessions = self.sessions.lock().await;
        let count = sessions.len();
        if count > 0 {
            info!(count, "stopping all profiling sessions");
        }
        for (id, session) in sessions.drain() {
            debug!(session_id = %id, "stopping profiling session");
            session.sampler.stop();
        }
    }
}

/// Drain all pending sample batches from the mpsc channel into `ProfileData`.
fn drain_samples(session: &mut ProfileSession) {
    let mut drained = 0u64;
    while let Ok(batch) = session.sampler.receiver.try_recv() {
        session
            .data
            .ingest_traces(&batch.traces, session.sample_weight, session.include_idle);
        drained += 1;
    }
    if drained > 0 {
        debug!(
            session_id = %session.session_id,
            batches = drained,
            total_samples = session.data.total_samples,
            "drained sample batches"
        );
    }
}

/// Generate a unique session ID.
fn generate_session_id() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.subsec_nanos();
    let secs_low = u32::try_from(now.as_secs()).unwrap_or(u32::MAX);
    let mixed = nanos.wrapping_mul(2_654_435_761).wrapping_add(secs_low);
    format!("prof-{mixed:08x}")
}

/// Return the current time as an ISO 8601 UTC string.
fn iso_now() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Calculate UTC date/time without chrono.
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Days since epoch to Y-M-D (simplified Gregorian).
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from Howard Hinnant's `chrono`-compatible date library.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let yoe = (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let y = yoe + era * 400;
    let day_of_year = day_of_era - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let d = day_of_year - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
