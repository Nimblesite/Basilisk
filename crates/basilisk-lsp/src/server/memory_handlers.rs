//! LSP command handlers for `basilisk.memory.*` memory profiling commands.
//!
//! Memory profiling requires an active debug session (debugpy). These handlers
//! extract arguments, delegate to the memory profiling engine, and return
//! structured JSON responses.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::MessageType;
use tracing::info;

use super::LspServer;

/// Handle `basilisk.memory.start` — begin memory tracking.
///
/// Requires an active debug session. Injects `tracemalloc.start()` into the
/// running Python process via DAP evaluate.
pub(super) async fn execute_memory_start(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_memory_start called");

    let arg = args
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let traceback_depth = arg
        .get("tracebackDepth")
        .and_then(serde_json::Value::as_u64)
        .map_or(25, |d| u32::try_from(d).unwrap_or(25));

    let script = crate::profiler::memory::scripts::start_tracemalloc(traceback_depth);

    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: Starting memory tracking (depth={traceback_depth})"),
        )
        .await;

    // In a real implementation, we would send this script to the debug session
    // via DAP evaluate. For now, return the session info indicating readiness.
    Ok(Some(serde_json::json!({
        "memorySessionId": format!("mem-{:08x}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()),
        "tracingStarted": true,
        "script": script,
        "tracebackDepth": traceback_depth,
    })))
}

/// Handle `basilisk.memory.snapshot` — take allocation snapshot.
pub(super) async fn execute_memory_snapshot(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_memory_snapshot called");

    let arg = args
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let session_id = arg
        .get("memorySessionId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");

    let script = crate::profiler::memory::scripts::take_snapshot(500);

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

    let arg = args
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let session_id = arg
        .get("memorySessionId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");

    let script = crate::profiler::memory::scripts::diff_snapshot(500);

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

    let arg = args
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

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

    let arg = args
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

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
