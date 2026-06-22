//! Tests for [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
//!
//! REAL full-stack memory profiling through the LSP wire — no mocks.
//!
//! `profiler_memory_e2e.rs` drives the ingest engine directly; this suite goes
//! through the ACTUAL `workspace/executeCommand` surface of a live server. It
//! asks the server for the injection scripts (`basilisk.memory.start`,
//! `.snapshot`, `.gcCollect`), runs them against a real allocating Python
//! program, then posts the genuine `tracemalloc`/`gc` output back through
//! `basilisk.memory.ingest` — asserting the server's camelCase result shaping,
//! the `.heapprofile` it writes for VS Code's viewer, and the at-exit
//! final-snapshot file the breakpoint-free flow depends on
//! ([PROFILE-MEMORY-FINAL]). Cross-platform; SKIPs when `python3` is absent.

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
#[path = "lsp/ws_test_common.rs"]
mod ws_test_common;

use common::{harvest_mem_payloads, run_python_program};
use ws_test_common::*;

/// Retain ~11 MB of distinct bytes at one attributable line.
const RETAIN_11MB: &str = "_BASILISK_KEEP = [bytes(256) for _ in range(40000)]";

/// Begin a memory session over the wire and return `(memorySessionId, startScript,
/// finalSnapshotFile)`.
async fn start_memory_session(
    fixture: &mut WsTestFixture,
    id: u64,
) -> TestResult<(String, String, String)> {
    let resp = execute_command(
        fixture,
        id,
        "basilisk.memory.start",
        serde_json::json!({ "tracebackDepth": 25 }),
    )
    .await?;
    let result = command_result(&resp, "memory.start")?;
    let session = result["memorySessionId"]
        .as_str()
        .ok_or("memory.start must return a memorySessionId")?
        .to_owned();
    assert!(
        session.starts_with("mem-"),
        "memory session id must be prefixed: {session}"
    );
    let script = result["script"]
        .as_str()
        .ok_or("memory.start must return the tracemalloc script")?
        .to_owned();
    let final_file = result["finalSnapshotFile"]
        .as_str()
        .ok_or("memory.start must return a finalSnapshotFile path")?
        .to_owned();
    Ok((session, script, final_file))
}

// ── Full snapshot round-trip with .heapprofile export ────────────────────────

/// start → snapshot script → real allocation → ingest. The server must return a
/// camelCase snapshot showing real memory and write a valid `.heapprofile`.
#[tokio::test]
async fn memory_full_lsp_roundtrip_snapshot() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let (session, start_script, final_file) = start_memory_session(&mut fixture, 300).await?;

    // Ask the server for the snapshot script (leg 1 of the snapshot round-trip).
    let snap_resp = execute_command(
        &mut fixture,
        301,
        "basilisk.memory.snapshot",
        serde_json::json!({ "memorySessionId": session }),
    )
    .await?;
    let snapshot_script = command_result(&snap_resp, "memory.snapshot")?["script"]
        .as_str()
        .ok_or("memory.snapshot must return a script")?
        .to_owned();

    // Run start + allocate + snapshot in a real interpreter; harvest the payload.
    let program = format!("{start_script}\n{RETAIN_11MB}\n{snapshot_script}");
    let Some(run) = run_python_program(&program) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    let _ = std::fs::remove_file(&final_file);
    assert!(
        run.success,
        "the injected program must run cleanly.\nstderr:\n{}",
        run.stderr
    );
    let payloads = harvest_mem_payloads(&run.stdout);
    assert_eq!(
        payloads.len(),
        1,
        "one snapshot payload expected: {payloads:?}"
    );

    // Leg 2: post the genuine output back through ingest.
    let ingest_resp = execute_command(
        &mut fixture,
        302,
        "basilisk.memory.ingest",
        serde_json::json!({ "memorySessionId": session, "output": payloads[0] }),
    )
    .await?;
    let ingest = command_result(&ingest_resp, "memory.ingest")?;

    assert_eq!(
        ingest["kind"].as_str(),
        Some("snapshot"),
        "ingest must classify the marker as a snapshot: {ingest}"
    );
    assert!(
        ingest["currentMemory"]
            .as_u64()
            .is_some_and(|bytes| bytes >= 4_000_000),
        "ingest must report the real traced memory: {ingest}"
    );
    let allocations = ingest["topAllocations"]
        .as_array()
        .ok_or("ingest snapshot must carry topAllocations")?;
    assert!(!allocations.is_empty(), "real allocations must be reported");
    let fname = run.script_file_name();
    assert!(
        allocations.iter().any(|site| site["file"]
            .as_str()
            .is_some_and(|file| file.ends_with(&fname))),
        "the program's own allocation site must be attributed: {allocations:?}"
    );

    // The server must export a VS Code-openable .heapprofile.
    let heap_path = ingest["heapProfilePath"]
        .as_str()
        .ok_or("ingest must export a heapProfilePath")?;
    assert!(
        std::path::Path::new(heap_path).exists(),
        "heapprofile must exist on disk: {heap_path}"
    );
    let heap_json = std::fs::read_to_string(heap_path)?;
    let parsed: serde_json::Value = serde_json::from_str(&heap_json)?;
    assert!(
        parsed.get("nodes").is_some() || parsed.get("head").is_some(),
        "heapprofile must be a structured V8 heap profile: {parsed:.120}"
    );
    let _ = std::fs::remove_file(heap_path);
    Ok(())
}

// ── At-exit final snapshot, ingested over the wire ───────────────────────────

/// [PROFILE-MEMORY-FINAL] The no-breakpoint flow: start, let the program run to
/// completion, then ingest the at-exit final-snapshot file the start script
/// wrote — all command shaping verified through the real server.
#[tokio::test]
async fn memory_final_snapshot_flow_via_wire() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let (session, start_script, final_file) = start_memory_session(&mut fixture, 310).await?;

    // Run start + allocate, then exit: the atexit hook writes the final file.
    let program = format!("{start_script}\n{RETAIN_11MB}");
    let Some(run) = run_python_program(&program) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    assert!(
        run.success,
        "run-to-completion program must exit cleanly.\nstderr:\n{}",
        run.stderr
    );

    let payload = std::fs::read_to_string(&final_file).map_err(|err| {
        format!("the atexit hook must write the final-snapshot file {final_file}: {err}")
    })?;
    let _ = std::fs::remove_file(&final_file);

    let ingest_resp = execute_command(
        &mut fixture,
        311,
        "basilisk.memory.ingest",
        serde_json::json!({ "memorySessionId": session, "output": payload }),
    )
    .await?;
    let ingest = command_result(&ingest_resp, "memory.ingest (final)")?;
    assert_eq!(
        ingest["kind"].as_str(),
        Some("snapshot"),
        "the final-snapshot file must ingest as a snapshot: {ingest}"
    );
    assert!(
        ingest["currentMemory"]
            .as_u64()
            .is_some_and(|bytes| bytes >= 4_000_000),
        "the final snapshot must show the retained memory: {ingest}"
    );
    Ok(())
}

// ── GC collect over the wire detects a real cycle ────────────────────────────

/// start (sets `DEBUG_SAVEALL`) → gcCollect script → real dropped `__del__`
/// cycle → ingest. The server must return a `gc` outcome reporting the collection.
#[tokio::test]
async fn memory_gc_collect_via_wire_detects_cycle() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let (session, start_script, final_file) = start_memory_session(&mut fixture, 320).await?;

    let gc_resp = execute_command(
        &mut fixture,
        321,
        "basilisk.memory.gcCollect",
        serde_json::json!({}),
    )
    .await?;
    let gc_script = command_result(&gc_resp, "memory.gcCollect")?["script"]
        .as_str()
        .ok_or("memory.gcCollect must return a script")?
        .to_owned();

    let cycle = "\
class _Cycle:
    def __del__(self):
        pass
def _make():
    a = _Cycle(); b = _Cycle()
    a.peer = b; b.peer = a
for _ in range(40):
    _make()
";
    let program = format!("{start_script}\n{cycle}\n{gc_script}");
    let Some(run) = run_python_program(&program) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };
    let _ = std::fs::remove_file(&final_file);
    assert!(
        run.success,
        "gc program must run cleanly.\nstderr:\n{}",
        run.stderr
    );

    let payloads = harvest_mem_payloads(&run.stdout);
    assert_eq!(
        payloads.len(),
        1,
        "gc collect couriers one payload: {payloads:?}"
    );

    let ingest_resp = execute_command(
        &mut fixture,
        322,
        "basilisk.memory.ingest",
        serde_json::json!({ "memorySessionId": session, "output": payloads[0] }),
    )
    .await?;
    let ingest = command_result(&ingest_resp, "memory.ingest (gc)")?;
    assert_eq!(
        ingest["kind"].as_str(),
        Some("gc"),
        "ingest must classify the gc marker: {ingest}"
    );
    assert!(
        ingest["collected"].as_u64().is_some_and(|n| n > 0),
        "gc.collect() must report collecting our dropped cycles: {ingest}"
    );
    assert!(
        ingest["uncollectable"].as_u64().is_some_and(|n| n > 0),
        "DEBUG_SAVEALL must retain the collected cycle objects: {ingest}"
    );
    Ok(())
}

// ── Wire guardrail ───────────────────────────────────────────────────────────

/// Ingesting against an unknown session over the wire is a clean `-32010`
/// error, not a panic — even when the payload itself is valid.
#[tokio::test]
async fn memory_ingest_unknown_session_via_wire_errors() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let resp = execute_command(
        &mut fixture,
        330,
        "basilisk.memory.ingest",
        serde_json::json!({
            "memorySessionId": "mem-does-not-exist",
            "output": "__BASILISK_MEM_OK__",
        }),
    )
    .await?;
    let error = resp
        .get("error")
        .filter(|err| !err.is_null())
        .ok_or("an unknown session must be rejected with an error")?;
    assert_eq!(
        error["code"].as_i64(),
        Some(-32010),
        "memory errors must use the -32010 domain code: {error}"
    );
    Ok(())
}
