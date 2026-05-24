//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Memory snapshot diffing — compare two snapshots to detect growth and leaks.
//!
//! Parses the JSON output from the `diff_snapshot` injection script and
//! produces structured diff results for diagnostics and UI.

use serde::{Deserialize, Serialize};

/// A single allocation site that grew between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationGrowth {
    /// Source file path.
    pub file: String,
    /// Line number (1-based).
    pub line: i32,
    /// Size growth in bytes.
    pub size_diff: i64,
    /// Object count growth.
    pub count_diff: i64,
    /// Current total size in bytes.
    pub size: u64,
    /// Current total object count.
    pub count: u64,
    /// Allocation traceback (deepest frame first).
    pub traceback: Vec<TraceFrame>,
}

/// A single frame in an allocation traceback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceFrame {
    /// Source file path.
    pub file: String,
    /// Line number (1-based).
    pub line: i32,
}

/// Result of comparing two memory snapshots.
#[derive(Debug, Clone, Serialize)]
pub struct MemoryDiff {
    /// Total bytes that grew.
    pub total_growth: i64,
    /// Total bytes freed.
    pub total_freed: i64,
    /// Net growth (positive = memory increased).
    pub net_growth: i64,
    /// Allocation sites that grew, sorted by `size_diff` descending.
    pub grown_allocations: Vec<AllocationGrowth>,
    /// Allocation sites that shrank.
    pub freed_allocations: Vec<AllocationGrowth>,
}

/// Parse the JSON output from the `diff_snapshot` script.
///
/// # Errors
///
/// Returns an error if the JSON is malformed or missing expected fields.
pub fn parse_diff_output(json_str: &str) -> Result<MemoryDiff, String> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|err| format!("invalid diff JSON: {err}"))?;

    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return Err(error.to_owned());
    }

    let leaks = value
        .get("leaks")
        .and_then(|v| v.as_array())
        .ok_or("missing 'leaks' array")?;

    let mut grown = Vec::new();
    let mut freed = Vec::new();
    let mut total_growth: i64 = 0;
    let mut total_freed: i64 = 0;

    for entry in leaks {
        let alloc = parse_allocation_entry(entry);
        if alloc.size_diff > 0 {
            total_growth += alloc.size_diff;
            grown.push(alloc);
        } else {
            total_freed += alloc.size_diff.abs();
            freed.push(alloc);
        }
    }

    grown.sort_by_key(|a| std::cmp::Reverse(a.size_diff));
    freed.sort_by_key(|a| a.size_diff);

    Ok(MemoryDiff {
        total_growth,
        total_freed,
        net_growth: total_growth - total_freed,
        grown_allocations: grown,
        freed_allocations: freed,
    })
}

/// Parse a single allocation entry from the diff JSON.
fn parse_allocation_entry(value: &serde_json::Value) -> AllocationGrowth {
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
    let size_diff = value
        .get("sizeDiff")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let count_diff = value
        .get("countDiff")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let size = value
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let count = value
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    let traceback = value
        .get("traceback")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|frame| TraceFrame {
                    file: frame
                        .get("file")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    line: frame
                        .get("line")
                        .and_then(serde_json::Value::as_i64)
                        .and_then(|v| i32::try_from(v).ok())
                        .unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();

    AllocationGrowth {
        file,
        line,
        size_diff,
        count_diff,
        size,
        count,
        traceback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_diff() -> Result<(), String> {
        let json = r#"{
            "leaks": [
                {
                    "file": "src/cache.py",
                    "line": 34,
                    "sizeDiff": 18234567,
                    "countDiff": 8923,
                    "size": 24567890,
                    "count": 12345,
                    "traceback": [
                        {"file": "src/cache.py", "line": 34},
                        {"file": "src/app.py", "line": 78}
                    ]
                }
            ],
            "current": 89012345,
            "peak": 102345678
        }"#;

        let diff = parse_diff_output(json)?;
        assert_eq!(diff.grown_allocations.len(), 1);
        let first = diff
            .grown_allocations
            .first()
            .ok_or("expected at least one grown allocation")?;
        assert_eq!(first.file, "src/cache.py");
        assert_eq!(first.size_diff, 18_234_567);
        assert_eq!(first.traceback.len(), 2);
        assert_eq!(diff.total_growth, 18_234_567);
        Ok(())
    }

    #[test]
    fn parse_error_response() {
        let json = r#"{"error": "no previous snapshot"}"#;
        let result = parse_diff_output(json);
        assert!(result.is_err());
        let err_msg = result.err().unwrap_or_default();
        assert!(
            err_msg.contains("no previous snapshot"),
            "error should mention 'no previous snapshot', got: {err_msg}"
        );
    }

    #[test]
    fn parse_empty_leaks() -> Result<(), String> {
        let json = r#"{"leaks": [], "current": 0, "peak": 0}"#;
        let diff = parse_diff_output(json)?;
        assert!(diff.grown_allocations.is_empty());
        assert_eq!(diff.net_growth, 0);
        Ok(())
    }
}
