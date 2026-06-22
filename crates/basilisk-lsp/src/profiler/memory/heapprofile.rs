//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY
//!
//! Export a memory snapshot as a V8 `.heapprofile` (the Chrome `DevTools`
//! `HeapProfiler.SamplingHeapProfile` schema) so VS Code's built-in profile
//! viewer renders it natively (flame chart + table, Self/Total size) — the same
//! UI used for Node.js heap profiles. See
//! <https://code.visualstudio.com/docs/nodejs/profiling>.
//!
//! Mapping: each allocation's `tracemalloc` traceback becomes a path from the
//! synthetic root down to the allocation site; shared call-stack prefixes merge,
//! so the result is a real call tree (a flame graph with depth), not a flat list
//! ([PROFILE-MEMORY-FINAL]). `selfSize` accumulates at the leaf of each path.
//! Node labels are the source line at that frame (read once per file, cached),
//! falling back to the file's basename — so the chart reads like the code.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

use super::{AllocationSite, MemorySnapshot};

/// One node in the call tree under construction (a flat arena addressed by index).
struct TreeNode {
    /// Source file of this frame (empty for the synthetic root).
    file: String,
    /// 1-based line of this frame (`-1` for the root).
    line: i32,
    /// Bytes allocated *directly* at this frame (the leaf of some traceback).
    self_size: u64,
    /// Stable node id for the `.heapprofile` schema.
    id: u64,
    /// Child node indices, in insertion order.
    children: Vec<usize>,
    /// `(file, line)` → child index, so a shared call-stack prefix reuses nodes.
    index: HashMap<(String, i32), usize>,
}

/// Build a V8 `SamplingHeapProfile` (`.heapprofile`) JSON value from a snapshot.
#[must_use]
pub fn snapshot_to_heapprofile(snapshot: &MemorySnapshot) -> Value {
    let mut nodes: Vec<TreeNode> = vec![new_node(0, String::new(), -1)];
    let mut next_id: u64 = 1; // root keeps id 0; real frames start at 1.
    let mut samples = Vec::with_capacity(snapshot.top_allocations.len());

    for (ordinal, alloc) in snapshot.top_allocations.iter().enumerate() {
        let leaf = insert_path(&mut nodes, &mut next_id, alloc);
        // `leaf` is always a live arena index; skip defensively rather than panic.
        if let Some(node) = nodes.get_mut(leaf) {
            node.self_size += alloc.size;
            samples.push(json!({
                "size": alloc.size,
                "nodeId": node.id,
                "ordinal": ordinal,
            }));
        }
    }

    let mut labels = SourceLabels::default();
    let head = nodes
        .first()
        .map_or_else(|| json!({}), |root| node_to_json(&nodes, root, &mut labels));
    json!({ "head": head, "samples": samples })
}

/// Walk an allocation's traceback root→leaf, creating or reusing child nodes,
/// and return the leaf node's arena index. Falls back to the single allocation
/// site when no traceback was captured.
fn insert_path(nodes: &mut Vec<TreeNode>, next_id: &mut u64, alloc: &AllocationSite) -> usize {
    let mut cursor = 0usize; // the root.
    for (file, line) in frames_of(alloc) {
        let key = (file.to_owned(), line);
        if let Some(child) = nodes
            .get(cursor)
            .and_then(|node| node.index.get(&key).copied())
        {
            cursor = child;
            continue;
        }
        let child = nodes.len();
        let id = *next_id;
        *next_id += 1;
        nodes.push(new_node(id, file.to_owned(), line));
        if let Some(parent) = nodes.get_mut(cursor) {
            let _ = parent.index.insert(key, child);
            parent.children.push(child);
        }
        cursor = child;
    }
    cursor
}

/// The allocation's frames in root→leaf order (`tracemalloc` orders oldest→most
/// recent), or just the allocation site if the traceback is empty.
fn frames_of(alloc: &AllocationSite) -> Vec<(&str, i32)> {
    if alloc.traceback.is_empty() {
        vec![(alloc.file.as_str(), alloc.line)]
    } else {
        alloc
            .traceback
            .iter()
            .map(|frame| (frame.file.as_str(), frame.line))
            .collect()
    }
}

/// An empty arena node.
fn new_node(id: u64, file: String, line: i32) -> TreeNode {
    TreeNode {
        file,
        line,
        self_size: 0,
        id,
        children: Vec::new(),
        index: HashMap::new(),
    }
}

/// Recursively serialize an arena node to the `.heapprofile` schema. Child
/// indices are resolved through `get`, so a stray index is skipped, not panicked.
fn node_to_json(nodes: &[TreeNode], node: &TreeNode, labels: &mut SourceLabels) -> Value {
    let children: Vec<Value> = node
        .children
        .iter()
        .filter_map(|&child| nodes.get(child))
        .map(|child| node_to_json(nodes, child, labels))
        .collect();
    json!({
        "callFrame": call_frame(node, labels),
        "selfSize": node.self_size,
        "id": node.id,
        "children": children,
    })
}

/// The `Runtime.CallFrame` for a node: the synthetic root, or a frame labelled by
/// its source line (basename fallback). Line numbers are 0-based in V8.
fn call_frame(node: &TreeNode, labels: &mut SourceLabels) -> Value {
    if node.line < 0 {
        return json!({
            "functionName": "(root)",
            "scriptId": "0",
            "url": "",
            "lineNumber": -1,
            "columnNumber": -1,
        });
    }
    json!({
        "functionName": labels.label(&node.file, node.line),
        "scriptId": "0",
        "url": node.file,
        "lineNumber": (node.line - 1).max(0),
        "columnNumber": 0,
    })
}

/// Resolves a readable label for a `(file, line)` frame: the trimmed source line,
/// or the file's basename when the source can't be read. Each file is read at
/// most once.
#[derive(Default)]
struct SourceLabels {
    cache: HashMap<String, Option<Vec<String>>>,
}

impl SourceLabels {
    fn label(&mut self, file: &str, line: i32) -> String {
        let lines = self
            .cache
            .entry(file.to_owned())
            .or_insert_with(|| read_source_lines(file));
        usize::try_from(line - 1)
            .ok()
            .and_then(|idx| lines.as_ref().and_then(|all| all.get(idx)))
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| file_basename(file))
    }
}

/// Largest source file read to label a frame. Labels are a nicety; a pathological
/// (generated/minified) source must never make the builder slurp a huge file —
/// it falls back to the basename instead.
const MAX_LABEL_SOURCE_BYTES: u64 = 2 * 1024 * 1024;

/// Read a source file into lines for labelling, or `None` when it can't be read
/// or is larger than [`MAX_LABEL_SOURCE_BYTES`]. Paths come from the debuggee's
/// own tracebacks (real source files), and each file is read at most once.
fn read_source_lines(file: &str) -> Option<Vec<String>> {
    if std::fs::metadata(file).ok()?.len() > MAX_LABEL_SOURCE_BYTES {
        return None;
    }
    std::fs::read_to_string(file)
        .ok()
        .map(|content| content.lines().map(str::to_owned).collect())
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

    fn frame(file: &str, line: i32) -> TraceFrame {
        TraceFrame {
            file: file.to_owned(),
            line,
        }
    }

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
                traceback: vec![frame(file, line)],
            }],
        }
    }

    /// Find a node by its (basename-fallback) label among a children array.
    fn child_with_url<'a>(children: &'a [Value], url: &str) -> Option<&'a Value> {
        children.iter().find(|node| {
            node.get("callFrame")
                .and_then(|cf| cf.get("url"))
                .and_then(Value::as_str)
                == Some(url)
        })
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
        // Unreadable synthetic path -> basename fallback label.
        assert_eq!(
            frame.get("functionName").and_then(Value::as_str),
            Some("app.py")
        );

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

    #[test]
    fn shared_prefixes_merge_into_a_real_call_tree() -> Result<(), String> {
        // Two allocations that share a `main` prefix then diverge must produce a
        // nested tree (root -> main -> {build_a -> leaf, build_b -> leaf}), with
        // size accumulating at each leaf — not a flat list of sites.
        let snapshot = MemorySnapshot {
            snapshot_id: "tree".to_owned(),
            current_memory: 300,
            peak_memory: 300,
            gc_objects: 0,
            gc_counts: vec![],
            top_allocations: vec![
                AllocationSite {
                    file: "/w/a.py".to_owned(),
                    line: 9,
                    size: 100,
                    count: 1,
                    traceback: vec![
                        frame("/w/main.py", 1),
                        frame("/w/a.py", 5),
                        frame("/w/a.py", 9),
                    ],
                },
                AllocationSite {
                    file: "/w/b.py".to_owned(),
                    line: 12,
                    size: 200,
                    count: 1,
                    traceback: vec![
                        frame("/w/main.py", 1),
                        frame("/w/b.py", 6),
                        frame("/w/b.py", 12),
                    ],
                },
            ],
        };
        let profile = snapshot_to_heapprofile(&snapshot);
        let head = profile.get("head").ok_or("missing head")?;

        // root -> exactly one shared `main` node.
        let top = head
            .get("children")
            .and_then(Value::as_array)
            .ok_or("missing children")?;
        assert_eq!(
            top.len(),
            1,
            "the shared main prefix must merge into one node"
        );
        let main = top.first().ok_or("missing main node")?;

        // main -> two diverging branches.
        let branches = main
            .get("children")
            .and_then(Value::as_array)
            .ok_or("missing main.children")?;
        assert_eq!(branches.len(), 2, "main must branch into a.py and b.py");

        // Depth is real: root(1) -> main(2) -> build(3) -> leaf(4).
        assert_eq!(tree_depth(head), 4, "the call tree must have genuine depth");

        // selfSize lands on the leaves only; intermediate frames stay at 0.
        assert_eq!(main.get("selfSize").and_then(Value::as_u64), Some(0));
        let a_branch = child_with_url(branches, "/w/a.py").ok_or("missing a.py branch")?;
        let a_leaf = a_branch
            .get("children")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
            .ok_or("missing a.py leaf")?;
        assert_eq!(a_leaf.get("selfSize").and_then(Value::as_u64), Some(100));
        Ok(())
    }

    fn tree_depth(node: &Value) -> usize {
        let children = node.get("children").and_then(Value::as_array);
        let deepest = children
            .into_iter()
            .flatten()
            .map(tree_depth)
            .max()
            .unwrap_or(0);
        1 + deepest
    }

    #[test]
    fn allocation_without_traceback_falls_back_to_its_site() {
        // A site with an empty traceback still becomes one leaf under the root.
        let mut snapshot = snapshot_with_site("/tmp/app.py", 7, 512);
        if let Some(site) = snapshot.top_allocations.first_mut() {
            site.traceback.clear();
        }
        let profile = snapshot_to_heapprofile(&snapshot);
        let children = profile
            .get("head")
            .and_then(|h| h.get("children"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        assert_eq!(children, 1);
    }
}
