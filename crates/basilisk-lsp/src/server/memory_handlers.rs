//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! LSP command handlers for `basilisk.memory.*` memory profiling commands.
//!
//! Memory profiling requires an active debug session (debugpy). These handlers
//! extract arguments, delegate to the memory profiling engine, and return
//! structured JSON responses.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::MessageType;
use tracing::{error, info};

use super::LspServer;
use crate::profiler::memory::diagnostics::DiagnosticsByUri;
use crate::profiler::memory::session::IngestOutcome;

/// Max allocation sites a snapshot/diff script emits. Kept modest so the printed
/// `__BASILISK_MEM__` line (which surfaces in the Debug Console) stays readable —
/// the dashboard only needs the top sites.
const MAX_SNAPSHOT_STATS: usize = 100;

/// Construct a memory-domain LSP error (`-32010`).
fn memory_error(message: impl Into<String>) -> tower_lsp::jsonrpc::Error {
    tower_lsp::jsonrpc::Error {
        code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32010),
        message: message.into().into(),
        data: None,
    }
}

/// Return the first command argument, or an empty JSON object if absent.
///
/// LSP `executeCommand` always delivers `args` as an array; for these handlers,
/// callers can either omit it entirely or pass a single object. Treating "absent"
/// as "empty object" lets the rest of the handler use the same `.get(...)` path
/// for both cases.
fn first_arg_or_empty(args: &[serde_json::Value]) -> serde_json::Value {
    args.first()
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()))
}

/// Handle `basilisk.memory.start` — begin memory tracking.
///
/// Requires an active debug session. Injects `tracemalloc.start()` into the
/// running Python process via DAP evaluate.
pub(super) async fn execute_memory_start(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_memory_start called");

    let arg = first_arg_or_empty(args);

    let traceback_depth = arg
        .get("tracebackDepth")
        .and_then(serde_json::Value::as_u64)
        .map_or(25, |d| u32::try_from(d).unwrap_or(25));

    // Register a real session so subsequent snapshot/diff ingests can accumulate
    // cross-call leak history. The editor runs `script` in the debuggee and posts
    // the output back via `basilisk.memory.ingest` (the LSP holds no DAP wire).
    let memory_session_id = server.memory_manager.start_session(traceback_depth).await;

    // [PROFILE-MEMORY-FINAL] Mint the at-exit snapshot file the injected script's
    // `atexit` hook writes to, and return its path so the editor can read it when
    // the debug session terminates — this is what gives the breakpoint-free
    // "Run & Track Memory (Current File)" flow a visible result instead of a
    // dead end. The path is unique per session, just like the cooperative
    // sampler's sample file ([PROFILE-COOPERATIVE]).
    let final_snapshot_file = std::env::temp_dir()
        .join(format!("basilisk-{memory_session_id}.memfinal"))
        .to_string_lossy()
        .into_owned();
    let script = crate::profiler::memory::scripts::start_tracemalloc(
        traceback_depth,
        &final_snapshot_file,
        MAX_SNAPSHOT_STATS,
    );

    server
        .client
        .log_message(
            MessageType::INFO,
            format!(
                "Basilisk: memory tracking started ({memory_session_id}, depth={traceback_depth})"
            ),
        )
        .await;

    Ok(Some(serde_json::json!({
        "memorySessionId": memory_session_id,
        "tracingStarted": true,
        "script": script,
        "tracebackDepth": traceback_depth,
        "finalSnapshotFile": final_snapshot_file,
    })))
}

/// Handle `basilisk.memory.snapshot` — take allocation snapshot.
pub(super) async fn execute_memory_snapshot(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_memory_snapshot called");

    let arg = first_arg_or_empty(args);

    let session_id = arg
        .get("memorySessionId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");

    let script = crate::profiler::memory::scripts::take_snapshot(MAX_SNAPSHOT_STATS);

    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: Taking memory snapshot for session {session_id}"),
        )
        .await;

    Ok(Some(serde_json::json!({
        "memorySessionId": session_id,
        "script": script,
    })))
}

/// Handle `basilisk.memory.diff` — compare two snapshots.
pub(super) async fn execute_memory_diff(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_memory_diff called");

    let arg = first_arg_or_empty(args);

    let session_id = arg
        .get("memorySessionId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");

    let script = crate::profiler::memory::scripts::diff_snapshot(MAX_SNAPSHOT_STATS);

    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: Diffing memory snapshots for session {session_id}"),
        )
        .await;

    Ok(Some(serde_json::json!({
        "memorySessionId": session_id,
        "script": script,
    })))
}

/// Handle `basilisk.memory.references` — walk reference graph.
pub(super) async fn execute_memory_references(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_memory_references called");

    let arg = first_arg_or_empty(args);

    let target_type = arg
        .get("targetType")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("object");

    let max_depth = arg
        .get("maxDepth")
        .and_then(serde_json::Value::as_u64)
        .map_or(5, |d| u32::try_from(d).unwrap_or(5));

    let max_nodes = arg
        .get("maxNodes")
        .and_then(serde_json::Value::as_u64)
        .map_or(200, |n| u32::try_from(n).unwrap_or(200));

    let repr_contains = arg
        .get("targetReprContains")
        .and_then(serde_json::Value::as_str);

    let script = crate::profiler::memory::scripts::walk_references(
        target_type,
        repr_contains,
        max_depth,
        max_nodes,
    );

    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: Walking references for type '{target_type}'"),
        )
        .await;

    Ok(Some(serde_json::json!({
        "targetType": target_type,
        "maxDepth": max_depth,
        "maxNodes": max_nodes,
        "script": script,
    })))
}

/// Handle `basilisk.memory.objectsByType` — list objects of a given type.
pub(super) async fn execute_memory_objects_by_type(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_memory_objects_by_type called");

    let arg = first_arg_or_empty(args);

    let type_name = arg
        .get("typeName")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("object");

    let limit = arg
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(50, |l| u32::try_from(l).unwrap_or(50));

    let script = crate::profiler::memory::scripts::objects_by_type(type_name, limit);

    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: Listing objects of type '{type_name}' (limit={limit})"),
        )
        .await;

    Ok(Some(serde_json::json!({
        "typeName": type_name,
        "limit": limit,
        "script": script,
    })))
}

/// Handle `basilisk.memory.gcCollect` — force garbage collection.
pub(super) async fn execute_memory_gc_collect(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_memory_gc_collect called");

    let script = crate::profiler::memory::scripts::gc_collect();

    server
        .client
        .log_message(
            MessageType::INFO,
            "Basilisk: Forcing garbage collection".to_owned(),
        )
        .await;

    Ok(Some(serde_json::json!({
        "script": script,
    })))
}

/// Handle `basilisk.memory.ingest` — second leg of the round-trip.
///
/// The editor ran an injection script in the debuggee via DAP `evaluate` and
/// posts the raw stdout here. The marker in the output selects the parser; the
/// [`MemorySessionManager`](crate::profiler::memory::session::MemorySessionManager)
/// updates session state and produces diagnostics, which we publish before
/// returning the structured result to the editor.
pub(super) async fn execute_memory_ingest(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_memory_ingest called");

    let arg = first_arg_or_empty(args);

    let Some(session_id) = arg
        .get("memorySessionId")
        .and_then(serde_json::Value::as_str)
    else {
        return Err(memory_error("Missing required parameter: memorySessionId"));
    };
    let output = arg
        .get("output")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    match server.memory_manager.ingest(session_id, output).await {
        Ok(result) => {
            publish_memory_diagnostics(server, &result.diagnostics).await;
            let mut json = ingest_outcome_to_json(session_id, &result.outcome);
            // A snapshot is also exported as a V8 `.heapprofile` so the editor can
            // open it in VS Code's built-in profile viewer (flame chart + table).
            if let IngestOutcome::Snapshot(snapshot) = &result.outcome {
                if let Some(path) = write_heapprofile(snapshot) {
                    if let Some(obj) = json.as_object_mut() {
                        let _ = obj.insert(
                            "heapProfilePath".to_owned(),
                            serde_json::Value::String(path),
                        );
                    }
                }
            }
            Ok(Some(json))
        }
        Err(err) => {
            error!(%err, "memory ingest failed");
            server
                .client
                .log_message(
                    MessageType::ERROR,
                    format!("Basilisk: memory ingest failed: {err}"),
                )
                .await;
            Err(memory_error(err))
        }
    }
}

/// Write a snapshot as a V8 `.heapprofile` to the temp dir and return its path.
///
/// Returns `None` (logging the cause) if serialization or the write fails — the
/// snapshot result is still returned to the editor, just without a file to open.
fn write_heapprofile(snapshot: &crate::profiler::memory::MemorySnapshot) -> Option<String> {
    let profile = crate::profiler::memory::heapprofile::snapshot_to_heapprofile(snapshot);
    let json = serde_json::to_string(&profile)
        .map_err(|err| error!(%err, "failed to serialize heapprofile"))
        .ok()?;
    let path = std::env::temp_dir().join(format!("basilisk-{}.heapprofile", snapshot.snapshot_id));
    std::fs::write(&path, json)
        .map_err(|err| error!(%err, "failed to write heapprofile"))
        .ok()?;
    info!(path = %path.display(), "wrote heapprofile");
    Some(path.display().to_string())
}

/// Publish memory diagnostics for every affected URI.
async fn publish_memory_diagnostics(server: &LspServer, diagnostics: &DiagnosticsByUri) {
    for (uri, items) in diagnostics {
        server
            .client
            .publish_diagnostics(uri.clone(), items.clone(), None)
            .await;
    }
}

/// Serialize an ingest outcome into the editor wire format (camelCase, tagged
/// by `kind` so the editor can dispatch).
fn ingest_outcome_to_json(session_id: &str, outcome: &IngestOutcome) -> serde_json::Value {
    match outcome {
        IngestOutcome::Snapshot(snapshot) => serde_json::json!({
            "kind": "snapshot",
            "memorySessionId": session_id,
            "snapshotId": snapshot.snapshot_id,
            "currentMemory": snapshot.current_memory,
            "peakMemory": snapshot.peak_memory,
            "gcObjects": snapshot.gc_objects,
            "gcCounts": snapshot.gc_counts,
            "topAllocations": allocation_sites_json(snapshot),
        }),
        IngestOutcome::Diff { diff, leaks } => serde_json::json!({
            "kind": "diff",
            "memorySessionId": session_id,
            "totalGrowth": diff.total_growth,
            "totalFreed": diff.total_freed,
            "netGrowth": diff.net_growth,
            "suspectedLeaks": suspected_leaks_json(leaks),
        }),
        IngestOutcome::Gc(gc) => serde_json::json!({
            "kind": "gc",
            "memorySessionId": session_id,
            "collected": gc.collected,
            "uncollectable": gc.uncollectable_count,
            "memoryFreed": gc.memory_freed,
            "uncollectableObjects": uncollectable_objects_json(gc),
        }),
        IngestOutcome::Refs(graph) => serde_json::json!({
            "kind": "refs",
            "memorySessionId": session_id,
            "graph": graph,
        }),
        IngestOutcome::Objects(objects) => serde_json::json!({
            "kind": "objects",
            "memorySessionId": session_id,
            "objects": objects,
        }),
        IngestOutcome::Ack => serde_json::json!({
            "kind": "ack",
            "memorySessionId": session_id,
        }),
    }
}

/// Build the `topAllocations` array for a snapshot response.
fn allocation_sites_json(
    snapshot: &crate::profiler::memory::MemorySnapshot,
) -> Vec<serde_json::Value> {
    snapshot
        .top_allocations
        .iter()
        .map(|alloc| {
            serde_json::json!({
                "file": alloc.file,
                "line": alloc.line,
                "size": alloc.size,
                "count": alloc.count,
            })
        })
        .collect()
}

/// Build the `suspectedLeaks` array for a diff response.
fn suspected_leaks_json(
    leaks: &[crate::profiler::memory::leaks::SuspectedLeak],
) -> Vec<serde_json::Value> {
    leaks
        .iter()
        .map(|leak| {
            serde_json::json!({
                "file": leak.file,
                "line": leak.line,
                "sizeGrowth": leak.size_growth,
                "countGrowth": leak.count_growth,
                "currentSize": leak.current_size,
                "currentCount": leak.current_count,
                "confidence": leak.confidence.to_string(),
                "reason": leak.reason,
            })
        })
        .collect()
}

/// Build the `uncollectableObjects` array for a gc response.
fn uncollectable_objects_json(
    gc: &crate::profiler::memory::diagnostics::GcCollectResult,
) -> Vec<serde_json::Value> {
    gc.uncollectable_objects
        .iter()
        .map(|obj| {
            serde_json::json!({
                "typeName": obj.type_name,
                "size": obj.size,
                "repr": obj.repr,
                "reason": obj.reason,
            })
        })
        .collect()
}
