//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-SPEEDSCOPE
//!
//! Export aggregated CPU samples as a V8 `.cpuprofile` (the Chrome `DevTools`
//! `Profiler.Profile` schema) so VS Code's built-in profile viewer renders it
//! natively (flame chart, bottom-up + left-heavy tables). Same UI as Node.js CPU
//! profiles. See <https://code.visualstudio.com/docs/nodejs/profiling>.
//!
//! All threads are merged into one timeline (Python's GIL serializes execution),
//! producing a single call tree of `nodes` plus the `samples`/`timeDeltas`
//! arrays the viewer needs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tracing::info;

use super::aggregator::{ProfileData, SpeedscopeFrame};

/// One call-tree node while building the profile (`id` = index + 1).
struct CpuNode {
    /// Index into `ProfileData::frames`, or `None` for the synthetic root.
    frame: Option<usize>,
    /// Samples whose leaf is this node.
    hit_count: u64,
    /// Child node indices.
    children: Vec<usize>,
}

/// Build a V8 `Profiler.Profile` (`.cpuprofile`) value from aggregated data.
///
/// `sample_rate` (Hz) yields the per-sample interval as **integer**
/// microseconds (`1_000_000 / rate`), avoiding any float→int cast.
#[must_use]
pub fn build_cpuprofile(data: &ProfileData, sample_rate: u64) -> Value {
    // Index 0 is the synthetic root.
    let mut nodes = vec![CpuNode {
        frame: None,
        hit_count: 0,
        children: Vec::new(),
    }];
    let mut child_of: HashMap<(usize, usize), usize> = HashMap::new();
    let mut samples: Vec<usize> = Vec::new();
    let mut time_deltas: Vec<i64> = Vec::new();
    let micros = i64::try_from(1_000_000_u64 / sample_rate.max(1)).unwrap_or(0);

    let mut thread_ids: Vec<u64> = data.thread_stacks.keys().copied().collect();
    thread_ids.sort_unstable();

    for tid in thread_ids {
        let Some(stacks) = data.thread_stacks.get(&tid) else {
            continue;
        };
        for stack in stacks {
            // Walk root → leaf (stacks are stored root-first), creating nodes.
            let mut current = 0usize;
            for &frame_idx in stack {
                current = if let Some(&existing) = child_of.get(&(current, frame_idx)) {
                    existing
                } else {
                    let new_idx = nodes.len();
                    nodes.push(CpuNode {
                        frame: Some(frame_idx),
                        hit_count: 0,
                        children: Vec::new(),
                    });
                    if let Some(parent) = nodes.get_mut(current) {
                        parent.children.push(new_idx);
                    }
                    let _ = child_of.insert((current, frame_idx), new_idx);
                    new_idx
                };
            }
            if let Some(leaf) = nodes.get_mut(current) {
                leaf.hit_count += 1;
            }
            samples.push(current + 1);
            time_deltas.push(micros);
        }
    }

    let nodes_json: Vec<Value> = nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| node_to_json(idx, node, &data.frames))
        .collect();

    json!({
        "nodes": nodes_json,
        "startTime": 0,
        "endTime": time_deltas.iter().sum::<i64>(),
        "samples": samples,
        "timeDeltas": time_deltas,
    })
}

/// Export `ProfileData` to a `.cpuprofile` file in `output_dir`; returns the path.
///
/// # Errors
///
/// Returns an error string if serialization or the file write fails.
pub fn export_cpuprofile(
    data: &ProfileData,
    session_id: &str,
    sample_rate: u64,
    output_dir: &Path,
) -> Result<PathBuf, String> {
    let profile = build_cpuprofile(data, sample_rate);
    let json = serde_json::to_string(&profile)
        .map_err(|err| format!("Failed to serialize cpuprofile: {err}"))?;
    let path = output_dir.join(format!("basilisk-{session_id}.cpuprofile"));
    std::fs::write(&path, json)
        .map_err(|err| format!("Failed to write cpuprofile {}: {err}", path.display()))?;
    info!(path = %path.display(), "exported cpuprofile");
    Ok(path)
}

/// Serialize one node to the `ProfileNode` shape.
fn node_to_json(index: usize, node: &CpuNode, frames: &[SpeedscopeFrame]) -> Value {
    json!({
        "id": index + 1,
        "callFrame": call_frame(node.frame, frames),
        "hitCount": node.hit_count,
        "children": node.children.iter().map(|&c| c + 1).collect::<Vec<_>>(),
    })
}

/// Build a `Runtime.CallFrame` for a node (root or a real frame; lines 0-based).
fn call_frame(frame: Option<usize>, frames: &[SpeedscopeFrame]) -> Value {
    match frame.and_then(|idx| frames.get(idx)) {
        None => json!({
            "functionName": "(root)",
            "scriptId": "0",
            "url": "",
            "lineNumber": -1,
            "columnNumber": -1,
        }),
        Some(frame) => json!({
            "functionName": frame.name,
            "scriptId": "0",
            "url": frame.file,
            "lineNumber": (frame.line - 1).max(0),
            "columnNumber": 0,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(name: &str, file: &str, line: i32) -> SpeedscopeFrame {
        SpeedscopeFrame {
            name: name.to_owned(),
            file: file.to_owned(),
            line,
        }
    }

    #[test]
    fn cpuprofile_matches_v8_schema() -> Result<(), String> {
        // Two samples, both the single-frame stack [frame 0], at 100 Hz.
        let data = ProfileData {
            frames: vec![frame("hot_function", "/tmp/app.py", 19)],
            thread_stacks: HashMap::from([(1_u64, vec![vec![0], vec![0]])]),
            thread_weights: HashMap::from([(1_u64, vec![0.01, 0.01])]),
            ..ProfileData::default()
        };

        let profile = build_cpuprofile(&data, 100);

        let nodes = profile
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or("missing nodes")?;
        assert_eq!(nodes.len(), 2, "root + one frame node");

        let leaf = nodes.get(1).ok_or("missing leaf node")?;
        assert_eq!(leaf.get("hitCount").and_then(Value::as_u64), Some(2));
        let call = leaf.get("callFrame").ok_or("missing callFrame")?;
        assert_eq!(
            call.get("functionName").and_then(Value::as_str),
            Some("hot_function")
        );
        assert_eq!(call.get("url").and_then(Value::as_str), Some("/tmp/app.py"));
        assert_eq!(call.get("lineNumber").and_then(Value::as_i64), Some(18));

        // The root references the leaf as a child.
        let root_children = nodes
            .first()
            .and_then(|root| root.get("children"))
            .and_then(Value::as_array)
            .ok_or("missing root children")?;
        assert_eq!(root_children.first().and_then(Value::as_u64), Some(2));

        // Samples point at the leaf (id 2); 10 ms per sample at 100 Hz.
        assert_eq!(
            profile.get("samples").and_then(Value::as_array),
            Some(&vec![json!(2), json!(2)])
        );
        assert_eq!(
            profile.get("timeDeltas").and_then(Value::as_array),
            Some(&vec![json!(10_000), json!(10_000)])
        );
        assert_eq!(profile.get("endTime").and_then(Value::as_i64), Some(20_000));
        Ok(())
    }

    #[test]
    fn export_writes_a_valid_cpuprofile_file() -> Result<(), String> {
        let data = ProfileData {
            frames: vec![frame("f", "/tmp/a.py", 1)],
            thread_stacks: HashMap::from([(1_u64, vec![vec![0]])]),
            thread_weights: HashMap::from([(1_u64, vec![0.01])]),
            ..ProfileData::default()
        };
        let path = export_cpuprofile(&data, "basilisk-unit-test", 100, &std::env::temp_dir())?;
        assert!(
            path.extension().is_some_and(|ext| ext == "cpuprofile"),
            "path: {}",
            path.display()
        );
        let contents = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let parsed: Value = serde_json::from_str(&contents).map_err(|err| err.to_string())?;
        assert!(parsed
            .get("nodes")
            .and_then(Value::as_array)
            .is_some_and(|nodes| nodes.len() == 2));
        let _ = std::fs::remove_file(&path);
        Ok(())
    }
}
