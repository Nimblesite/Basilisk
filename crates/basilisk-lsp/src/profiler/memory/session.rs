//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
//!
//! Memory-profiling session state — the server-side brain of the ingest
//! round-trip.
//!
//! The LSP holds no DAP connection (the editor owns it), so memory profiling is
//! a two-leg round-trip: the LSP hands the editor a Python injection script
//! (see [`super::scripts`]), the editor runs it in the debuggee via DAP
//! `evaluate`, and couriers the raw stdout back through `basilisk.memory.ingest`.
//!
//! [`MemorySessionManager`] owns the cross-call state the stateless parsers and
//! scorers can't hold on their own: the per-session [`LeakTracker`] that
//! escalates leak confidence across diffs, the last snapshot, and the
//! [`MemoryTimeline`]. Each ingest marker-dispatches the output to the existing
//! parser and returns both the structured outcome and the diagnostics to publish
//! — no parsing logic is duplicated here; this is thin orchestration glue.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime};

use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{debug, info};

use super::diagnostics::{self, DiagnosticsByUri, GcCollectResult, MemoryHotspotConfig};
use super::diff::{parse_diff_output, MemoryDiff};
use super::leaks::{LeakTracker, SuspectedLeak};
use super::timeline::{AutoSnapshotConfig, MemoryTimeline};
use super::{
    extract_marker_json, parse_objects_output, parse_refs_output, parse_snapshot_output,
    MemorySnapshot, DIFF_MARKER, GC_MARKER, OBJECTS_MARKER, OK_MARKER, REFS_MARKER,
    SNAPSHOT_MARKER,
};

/// The marker-dispatched outcome of ingesting one memory-script's output.
#[derive(Debug)]
pub enum IngestOutcome {
    /// A `tracemalloc` snapshot (`__BASILISK_MEM__`).
    Snapshot(MemorySnapshot),
    /// A snapshot diff with leak-confidence scoring (`__BASILISK_MEM_DIFF__`).
    Diff {
        /// The parsed growth/free diff.
        diff: MemoryDiff,
        /// Suspected leaks scored against this session's accumulated history.
        leaks: Vec<SuspectedLeak>,
    },
    /// A gc-collection result (`__BASILISK_MEM_GC__`).
    Gc(GcCollectResult),
    /// A reference-graph payload, passed through to the editor (`__BASILISK_MEM_REFS__`).
    Refs(Value),
    /// An objects-by-type payload, passed through to the editor (`__BASILISK_MEM_OBJECTS__`).
    Objects(Value),
    /// A bare acknowledgment (`__BASILISK_MEM_OK__`), e.g. from start/stop scripts.
    Ack,
}

/// The structured result of a single ingest, plus diagnostics to publish.
#[derive(Debug)]
pub struct IngestResult {
    /// The typed outcome the editor renders.
    pub outcome: IngestOutcome,
    /// Diagnostics to publish via `textDocument/publishDiagnostics`, keyed by URI.
    /// Empty for outcomes that produce none (refs, objects, ack).
    pub diagnostics: DiagnosticsByUri,
}

/// Per-session memory-profiling state.
struct MemorySession {
    /// Unique session identifier (`mem-XXXXXXXX`).
    session_id: String,
    /// When the session started (for snapshot ids and timeline elapsed time).
    started_at: Instant,
    /// Most recent parsed snapshot (for UI summaries / future baseline use).
    last_snapshot: Option<MemorySnapshot>,
    /// Cross-diff leak-confidence accumulator.
    leak_tracker: LeakTracker,
    /// Rolling memory timeline for the dashboard chart.
    timeline: MemoryTimeline,
    /// Allocation-hotspot thresholds for diagnostics.
    hotspot_config: MemoryHotspotConfig,
    /// Number of snapshots ingested so far (for snapshot ids).
    snapshot_count: u64,
}

impl MemorySession {
    fn new(session_id: String) -> Self {
        let mut timeline = MemoryTimeline::new(AutoSnapshotConfig::default());
        timeline.start();
        Self {
            session_id,
            started_at: Instant::now(),
            last_snapshot: None,
            leak_tracker: LeakTracker::new(),
            timeline,
            hotspot_config: MemoryHotspotConfig::default(),
            snapshot_count: 0,
        }
    }

    /// Marker-dispatch the raw script output to the matching parser.
    ///
    /// Implements [PROFILE-MEMORY-INGEST] (server side): detects the
    /// `__BASILISK_MEM*__` marker, parses with the existing parsers, scores leaks
    /// via the per-session `LeakTracker`, and returns the kind-tagged outcome plus
    /// the diagnostics to publish. A marker-less payload is an error.
    fn ingest(&mut self, output: &str) -> Result<IngestResult, String> {
        // Dispatch to the marker that appears EARLIEST in the output. Each
        // script prints exactly one marker at the start of its payload line; an
        // object `repr` embedded later in the JSON could contain a marker-like
        // substring, so position — not mere presence — selects the true marker.
        let detected = [
            (DIFF_MARKER, MarkerKind::Diff),
            (GC_MARKER, MarkerKind::Gc),
            (REFS_MARKER, MarkerKind::Refs),
            (OBJECTS_MARKER, MarkerKind::Objects),
            (SNAPSHOT_MARKER, MarkerKind::Snapshot),
            (OK_MARKER, MarkerKind::Ack),
        ]
        .into_iter()
        .filter_map(|(marker, kind)| output.find(marker).map(|idx| (idx, kind)))
        .min_by_key(|(idx, _)| *idx)
        .map(|(_, kind)| kind);

        match detected {
            Some(MarkerKind::Diff) => self.ingest_diff(output),
            Some(MarkerKind::Gc) => self.ingest_gc(output),
            Some(MarkerKind::Refs) => Ok(no_diagnostics(IngestOutcome::Refs(parse_refs_output(
                output,
            )?))),
            Some(MarkerKind::Objects) => Ok(no_diagnostics(IngestOutcome::Objects(
                parse_objects_output(output)?,
            ))),
            Some(MarkerKind::Snapshot) => self.ingest_snapshot(output),
            Some(MarkerKind::Ack) => {
                debug!(session_id = %self.session_id, "ingested ack");
                Ok(no_diagnostics(IngestOutcome::Ack))
            }
            None => Err(format!(
                "no recognized __BASILISK_MEM*__ marker in script output: {}",
                marker_less_evidence(output)
            )),
        }
    }

    fn ingest_snapshot(&mut self, output: &str) -> Result<IngestResult, String> {
        let snapshot_id = format!("{}-snap-{}", self.session_id, self.snapshot_count);
        self.snapshot_count += 1;
        let snapshot = parse_snapshot_output(output, &snapshot_id)?;
        let diagnostics =
            diagnostics::generate_allocation_diagnostics(&snapshot, &self.hotspot_config);
        self.timeline.record(&snapshot);
        self.last_snapshot = Some(snapshot.clone());
        info!(
            session_id = %self.session_id,
            current = snapshot.current_memory,
            allocations = snapshot.top_allocations.len(),
            "ingested memory snapshot"
        );
        Ok(IngestResult {
            outcome: IngestOutcome::Snapshot(snapshot),
            diagnostics,
        })
    }

    fn ingest_diff(&mut self, output: &str) -> Result<IngestResult, String> {
        let json = extract_marker_json(output, DIFF_MARKER)?;
        let diff = match parse_diff_output(json) {
            Ok(diff) => diff,
            // The very first diff has no baseline yet — the injection script
            // seeds it for the next call. Surface a clean empty diff rather than
            // a hard error so the editor shows "0 leaks" instead of a scary
            // message on the first "Compare Snapshots".
            Err(err) if err.contains("no previous snapshot") => empty_diff(),
            Err(err) => return Err(err),
        };
        // `generate_diff_diagnostics` scores once and returns both the leaks
        // (for the outcome) and the diagnostics, so confidence isn't corrupted.
        let (leaks, diagnostics) =
            diagnostics::generate_diff_diagnostics(&diff, &mut self.leak_tracker);
        info!(
            session_id = %self.session_id,
            growths = diff.grown_allocations.len(),
            suspected = leaks.len(),
            "ingested memory diff"
        );
        Ok(IngestResult {
            outcome: IngestOutcome::Diff { diff, leaks },
            diagnostics,
        })
    }

    fn ingest_gc(&mut self, output: &str) -> Result<IngestResult, String> {
        let json = extract_marker_json(output, GC_MARKER)?;
        let gc = diagnostics::parse_gc_result(json)?;
        let diagnostics = diagnostics::generate_cycle_diagnostics(&gc);
        info!(
            session_id = %self.session_id,
            uncollectable = gc.uncollectable_count,
            "ingested gc result"
        );
        Ok(IngestResult {
            outcome: IngestOutcome::Gc(gc),
            diagnostics,
        })
    }
}

/// Which marker a script's output carries — selected by earliest position.
#[derive(Debug, Clone, Copy)]
enum MarkerKind {
    Snapshot,
    Diff,
    Gc,
    Refs,
    Objects,
    Ack,
}

/// An empty diff, used when the first comparison has no baseline yet.
fn empty_diff() -> MemoryDiff {
    MemoryDiff {
        total_growth: 0,
        total_freed: 0,
        net_growth: 0,
        grown_allocations: Vec::new(),
        freed_allocations: Vec::new(),
    }
}

/// Longest marker-less payload excerpt quoted back in the ingest error. Long
/// enough to carry a Python `repr` or an editor fallback line whole, short
/// enough that a truncated 200 KB snapshot cannot flood the log.
const EVIDENCE_LIMIT: usize = 400;

/// What the courier actually delivered, as one line, for a marker-less ingest.
///
/// Every distinct way this round-trip can fail — the render worker raising, the
/// editor's payload-file wait expiring and falling back to the bare
/// `__BASILISK_MEM_FILE__` line, a marker-line delivery that timed out
/// mid-flight — arrives here as the same "no recognized marker" rejection. With
/// nothing quoted back, all three read as a broken injection script and the
/// win32 flake could not be told apart from a real regression
/// ([VSIX-CI-PLATFORM-COVERAGE]). Newlines are folded so the evidence stays one
/// grep-able line, and the excerpt is bounded by [`EVIDENCE_LIMIT`].
fn marker_less_evidence(output: &str) -> String {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return "<empty>".to_owned();
    }
    let folded: String = trimmed
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    match folded.char_indices().nth(EVIDENCE_LIMIT) {
        Some((end, _)) => format!("{}… ({} bytes total)", &folded[..end], output.len()),
        None => folded,
    }
}

/// Wrap an outcome that produces no diagnostics.
fn no_diagnostics(outcome: IngestOutcome) -> IngestResult {
    IngestResult {
        outcome,
        diagnostics: DiagnosticsByUri::new(),
    }
}

/// Upper bound on retained memory sessions. The editor tracks one at a time,
/// but repeated start-without-stop cycles would otherwise accumulate entries;
/// the oldest is evicted past this cap so growth stays bounded.
const MAX_SESSIONS: usize = 32;

/// Manages active memory-profiling sessions for the LSP.
///
/// One session per `memorySessionId`. Lives on `LspServer` alongside
/// `ProfileSessionManager` and `DebugSessionManager`.
pub struct MemorySessionManager {
    sessions: Mutex<HashMap<String, MemorySession>>,
    /// Monotonic counter mixed into session ids so two sessions minted in the
    /// same nanosecond cannot collide.
    next_seq: AtomicU64,
}

impl std::fmt::Debug for MemorySessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemorySessionManager")
            .finish_non_exhaustive()
    }
}

impl Default for MemorySessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySessionManager {
    /// Create a new memory session manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            next_seq: AtomicU64::new(0),
        }
    }

    /// Begin a new memory-tracking session and return its id.
    ///
    /// `traceback_depth` is the `tracemalloc` frame depth the editor will inject;
    /// it is logged for diagnostics but the script itself is generated by the
    /// command handler from [`super::scripts::start_tracemalloc`].
    pub async fn start_session(&self, traceback_depth: u32) -> String {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let session_id = generate_memory_session_id(seq);
        info!(session_id = %session_id, traceback_depth, "memory session started");
        let mut sessions = self.sessions.lock().await;
        let _ = sessions.insert(session_id.clone(), MemorySession::new(session_id.clone()));
        evict_oldest_over_cap(&mut sessions);
        session_id
    }

    /// Ingest raw script output for a session, returning the structured outcome
    /// and the diagnostics to publish.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is unknown, no marker is present, or the
    /// payload JSON is malformed.
    pub async fn ingest(&self, session_id: &str, output: &str) -> Result<IngestResult, String> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("unknown memory session: {session_id}"))?;
        session.ingest(output)
    }
}

/// Evict the oldest session(s) once the cap is exceeded (keeps growth bounded).
fn evict_oldest_over_cap(sessions: &mut HashMap<String, MemorySession>) {
    while sessions.len() > MAX_SESSIONS {
        let oldest = sessions
            .values()
            .min_by_key(|session| session.started_at)
            .map(|session| session.session_id.clone());
        match oldest {
            Some(id) => {
                let _ = sessions.remove(&id);
            }
            None => break,
        }
    }
}

/// Generate a unique memory session id (`mem-XXXXXXXX-SEQ`).
fn generate_memory_session_id(seq: u64) -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.subsec_nanos();
    let secs_low = u32::try_from(now.as_secs()).unwrap_or(u32::MAX);
    let mixed = nanos.wrapping_mul(2_654_435_761).wrapping_add(secs_low);
    format!("mem-{mixed:08x}-{seq:x}")
}
