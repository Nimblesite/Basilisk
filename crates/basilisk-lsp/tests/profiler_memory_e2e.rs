//! Tests for [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
//!
//! REAL end-to-end memory profiling — no mocks, no hand-written marker strings.
//!
//! Every test here runs the ACTUAL Python injection scripts from
//! `profiler::memory::scripts` inside a real interpreter, allocates a *known*
//! amount of memory, harvests the genuine `tracemalloc`/`gc` output the editor
//! would courier back, and drives it through the real [`MemorySessionManager`]
//! ingest engine. Where the existing `memory_session_manager.rs` feeds the
//! parser fixed JSON, this proves the whole round-trip against real allocations:
//! the scripts are valid, they measure what the program actually did, the
//! courier markers line up, and the parser/leak-scorer agree with reality.
//!
//! These run on every platform (just `python3`, no ptrace / task ports), so
//! they are NOT `#[ignore]`d. They SKIP cleanly when no interpreter is present.

#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    missing_docs,
    dead_code,
    unused_imports
)]

mod common;

use basilisk_lsp::profiler::memory::scripts;
use basilisk_lsp::profiler::memory::session::{IngestOutcome, IngestResult, MemorySessionManager};
use common::{harvest_mem_payloads, run_python_program, unique_temp_file, PythonRun};

/// Max allocation sites the scripts emit — mirrors the LSP handler's constant.
const MAX_STATS: usize = 100;
/// `tracemalloc` traceback depth, mirroring the handler default.
const DEPTH: u32 = 25;

/// Join program fragments into one Python source string.
fn program(parts: &[&str]) -> String {
    parts.join("\n")
}

/// A workload that retains ~11 MB of distinct `bytes` objects at one line, so a
/// snapshot has a single dominant, attributable allocation site.
const RETAIN_11MB: &str = "_BASILISK_KEEP = [bytes(256) for _ in range(40000)]";

/// A growable global plus a `grow(n)` whose allocation site is a single stable
/// line — repeated growth there is what a leak diff must surface.
const GROWER: &str = "\
_BASILISK_GROWING = []
def _basilisk_grow(n):
    _BASILISK_GROWING.extend([bytes(256) for _ in range(n)])
";

/// Run a memory program and return the harvested payloads, or `None` to SKIP.
fn run_and_harvest(src: &str) -> Option<(PythonRun, Vec<String>)> {
    let run = run_python_program(src)?;
    assert!(
        run.success,
        "memory program must exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        run.stdout, run.stderr
    );
    let payloads = harvest_mem_payloads(&run.stdout);
    Some((run, payloads))
}

/// Confidence as an ordinal so escalation is checkable.
fn confidence_rank(label: &str) -> u8 {
    match label {
        "LOW" => 1,
        "MEDIUM" => 2,
        "HIGH" => 3,
        _ => 0,
    }
}

// ── Snapshot: real allocations are measured and attributed ───────────────────

/// Start tracemalloc, retain ~11 MB, snapshot. The ingested snapshot must
/// report real traced memory and attribute the bytes to OUR program's line —
/// proving the snapshot script, courier file hand-off, and parser all agree.
#[tokio::test]
async fn snapshot_measures_and_attributes_real_allocations() -> Result<(), String> {
    let finalfile = unique_temp_file("basilisk_final", "memfinal");
    let src = program(&[
        &scripts::start_tracemalloc(DEPTH, &finalfile.to_string_lossy(), MAX_STATS),
        RETAIN_11MB,
        &scripts::take_snapshot(MAX_STATS),
    ]);
    let Some((run, payloads)) = run_and_harvest(&src) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    let _ = std::fs::remove_file(&finalfile);

    assert_eq!(
        payloads.len(),
        1,
        "exactly one snapshot payload should be couriered, got {payloads:?}"
    );

    let manager = MemorySessionManager::new();
    let session = manager.start_session(DEPTH).await;
    let result = manager.ingest(&session, &payloads[0]).await?;

    let IngestOutcome::Snapshot(snapshot) = &result.outcome else {
        return Err(format!(
            "expected snapshot outcome, got {:?}",
            result.outcome
        ));
    };

    assert!(
        snapshot.current_memory >= 4_000_000,
        "tracemalloc must report the ~11 MB we retained, got {} bytes",
        snapshot.current_memory
    );
    assert!(
        snapshot.peak_memory >= snapshot.current_memory,
        "peak ({}) must be >= current ({})",
        snapshot.peak_memory,
        snapshot.current_memory
    );
    assert!(snapshot.gc_objects > 0, "gc object count must be populated");

    let fname = run.script_file_name();
    let ours = snapshot
        .top_allocations
        .iter()
        .find(|site| site.file.ends_with(&fname))
        .ok_or_else(|| {
            format!(
                "our program's allocation site ({fname}) must appear in top allocations: {:?}",
                snapshot
                    .top_allocations
                    .iter()
                    .map(|s| (&s.file, s.size))
                    .collect::<Vec<_>>()
            )
        })?;
    assert!(
        ours.size >= 1_000_000,
        "our retained site should hold >=1 MB, got {}",
        ours.size
    );
    assert!(
        !ours.traceback.is_empty(),
        "a real snapshot keeps the allocation's call stack"
    );

    // A snapshot must yield allocation diagnostics (the dashboard heat).
    assert!(
        !result.diagnostics.is_empty(),
        "a multi-MB allocation must produce diagnostics"
    );
    Ok(())
}

// ── At-exit final snapshot: the breakpoint-free "Run & Track Memory" flow ─────

/// [PROFILE-MEMORY-FINAL] The `atexit` hook the start script installs must write
/// a real, ingestible snapshot of the program's end state to the final-snapshot
/// file — the only result the no-breakpoint run-to-completion flow ever gets.
#[tokio::test]
async fn atexit_final_snapshot_file_captures_end_state() -> Result<(), String> {
    let finalfile = unique_temp_file("basilisk_final", "memfinal");
    let src = program(&[
        &scripts::start_tracemalloc(DEPTH, &finalfile.to_string_lossy(), MAX_STATS),
        RETAIN_11MB,
        // No snapshot script and no breakpoint: rely solely on the atexit hook.
    ]);
    let Some(run) = run_python_program(&src) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    assert!(
        run.success,
        "program must exit cleanly.\nstderr:\n{}",
        run.stderr
    );

    let payload = std::fs::read_to_string(&finalfile).map_err(|err| {
        format!("the atexit hook must write the final-snapshot file {finalfile:?}: {err}")
    })?;
    let _ = std::fs::remove_file(&finalfile);
    assert!(
        payload.contains("__BASILISK_MEM__"),
        "final file must carry the snapshot marker: {payload:.120}"
    );

    let manager = MemorySessionManager::new();
    let session = manager.start_session(DEPTH).await;
    let result = manager.ingest(&session, &payload).await?;
    let IngestOutcome::Snapshot(snapshot) = &result.outcome else {
        return Err(format!(
            "expected snapshot outcome, got {:?}",
            result.outcome
        ));
    };
    assert!(
        snapshot.current_memory >= 4_000_000,
        "the final snapshot must show the retained memory, got {}",
        snapshot.current_memory
    );
    let fname = run.script_file_name();
    assert!(
        snapshot
            .top_allocations
            .iter()
            .any(|site| site.file.ends_with(&fname)),
        "the final snapshot must attribute the program's own allocations"
    );
    Ok(())
}

// ── Diff: real growth escalates leak confidence across snapshots ─────────────

/// Grow the SAME site three times across three real diffs. The per-session
/// `LeakTracker` must escalate that site LOW → MEDIUM → HIGH — exercised here
/// against genuine `tracemalloc.compare_to` output, not a canned string.
#[tokio::test]
async fn repeated_real_growth_escalates_leak_confidence() -> Result<(), String> {
    let finalfile = unique_temp_file("basilisk_final", "memfinal");
    let src = program(&[
        &scripts::start_tracemalloc(DEPTH, &finalfile.to_string_lossy(), MAX_STATS),
        GROWER,
        "_basilisk_grow(20000)",
        scripts::store_baseline(),
        "_basilisk_grow(20000)",
        &scripts::diff_snapshot(MAX_STATS),
        "_basilisk_grow(20000)",
        &scripts::diff_snapshot(MAX_STATS),
        "_basilisk_grow(20000)",
        &scripts::diff_snapshot(MAX_STATS),
    ]);
    let Some((run, payloads)) = run_and_harvest(&src) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    let _ = std::fs::remove_file(&finalfile);
    assert_eq!(
        payloads.len(),
        3,
        "three diffs should courier three payloads, got {}",
        payloads.len()
    );

    let fname = run.script_file_name();
    let manager = MemorySessionManager::new();
    let session = manager.start_session(DEPTH).await;

    let mut seen = Vec::new();
    for (idx, payload) in payloads.iter().enumerate() {
        let result = manager.ingest(&session, payload).await?;
        let IngestOutcome::Diff { leaks, .. } = &result.outcome else {
            return Err(format!(
                "diff {idx} must be a Diff outcome, got {:?}",
                result.outcome
            ));
        };
        let ours = leaks
            .iter()
            .find(|leak| leak.file.ends_with(&fname))
            .ok_or_else(|| {
                format!(
                    "diff {idx}: our growing site ({fname}) must be a suspected leak: {:?}",
                    leaks.iter().map(|l| &l.file).collect::<Vec<_>>()
                )
            })?;
        assert!(
            ours.size_growth > 0,
            "diff {idx}: the growing site must report positive growth, got {}",
            ours.size_growth
        );
        seen.push(confidence_rank(&ours.confidence.to_string()));
    }

    assert_eq!(
        seen,
        vec![1, 2, 3],
        "confidence for the repeated site must escalate LOW->MEDIUM->HIGH, got ranks {seen:?}"
    );
    Ok(())
}

// ── GC: real reference cycle is detected ─────────────────────────────────────

// Exercises [PROFILE-MEMORY-CODES]
/// Build and drop real `__del__` reference cycles, then run the gc-collect
/// script. With `DEBUG_SAVEALL` the collector retains them, so the ingested
/// result must report a real collection and surface uncollectable objects.
#[tokio::test]
async fn gc_collect_detects_real_reference_cycle() -> Result<(), String> {
    let workload = "\
import gc
gc.set_debug(gc.DEBUG_SAVEALL)
gc.disable()  # keep our dropped cycles alive until the explicit gc.collect() below: with auto-GC on, the generational collector reclaims them first on some runners (Linux CI) and the explicit collect then reports 0
class _Cycle:
    def __del__(self):
        pass
def _make():
    a = _Cycle(); b = _Cycle()
    a.peer = b; b.peer = a
for _ in range(40):
    _make()
";
    let src = program(&[workload, &scripts::gc_collect()]);
    let Some((_run, payloads)) = run_and_harvest(&src) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    assert_eq!(payloads.len(), 1, "gc collect couriers one payload");

    let manager = MemorySessionManager::new();
    let session = manager.start_session(DEPTH).await;
    let result = manager.ingest(&session, &payloads[0]).await?;
    let IngestOutcome::Gc(gc) = &result.outcome else {
        return Err(format!("expected gc outcome, got {:?}", result.outcome));
    };
    assert!(
        gc.collected > 0,
        "gc.collect() must report collecting our dropped cycles, got {}",
        gc.collected
    );
    assert!(
        gc.uncollectable_count > 0,
        "DEBUG_SAVEALL must retain the collected cycle objects, got {}",
        gc.uncollectable_count
    );
    assert!(
        gc.uncollectable_objects
            .iter()
            .any(|obj| obj.type_name == "_Cycle"),
        "our `_Cycle` objects must be reported among the retained objects"
    );
    Ok(())
}

// ── objects-by-type: real live-object census ─────────────────────────────────

/// Create 500 real dicts and census the heap. The objects-by-type payload must
/// pass straight through ingest and report a count that includes ours.
#[tokio::test]
async fn objects_by_type_counts_real_live_objects() -> Result<(), String> {
    let workload = "_BASILISK_DICTS = [dict(idx=i, payload=str(i)) for i in range(500)]";
    let src = program(&[workload, &scripts::objects_by_type("dict", 50)]);
    let Some((_run, payloads)) = run_and_harvest(&src) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    assert_eq!(payloads.len(), 1, "objects-by-type couriers one payload");

    let manager = MemorySessionManager::new();
    let session = manager.start_session(DEPTH).await;
    let result = manager.ingest(&session, &payloads[0]).await?;
    let IngestOutcome::Objects(value) = &result.outcome else {
        return Err(format!(
            "expected objects outcome, got {:?}",
            result.outcome
        ));
    };

    let total = value
        .get("totalCount")
        .and_then(serde_json::Value::as_u64)
        .ok_or("objects payload must carry a totalCount")?;
    assert!(
        total >= 500,
        "heap census must include our 500 dicts, got totalCount={total}"
    );
    let objects = value
        .get("objects")
        .and_then(serde_json::Value::as_array)
        .ok_or("objects payload must carry an objects array")?;
    assert!(
        !objects.is_empty() && objects.len() <= 50,
        "objects array must be populated and respect the limit, got {}",
        objects.len()
    );
    assert!(
        objects
            .iter()
            .all(|obj| obj.get("type").and_then(serde_json::Value::as_str) == Some("dict")),
        "every censused object must be of the requested type"
    );
    Ok(())
}

// ── reference graph: real retainers are walked ───────────────────────────────

/// Retain custom-typed objects behind a module global and walk the reference
/// graph. The ingested graph must contain real nodes, including our targets.
#[tokio::test]
async fn walk_references_builds_real_retention_graph() -> Result<(), String> {
    let workload = "\
class _Widget:
    pass
_BASILISK_WIDGETS = [_Widget() for _ in range(12)]
";
    let src = program(&[workload, &scripts::walk_references("_Widget", None, 4, 200)]);
    let Some((_run, payloads)) = run_and_harvest(&src) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    assert_eq!(payloads.len(), 1, "walk-references couriers one payload");

    let manager = MemorySessionManager::new();
    let session = manager.start_session(DEPTH).await;
    let result = manager.ingest(&session, &payloads[0]).await?;
    let IngestOutcome::Refs(graph) = &result.outcome else {
        return Err(format!("expected refs outcome, got {:?}", result.outcome));
    };

    let nodes = graph
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or("graph must carry a nodes array")?;
    assert!(
        !nodes.is_empty(),
        "the reference graph must contain real nodes"
    );
    assert!(
        nodes.iter().any(|node| {
            node.get("type").and_then(serde_json::Value::as_str) == Some("_Widget")
                && node.get("isTarget").and_then(serde_json::Value::as_bool) == Some(true)
        }),
        "our `_Widget` targets must appear as target nodes in the graph"
    );
    assert!(
        graph
            .get("edges")
            .and_then(serde_json::Value::as_array)
            .is_some(),
        "graph must carry an edges array"
    );
    Ok(())
}

// ── ack path: a real OK marker ingests as an acknowledgment ───────────────────

/// The stop script prints a bare `__BASILISK_MEM_OK__`; ingesting that real
/// output must yield an `Ack`, not an error.
#[tokio::test]
async fn real_ok_marker_ingests_as_ack() -> Result<(), String> {
    let Some(run) = run_python_program(&program(&[
        "import tracemalloc\ntracemalloc.start()",
        scripts::stop_tracemalloc(),
    ])) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    assert!(
        run.stdout.contains("__BASILISK_MEM_OK__"),
        "stop script must print the OK ack, got: {}",
        run.stdout
    );

    let manager = MemorySessionManager::new();
    let session = manager.start_session(DEPTH).await;
    let result = manager.ingest(&session, &run.stdout).await?;
    assert!(
        matches!(result.outcome, IngestOutcome::Ack),
        "a real OK marker must ingest as Ack, got {:?}",
        result.outcome
    );
    assert!(
        result.diagnostics.is_empty(),
        "an ack produces no diagnostics"
    );
    Ok(())
}
