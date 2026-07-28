//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Memory profiling and leak detection for the Basilisk LSP.
//!
//! Uses `tracemalloc` (Python stdlib) for per-line allocation tracking and
//! `gc` + introspection scripts for reference graph walking and cycle detection.
//! All analysis is performed by injecting Python code into the target process
//! via DAP `evaluate` requests — requires an active debug session.
//!
//! # Architecture
//!
//! - [`scripts`] — Python injection scripts for tracemalloc, gc, and reference walking
//! - [`diff`] — Snapshot comparison and growth detection
//! - [`leaks`] — Leak confidence scoring across multiple diffs

pub mod diagnostics;
pub mod diff;
pub mod heapprofile;
pub mod leaks;
pub mod scripts;
pub mod session;
pub mod timeline;

use serde::{Deserialize, Serialize};

/// Output marker for snapshot data.
pub const SNAPSHOT_MARKER: &str = "__BASILISK_MEM__";
/// Output marker for diff data.
pub const DIFF_MARKER: &str = "__BASILISK_MEM_DIFF__";
/// Output marker for reference graph data.
pub const REFS_MARKER: &str = "__BASILISK_MEM_REFS__";
/// Output marker for objects-by-type data.
pub const OBJECTS_MARKER: &str = "__BASILISK_MEM_OBJECTS__";
/// Output marker for gc collect data.
pub const GC_MARKER: &str = "__BASILISK_MEM_GC__";
/// Output marker for success acknowledgment.
pub const OK_MARKER: &str = "__BASILISK_MEM_OK__";

/// A memory snapshot parsed from the tracemalloc injection script output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    /// Snapshot identifier.
    pub snapshot_id: String,
    /// Current traced memory in bytes.
    pub current_memory: u64,
    /// Peak traced memory in bytes.
    pub peak_memory: u64,
    /// Number of gc-tracked objects.
    pub gc_objects: u64,
    /// gc generation counts `[gen0, gen1, gen2]`.
    pub gc_counts: Vec<u64>,
    /// Top allocation sites.
    pub top_allocations: Vec<AllocationSite>,
}

/// A single allocation site from a tracemalloc snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationSite {
    /// Source file path.
    pub file: String,
    /// Line number (1-based).
    pub line: i32,
    /// Total allocated bytes at this site.
    pub size: u64,
    /// Number of live objects allocated at this site.
    pub count: u64,
    /// Allocation call stack, root frame first (oldest call) → leaf last (the
    /// allocation site). Empty when no traceback was captured.
    pub traceback: Vec<diff::TraceFrame>,
}

/// Parse the `__BASILISK_MEM__` JSON output from a snapshot script.
///
/// # Errors
///
/// Returns an error if the output doesn't contain the marker or the JSON is invalid.
pub fn parse_snapshot_output(output: &str, snapshot_id: &str) -> Result<MemorySnapshot, String> {
    let json_str = extract_marker_json(output, SNAPSHOT_MARKER)?;
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|err| format!("invalid snapshot JSON: {err}"))?;

    let current = value
        .get("current")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let peak = value
        .get("peak")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let gc_objects = value
        .get("gcObjects")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let gc_counts = value
        .get("gcCounts")
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(serde_json::Value::as_u64).collect())
        .unwrap_or_default();

    let stats = value
        .get("stats")
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().map(parse_allocation_site).collect())
        .unwrap_or_default();

    Ok(MemorySnapshot {
        snapshot_id: snapshot_id.to_owned(),
        current_memory: current,
        peak_memory: peak,
        gc_objects,
        gc_counts,
        top_allocations: stats,
    })
}

/// Parse a single allocation site from snapshot JSON.
fn parse_allocation_site(value: &serde_json::Value) -> AllocationSite {
    let file = value
        .get("file")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("<unknown>")
        .to_owned();
    let line = value
        .get("line")
        .and_then(serde_json::Value::as_i64)
        .and_then(|v| i32::try_from(v).ok())
        .unwrap_or(0);
    let size = value
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let count = value
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    let traceback = diff::parse_traceback(value);

    AllocationSite {
        file,
        line,
        size,
        count,
        traceback,
    }
}

/// Parse the `__BASILISK_MEM_REFS__` reference-graph payload.
///
/// The reference-graph script's JSON (`{ nodes, edges, cycles }`) is passed
/// straight through to the editor's webview, so this validates the marker and
/// JSON shape and returns the parsed object verbatim rather than re-modelling
/// it in Rust (it has no server-side diagnostics).
///
/// # Errors
///
/// Returns an error if the marker is absent or the JSON is invalid.
pub fn parse_refs_output(output: &str) -> Result<serde_json::Value, String> {
    let json_str = extract_marker_json(output, REFS_MARKER)?;
    serde_json::from_str(json_str).map_err(|err| format!("invalid reference-graph JSON: {err}"))
}

/// Parse the `__BASILISK_MEM_OBJECTS__` objects-by-type payload.
///
/// Like [`parse_refs_output`], this is a validated pass-through to the editor.
///
/// # Errors
///
/// Returns an error if the marker is absent or the JSON is invalid.
pub fn parse_objects_output(output: &str) -> Result<serde_json::Value, String> {
    let json_str = extract_marker_json(output, OBJECTS_MARKER)?;
    serde_json::from_str(json_str).map_err(|err| format!("invalid objects-by-type JSON: {err}"))
}

/// Extract the JSON payload after a marker prefix from script output.
///
/// Scans each line for the marker and returns the JSON string after it.
pub(crate) fn extract_marker_json<'a>(output: &'a str, marker: &str) -> Result<&'a str, String> {
    for line in output.lines() {
        if let Some(json_start) = line.find(marker) {
            return Ok(&line[json_start + marker.len()..]);
        }
    }
    Err(format!("marker '{marker}' not found in output"))
}

/// Format bytes as a human-readable string (B, KB, MB, GB).
///
/// Uses integer division with one decimal place to avoid lossy float casts.
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    if bytes >= GB {
        let whole = bytes / GB;
        let frac = (bytes % GB) * 10 / GB;
        format!("{whole}.{frac} GB")
    } else if bytes >= MB {
        let whole = bytes / MB;
        let frac = (bytes % MB) * 10 / MB;
        format!("{whole}.{frac} MB")
    } else if bytes >= KB {
        let whole = bytes / KB;
        let frac = (bytes % KB) * 10 / KB;
        format!("{whole}.{frac} KB")
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only: a parse that defies its fixture must abort naming the expectation"
)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_snapshot() -> Result<(), String> {
        let output = r#"some debug output
__BASILISK_MEM__{"current": 45678912, "peak": 50000000, "gcObjects": 14523, "gcCounts": [712, 45, 3], "stats": [{"file": "src/app.py", "line": 42, "size": 24567890, "count": 15234, "traceback": [{"file": "src/app.py", "line": 42}]}]}
more output"#;

        let snapshot = parse_snapshot_output(output, "snap-001")?;
        assert_eq!(snapshot.snapshot_id, "snap-001");
        assert_eq!(snapshot.current_memory, 45_678_912);
        assert_eq!(snapshot.peak_memory, 50_000_000);
        assert_eq!(snapshot.gc_objects, 14523);
        assert_eq!(snapshot.gc_counts, vec![712, 45, 3]);
        assert_eq!(snapshot.top_allocations.len(), 1);
        let first = snapshot
            .top_allocations
            .first()
            .ok_or("expected at least one allocation")?;
        assert_eq!(first.file, "src/app.py");
        assert_eq!(first.size, 24_567_890);
        Ok(())
    }

    #[test]
    fn parse_missing_marker() {
        let output = "no marker here";
        let result = parse_snapshot_output(output, "snap-001");
        let _error = result.expect_err("output without the marker cannot yield a snapshot");
    }

    #[test]
    fn extract_marker_json_works() -> Result<(), String> {
        let output = "prefix__BASILISK_MEM__{\"key\": 42}suffix";
        let json = extract_marker_json(output, "__BASILISK_MEM__")?;
        assert_eq!(json, "{\"key\": 42}suffix");
        Ok(())
    }

    #[test]
    fn format_bytes_units() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
        assert_eq!(format_bytes(248_000_000), "236.5 MB");
    }
}
