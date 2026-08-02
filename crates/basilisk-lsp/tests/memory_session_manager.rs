//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
//!
//! Coarse e2e for the memory-profiling round-trip state machine.
//!
//! The LSP holds no DAP connection — the editor runs the injection scripts and
//! couriers their raw stdout back via `basilisk.memory.ingest`. The
//! [`MemorySessionManager`] is the server-side brain of that round-trip: it
//! marker-dispatches the output to the existing parsers, accumulates
//! cross-snapshot leak history per session, and produces diagnostics to
//! publish. These tests drive that engine with the exact marker-prefixed output
//! the Python injection scripts in `scripts.rs` emit.

use basilisk_lsp::profiler::memory::session::{IngestOutcome, IngestResult, MemorySessionManager};

/// A `__BASILISK_MEM__` snapshot line with one 24 MB allocation site.
const SNAPSHOT_OUTPUT: &str = r#"noise before
__BASILISK_MEM__{"current": 45678912, "peak": 50000000, "gcObjects": 14523, "gcCounts": [712, 45, 3], "stats": [{"file": "/tmp/app.py", "line": 42, "size": 24567890, "count": 15234, "traceback": [{"file": "/tmp/app.py", "line": 42}]}]}
noise after"#;

/// A `__BASILISK_MEM_DIFF__` line with one growing site at /tmp/cache.py:34.
const DIFF_OUTPUT: &str = r#"__BASILISK_MEM_DIFF__{"leaks": [{"file": "/tmp/cache.py", "line": 34, "sizeDiff": 5000, "countDiff": 100, "size": 1000000, "count": 500, "traceback": [{"file": "/tmp/cache.py", "line": 34}]}], "current": 60000000, "peak": 70000000}"#;

fn leak_confidence(result: &IngestResult) -> Result<String, String> {
    match &result.outcome {
        IngestOutcome::Diff { leaks, .. } => {
            let first = leaks
                .first()
                .ok_or("expected at least one suspected leak")?;
            Ok(first.confidence.to_string())
        }
        other => Err(format!("expected diff outcome, got {other:?}")),
    }
}

// Exercises [PROFILE-MEMORY-CONFIDENCE]
/// A snapshot is parsed, retained, and yields an allocation diagnostic; then
/// repeated diffs of the same growing site escalate leak confidence across
/// snapshots (the whole point of the per-session `LeakTracker`).
#[tokio::test]
async fn snapshot_then_repeated_diffs_escalate_leak_confidence() -> Result<(), String> {
    let manager = MemorySessionManager::new();
    let session_id = manager.start_session(25).await;
    assert!(
        session_id.starts_with("mem-"),
        "session id should be prefixed: {session_id}"
    );

    let snap = manager.ingest(&session_id, SNAPSHOT_OUTPUT).await?;
    match snap.outcome {
        IngestOutcome::Snapshot(ref snapshot) => {
            assert_eq!(snapshot.current_memory, 45_678_912);
            assert_eq!(snapshot.peak_memory, 50_000_000);
            assert_eq!(snapshot.gc_objects, 14_523);
            assert_eq!(snapshot.top_allocations.len(), 1);
        }
        ref other => return Err(format!("expected snapshot outcome, got {other:?}")),
    }
    assert!(
        !snap.diagnostics.is_empty(),
        "a 24 MB allocation must produce BSK-MEM-ALLOC diagnostics"
    );

    // Confidence escalates Low (1) -> Medium (2 consecutive) -> High (3+).
    let first = manager.ingest(&session_id, DIFF_OUTPUT).await?;
    assert_eq!(leak_confidence(&first)?, "LOW");

    let second = manager.ingest(&session_id, DIFF_OUTPUT).await?;
    assert_eq!(leak_confidence(&second)?, "MEDIUM");

    let third = manager.ingest(&session_id, DIFF_OUTPUT).await?;
    assert_eq!(leak_confidence(&third)?, "HIGH");

    Ok(())
}

/// Ingesting against an unknown session id is an error, not a panic.
#[tokio::test]
async fn ingest_unknown_session_is_error() {
    let manager = MemorySessionManager::new();
    let result = manager.ingest("mem-does-not-exist", SNAPSHOT_OUTPUT).await;
    assert!(result.is_err(), "unknown session must be rejected");
}

/// Output with no recognized marker is rejected, and the rejection QUOTES what
/// the courier actually delivered.
///
/// Every failure of the round-trip — a render worker that raised, an editor
/// payload-file wait that expired and fell back to the bare
/// `__BASILISK_MEM_FILE__` line, a marker line that timed out mid-delivery —
/// lands on this one error. Without the evidence they are indistinguishable, and
/// a win32 flake reads as a broken injection script ([VSIX-CI-PLATFORM-COVERAGE]).
#[tokio::test]
async fn ingest_without_marker_is_error() -> Result<(), String> {
    let manager = MemorySessionManager::new();
    let session_id = manager.start_session(25).await;
    let result = manager
        .ingest(&session_id, "basilisk render worker failed: OSError(13)")
        .await;
    let message = result.err().ok_or("missing marker must be rejected")?;
    assert!(
        message.contains("no recognized __BASILISK_MEM*__ marker"),
        "error must name the missing marker: {message}"
    );
    assert!(
        message.contains("basilisk render worker failed: OSError(13)"),
        "error must quote the delivered payload: {message}"
    );
    Ok(())
}

/// A marker-less payload is folded to one line and bounded, so a truncated
/// 200 KB snapshot cannot flood the log while still naming its own size.
#[tokio::test]
async fn ingest_evidence_is_single_line_and_bounded() -> Result<(), String> {
    let manager = MemorySessionManager::new();
    let session_id = manager.start_session(25).await;
    let noise = format!("line one\nline two\n{}", "x".repeat(5_000));
    let message = manager
        .ingest(&session_id, &noise)
        .await
        .err()
        .ok_or("missing marker must be rejected")?;
    assert!(
        !message.contains('\n'),
        "evidence must be one line: {message}"
    );
    assert!(
        message.contains("line one line two"),
        "control characters must fold to spaces: {message}"
    );
    assert!(
        message.contains(&format!("({} bytes total)", noise.len())),
        "a truncated excerpt must state the real payload size: {message}"
    );
    assert!(
        message.len() < 600,
        "evidence must stay bounded, got {} chars",
        message.len()
    );
    Ok(())
}

/// Empty output says so rather than quoting nothing at all.
#[tokio::test]
async fn ingest_empty_output_reports_empty_evidence() -> Result<(), String> {
    let manager = MemorySessionManager::new();
    let session_id = manager.start_session(25).await;
    let message = manager
        .ingest(&session_id, "   \n\t ")
        .await
        .err()
        .ok_or("empty output must be rejected")?;
    assert!(
        message.contains("<empty>"),
        "empty output must be named as empty: {message}"
    );
    Ok(())
}
