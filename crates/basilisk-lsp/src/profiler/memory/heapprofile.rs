//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
//!
//! Export a memory snapshot as a V8 `.heapprofile` (the Chrome `DevTools`
//! `HeapProfiler.SamplingHeapProfile` schema) so VS Code's built-in profile
//! viewer renders it natively (flame chart + table, Self/Total size) — the same
//! UI used for Node.js heap profiles. See
//! <https://code.visualstudio.com/docs/nodejs/profiling>.
//!
//! Mapping: each `tracemalloc` allocation site becomes a child of the synthetic
//! root, with `selfSize` = bytes allocated at that line. (`statistics('lineno')`
//! yields one frame per site, so the tree is root → sites; the schema and this
//! builder also handle deeper tracebacks should the script switch to
//! `statistics('traceback')`.)

use std::path::Path;

use serde_json::{json, Value};

use super::{AllocationSite, MemorySnapshot};

/// Build a V8 `SamplingHeapProfile` (`.heapprofile`) JSON value from a snapshot.
#[must_use]
pub fn snapshot_to_heapprofile(snapshot: &MemorySnapshot) -> Value {
    let mut next_id: u64 = 1;
    let root_id = next_id;
    next_id += 1;

    let mut children = Vec::with_capacity(snapshot.top_allocations.len());
    let mut samples = Vec::with_capacity(snapshot.top_allocations.len());

    for (ordinal, alloc) in snapshot.top_allocations.iter().enumerate() {
        let node_id = next_id;
        next_id += 1;
        children.push(json!({
            "callFrame": alloc_call_frame(alloc),
            "selfSize": alloc.size,
            "id": node_id,
            "children": [],
        }));
        samples.push(json!({
            "size": alloc.size,
            "nodeId": node_id,
            "ordinal": ordinal,
        }));
    }

    json!({
        "head": {
            "callFrame": root_call_frame(),
            "selfSize": 0,
            "id": root_id,
            "children": children,
        },
        "samples": samples,
    })
}

/// Synthetic root frame.
fn root_call_frame() -> Value {
    json!({
        "functionName": "(root)",
        "scriptId": "0",
        "url": "",
        "lineNumber": -1,
        "columnNumber": -1,
    })
}

/// A `Runtime.CallFrame` for an allocation site. `tracemalloc` knows only the
/// file and line, so the function name is the file's basename and the URL is the
/// path; line numbers are 0-based in V8.
fn alloc_call_frame(alloc: &AllocationSite) -> Value {
    json!({
        "functionName": file_basename(&alloc.file),
        "scriptId": "0",
        "url": alloc.file,
        "lineNumber": (alloc.line - 1).max(0),
        "columnNumber": 0,
    })
}

/// Last path component of `file`, for a readable node label.
fn file_basename(file: &str) -> String {
    Path::new(file).file_name().map_or_else(
        || file.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiler::memory::diff::TraceFrame;

    fn snapshot_with_site(file: &str, line: i32, size: u64) -> MemorySnapshot {
        MemorySnapshot {
            snapshot_id: "snap-1".to_owned(),
            current_memory: size,
            peak_memory: size,
            gc_objects: 0,
            gc_counts: vec![],
            top_allocations: vec![AllocationSite {
                file: file.to_owned(),
                line,
                size,
                count: 1,
                traceback: vec![TraceFrame {
                    file: file.to_owned(),
                    line,
                }],
            }],
        }
    }

    #[test]
    fn heapprofile_matches_v8_schema() -> Result<(), String> {
        let snapshot = snapshot_with_site("/tmp/app.py", 42, 24_567_890);
        let profile = snapshot_to_heapprofile(&snapshot);

        let head = profile.get("head").ok_or("missing head")?;
        assert_eq!(head.get("selfSize").and_then(Value::as_u64), Some(0));
        let children = head
            .get("children")
            .and_then(Value::as_array)
            .ok_or("missing head.children")?;
        assert_eq!(children.len(), 1);

        let site = children.first().ok_or("expected one allocation site")?;
        assert_eq!(
            site.get("selfSize").and_then(Value::as_u64),
            Some(24_567_890)
        );
        let frame = site.get("callFrame").ok_or("missing callFrame")?;
        assert_eq!(
            frame.get("url").and_then(Value::as_str),
            Some("/tmp/app.py")
        );
        assert_eq!(frame.get("lineNumber").and_then(Value::as_i64), Some(41));
        assert_eq!(
            frame.get("functionName").and_then(Value::as_str),
            Some("app.py")
        );

        // Every node id is unique (root + each site).
        let sample_node = profile
            .get("samples")
            .and_then(Value::as_array)
            .and_then(|s| s.first())
            .and_then(|s| s.get("nodeId"))
            .and_then(Value::as_u64);
        assert_eq!(sample_node, site.get("id").and_then(Value::as_u64));
        Ok(())
    }

    #[test]
    fn empty_snapshot_yields_empty_children() {
        let snapshot = MemorySnapshot {
            snapshot_id: "empty".to_owned(),
            current_memory: 0,
            peak_memory: 0,
            gc_objects: 0,
            gc_counts: vec![],
            top_allocations: vec![],
        };
        let profile = snapshot_to_heapprofile(&snapshot);
        let children = profile
            .get("head")
            .and_then(|h| h.get("children"))
            .and_then(Value::as_array)
            .map_or(usize::MAX, Vec::len);
        assert_eq!(children, 0);
    }
}
