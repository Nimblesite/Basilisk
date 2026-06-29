//! Tests for [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
//
// End-to-end memory-profiling round-trip over the real LSP. The editor is the
// DAP courier: it asks the LSP for an injection script, runs it in the debuggee,
// and posts the raw output back via `basilisk.memory.ingest`. These tests drive
// that round-trip with the exact marker-prefixed output the Python injection
// scripts emit, exercising the handlers, the `MemorySessionManager`, the
// marker dispatch, and the diagnostics-publish path.

use super::ws_test_common::*;

/// Marker-prefixed `tracemalloc` snapshot output with a 24 MB allocation site.
const SNAPSHOT_OUTPUT: &str = r#"__BASILISK_MEM__{"current": 45678912, "peak": 50000000, "gcObjects": 14523, "gcCounts": [712, 45, 3], "stats": [{"file": "/tmp/app.py", "line": 42, "size": 24567890, "count": 15234, "traceback": [{"file": "/tmp/app.py", "line": 42}]}]}"#;

/// Marker-prefixed diff output with one growing site at /tmp/cache.py:34.
const DIFF_OUTPUT: &str = r#"__BASILISK_MEM_DIFF__{"leaks": [{"file": "/tmp/cache.py", "line": 34, "sizeDiff": 5000, "countDiff": 100, "size": 1000000, "count": 500, "traceback": [{"file": "/tmp/cache.py", "line": 34}]}], "current": 60000000, "peak": 70000000}"#;

/// Marker-prefixed gc-collect output with one uncollectable cycle.
const GC_OUTPUT: &str = r#"__BASILISK_MEM_GC__{"collected": 42, "uncollectable": 1, "memoryFreed": 8192, "uncollectableObjects": [{"id": 1, "type": "CacheNode", "size": 4096, "repr": "<CacheNode>", "reason": "Instance has __del__ method and is in a reference cycle"}]}"#;

/// Marker-prefixed reference-graph output.
const REFS_OUTPUT: &str = r#"__BASILISK_MEM_REFS__{"nodes": [{"id": 1, "type": "dict", "size": 100, "repr": "{}", "depth": 0, "isTarget": true}], "edges": [], "cycles": []}"#;

/// Run a `workspace/executeCommand` and return its `result` object.
async fn exec(
    fixture: &mut WsTestFixture,
    id: u64,
    command: &str,
    args: serde_json::Value,
) -> TestResult<serde_json::Value> {
    let resp = fixture
        .request(
            id,
            "workspace/executeCommand",
            serde_json::json!({ "command": command, "arguments": [args] }),
        )
        .await?
        .ok_or_else(|| format!("no response to {command}"))?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "{command} returned an error: {resp}"
    );
    parsed
        .get("result")
        .cloned()
        .ok_or_else(|| format!("{command} response had no result: {resp}").into())
}

/// Field accessor: read a string field from a JSON object.
fn str_field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

/// Start a memory session over the wire and return its id + start script.
async fn start_memory_session(
    fixture: &mut WsTestFixture,
    id: u64,
) -> TestResult<(String, String)> {
    let start = exec(
        fixture,
        id,
        "basilisk.memory.start",
        serde_json::json!({ "tracebackDepth": 25 }),
    )
    .await?;
    let session_id = str_field(&start, "memorySessionId")
        .ok_or("start should return memorySessionId")?
        .to_owned();
    let script = str_field(&start, "script").unwrap_or_default().to_owned();
    Ok((session_id, script))
}

// Exercises [PROFILE-MEMORY-COMMANDS]
/// `start` mints a `mem-` session and hands out a tracemalloc script; ingesting a
/// snapshot returns the parsed allocation summary.
#[tokio::test]
async fn test_ws_memory_start_then_snapshot() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let (session_id, start_script) = start_memory_session(&mut fixture, 800).await?;
    assert!(session_id.starts_with("mem-"), "session id: {session_id}");
    assert!(
        start_script.contains("tracemalloc.start(25)"),
        "start script: {start_script}"
    );

    let snap_cmd = exec(
        &mut fixture,
        801,
        "basilisk.memory.snapshot",
        serde_json::json!({ "memorySessionId": session_id }),
    )
    .await?;
    assert!(
        str_field(&snap_cmd, "script")
            .unwrap_or_default()
            .contains("__BASILISK_MEM__"),
        "snapshot script should print the marker"
    );

    let snap = exec(
        &mut fixture,
        802,
        "basilisk.memory.ingest",
        serde_json::json!({ "memorySessionId": session_id, "output": SNAPSHOT_OUTPUT }),
    )
    .await?;
    assert_eq!(str_field(&snap, "kind"), Some("snapshot"));
    assert_eq!(
        snap.get("currentMemory")
            .and_then(serde_json::Value::as_u64),
        Some(45_678_912)
    );
    let allocs = snap
        .get("topAllocations")
        .and_then(serde_json::Value::as_array)
        .ok_or("snapshot should carry topAllocations")?;
    assert_eq!(allocs.len(), 1, "one allocation site expected");

    // The snapshot is also exported as a V8 `.heapprofile` the editor opens in
    // VS Code's native profile viewer — verify the file exists and is valid.
    let heap_path =
        str_field(&snap, "heapProfilePath").ok_or("snapshot should return heapProfilePath")?;
    assert!(
        heap_path.ends_with(".heapprofile"),
        "heapProfilePath: {heap_path}"
    );
    let contents =
        std::fs::read_to_string(heap_path).map_err(|err| format!("read heapprofile: {err}"))?;
    let profile: serde_json::Value = serde_json::from_str(&contents)?;
    let self_size = profile
        .get("head")
        .and_then(|head| head.get("children"))
        .and_then(serde_json::Value::as_array)
        .and_then(|children| children.first())
        .and_then(|node| node.get("selfSize"))
        .and_then(serde_json::Value::as_u64);
    assert_eq!(
        self_size,
        Some(24_567_890),
        "heapprofile self size matches the allocation"
    );
    Ok(())
}

/// Repeated diffs of the same growing site escalate leak confidence across
/// snapshots within one session: LOW (1) → MEDIUM (2) → HIGH (3+).
#[tokio::test]
async fn test_ws_memory_diff_escalates_leak_confidence() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let (session_id, _) = start_memory_session(&mut fixture, 810).await?;

    for (idx, want) in ["LOW", "MEDIUM", "HIGH"].iter().enumerate() {
        let id = 820 + idx as u64;
        let diff = exec(
            &mut fixture,
            id,
            "basilisk.memory.ingest",
            serde_json::json!({ "memorySessionId": session_id, "output": DIFF_OUTPUT }),
        )
        .await?;
        assert_eq!(str_field(&diff, "kind"), Some("diff"));
        let leaks = diff
            .get("suspectedLeaks")
            .and_then(serde_json::Value::as_array)
            .ok_or("diff should carry suspectedLeaks")?;
        let first = leaks.first().ok_or("expected one suspected leak")?;
        assert_eq!(str_field(first, "confidence"), Some(*want), "diff #{idx}");
    }
    Ok(())
}

/// gc-collect and reference-graph outputs marker-dispatch to their kinds.
#[tokio::test]
async fn test_ws_memory_gc_and_references() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let (session_id, _) = start_memory_session(&mut fixture, 840).await?;

    let gc = exec(
        &mut fixture,
        841,
        "basilisk.memory.ingest",
        serde_json::json!({ "memorySessionId": session_id, "output": GC_OUTPUT }),
    )
    .await?;
    assert_eq!(str_field(&gc, "kind"), Some("gc"));
    assert_eq!(
        gc.get("uncollectable").and_then(serde_json::Value::as_u64),
        Some(1)
    );

    let refs = exec(
        &mut fixture,
        842,
        "basilisk.memory.ingest",
        serde_json::json!({ "memorySessionId": session_id, "output": REFS_OUTPUT }),
    )
    .await?;
    assert_eq!(str_field(&refs, "kind"), Some("refs"));
    let graph = refs.get("graph").ok_or("refs should carry a graph")?;
    assert!(graph
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|nodes| nodes.len() == 1));
    Ok(())
}

/// Ingesting against an unknown session id is rejected, not silently accepted.
#[tokio::test]
async fn test_ws_memory_ingest_unknown_session_errors() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let resp = fixture
        .request(
            850,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.memory.ingest",
                "arguments": [{ "memorySessionId": "mem-nope", "output": SNAPSHOT_OUTPUT }]
            }),
        )
        .await?
        .ok_or("no response to ingest")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_some(),
        "unknown session must error: {resp}"
    );
    Ok(())
}
