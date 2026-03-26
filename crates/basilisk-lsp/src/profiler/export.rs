//! Export profiling data to speedscope JSON and flamegraph SVG.
//!
//! Converts aggregated `ProfileData` into formats that external viewers
//! (speedscope.app, browser) can consume.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tracing::{error, info};

use super::aggregator::ProfileData;

/// Output format for profile export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Speedscope JSON format (`.speedscope.json`).
    Speedscope,
    /// Flamegraph SVG via inferno (`.svg`).
    Flamegraph,
}

/// Result of exporting profile data to a file.
#[derive(Debug, Clone)]
pub struct ExportResult {
    /// Path to the exported file.
    pub path: PathBuf,
    /// The format that was written.
    pub format: ExportFormat,
}

// ── Speedscope JSON ───────────────────────────────────────────────────────────

/// Top-level speedscope JSON structure.
///
/// See <https://www.speedscope.app/file-format-schema.json>.
#[derive(Serialize)]
struct SpeedscopeFile {
    #[serde(rename = "$schema")]
    schema: &'static str,
    shared: SpeedscopeShared,
    profiles: Vec<SpeedscopeProfile>,
    name: String,
    exporter: &'static str,
    #[serde(rename = "activeProfileIndex")]
    active_profile_index: usize,
}

#[derive(Serialize)]
struct SpeedscopeShared {
    frames: Vec<SpeedscopeFrameRef>,
}

#[derive(Serialize)]
struct SpeedscopeFrameRef {
    name: String,
    file: String,
    line: i32,
}

#[derive(Serialize)]
struct SpeedscopeProfile {
    #[serde(rename = "type")]
    profile_type: &'static str,
    name: String,
    unit: &'static str,
    #[serde(rename = "startValue")]
    start_value: f64,
    #[serde(rename = "endValue")]
    end_value: f64,
    samples: Vec<Vec<usize>>,
    weights: Vec<f64>,
}

/// Export `ProfileData` to speedscope JSON format.
///
/// Writes the file to `output_dir` and returns the path.
///
/// # Errors
///
/// Returns an error string if serialization or file writing fails.
pub fn export_speedscope(
    data: &ProfileData,
    session_id: &str,
    pid: u32,
    duration_secs: f64,
    output_dir: &Path,
) -> Result<ExportResult, String> {
    let frames: Vec<SpeedscopeFrameRef> = data
        .frames
        .iter()
        .map(|f| SpeedscopeFrameRef {
            name: f.name.clone(),
            file: f.file.clone(),
            line: f.line,
        })
        .collect();

    // Build one profile per thread, sorted by thread ID.
    let mut thread_ids: Vec<u64> = data.thread_stacks.keys().copied().collect();
    thread_ids.sort_unstable();

    let profiles: Vec<SpeedscopeProfile> = thread_ids
        .iter()
        .map(|&tid| {
            let name = data
                .thread_names
                .get(&tid)
                .map_or_else(|| format!("Thread {tid}"), |n| format!("Thread {tid} ({n})"));

            let stacks = data.thread_stacks.get(&tid).cloned().unwrap_or_default();
            let weights = data.thread_weights.get(&tid).cloned().unwrap_or_default();
            let end_value: f64 = weights.iter().sum();

            SpeedscopeProfile {
                profile_type: "sampled",
                name,
                unit: "seconds",
                start_value: 0.0,
                end_value,
                samples: stacks,
                weights,
            }
        })
        .collect();

    let file = SpeedscopeFile {
        schema: "https://www.speedscope.app/file-format-schema.json",
        shared: SpeedscopeShared { frames },
        profiles,
        name: format!("basilisk profile \u{2014} PID {pid} ({duration_secs:.1}s)"),
        exporter: "basilisk-profiler 0.1.0",
        active_profile_index: 0,
    };

    let json = serde_json::to_string_pretty(&file)
        .map_err(|err| format!("Failed to serialize speedscope JSON: {err}"))?;

    let filename = format!("basilisk-{session_id}.speedscope.json");
    let path = output_dir.join(filename);

    std::fs::write(&path, json.as_bytes())
        .map_err(|err| format!("Failed to write speedscope file {}: {err}", path.display()))?;

    info!(path = %path.display(), "exported speedscope JSON");

    Ok(ExportResult {
        path,
        format: ExportFormat::Speedscope,
    })
}

// ── Flamegraph SVG ────────────────────────────────────────────────────────────

/// Export `ProfileData` to flamegraph SVG via the `inferno` crate.
///
/// Converts aggregated stacks to inferno's collapsed format, then renders SVG.
///
/// # Errors
///
/// Returns an error string if rendering or file writing fails.
pub fn export_flamegraph(
    data: &ProfileData,
    session_id: &str,
    output_dir: &Path,
) -> Result<ExportResult, String> {
    // Build collapsed stacks: "root;caller;leaf count\n"
    let collapsed = build_collapsed_stacks(data);

    let mut options = inferno::flamegraph::Options::default();
    options.title = format!("Basilisk Profile \u{2014} {session_id}");
    options.count_name = "samples".to_owned();
    options.colors = inferno::flamegraph::color::Palette::Basic(
        inferno::flamegraph::color::BasicPalette::Hot,
    );
    options.flame_chart = false;

    let mut svg_bytes: Vec<u8> = Vec::new();
    inferno::flamegraph::from_lines(
        &mut options,
        collapsed.lines(),
        &mut svg_bytes,
    )
    .map_err(|err| format!("Flamegraph rendering failed: {err}"))?;

    let filename = format!("basilisk-{session_id}.flamegraph.svg");
    let path = output_dir.join(filename);

    std::fs::write(&path, &svg_bytes)
        .map_err(|err| format!("Failed to write flamegraph SVG {}: {err}", path.display()))?;

    info!(path = %path.display(), "exported flamegraph SVG");

    Ok(ExportResult {
        path,
        format: ExportFormat::Flamegraph,
    })
}

/// Build inferno-compatible collapsed stack lines from profile data.
///
/// Format: `root_fn;caller_fn;leaf_fn count\n`
fn build_collapsed_stacks(data: &ProfileData) -> String {
    // Aggregate identical stacks into counts.
    let mut stack_counts: HashMap<String, u64> = HashMap::new();

    for stacks in data.thread_stacks.values() {
        for stack in stacks {
            // Stack is root-first (indices into data.frames).
            let collapsed: String = stack
                .iter()
                .filter_map(|&idx| {
                    let frame = data.frames.get(idx)?;
                    Some(format!("{} ({}:{})", frame.name, frame.file, frame.line))
                })
                .collect::<Vec<_>>()
                .join(";");

            *stack_counts.entry(collapsed).or_insert(0) += 1;
        }
    }

    let mut lines: Vec<String> = stack_counts
        .into_iter()
        .map(|(stack, count)| format!("{stack} {count}"))
        .collect();

    lines.sort();
    lines.join("\n")
}

/// Export profile data in the requested format.
///
/// Convenience wrapper that dispatches to the correct exporter.
///
/// # Errors
///
/// Returns an error string if export fails.
pub fn export(
    data: &ProfileData,
    format: ExportFormat,
    session_id: &str,
    pid: u32,
    duration_secs: f64,
    output_dir: &Path,
) -> Result<ExportResult, String> {
    match format {
        ExportFormat::Speedscope => {
            export_speedscope(data, session_id, pid, duration_secs, output_dir)
        }
        ExportFormat::Flamegraph => export_flamegraph(data, session_id, output_dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiler::aggregator::{FrameKey, SpeedscopeFrame};

    fn make_test_data() -> ProfileData {
        let mut data = ProfileData::default();

        // Manually build profile data for testing (no py-spy needed).
        data.frames.push(SpeedscopeFrame {
            name: "<module>".to_owned(),
            file: "main.py".to_owned(),
            line: 1,
        });
        data.frames.push(SpeedscopeFrame {
            name: "process".to_owned(),
            file: "pipeline.py".to_owned(),
            line: 42,
        });
        data.frames.push(SpeedscopeFrame {
            name: "parse".to_owned(),
            file: "parser.py".to_owned(),
            line: 15,
        });

        // Thread 1: two samples.
        data.thread_stacks
            .insert(1, vec![vec![0, 1, 2], vec![0, 1]]);
        data.thread_weights.insert(1, vec![0.01, 0.01]);
        data.thread_names
            .insert(1, "MainThread".to_owned());

        data.total_samples = 2;

        data
    }

    #[test]
    fn speedscope_json_valid_structure() {
        let data = make_test_data();
        let dir = std::env::temp_dir();

        let result = export_speedscope(&data, "test-001", 12345, 5.2, &dir);
        assert!(result.is_ok(), "export should succeed: {result:?}");

        let result = result.expect("already checked");
        assert!(result.path.exists());
        assert_eq!(result.format, ExportFormat::Speedscope);

        // Parse and validate structure.
        let contents = std::fs::read_to_string(&result.path).expect("read file");
        let json: serde_json::Value =
            serde_json::from_str(&contents).expect("valid JSON");

        assert!(json.get("$schema").is_some());
        assert!(json.get("shared").is_some());
        assert!(json.get("profiles").is_some());

        let frames = json["shared"]["frames"].as_array().expect("frames array");
        assert_eq!(frames.len(), 3);

        let profiles = json["profiles"].as_array().expect("profiles array");
        assert_eq!(profiles.len(), 1); // one thread

        // Clean up.
        let _ = std::fs::remove_file(&result.path);
    }

    #[test]
    fn flamegraph_svg_renders() {
        let data = make_test_data();
        let dir = std::env::temp_dir();

        let result = export_flamegraph(&data, "test-002", &dir);
        assert!(result.is_ok(), "export should succeed: {result:?}");

        let result = result.expect("already checked");
        assert!(result.path.exists());
        assert_eq!(result.format, ExportFormat::Flamegraph);

        let contents = std::fs::read_to_string(&result.path).expect("read file");
        assert!(contents.contains("<svg"), "should be SVG");

        // Clean up.
        let _ = std::fs::remove_file(&result.path);
    }

    #[test]
    fn collapsed_stacks_format() {
        let data = make_test_data();
        let collapsed = build_collapsed_stacks(&data);

        // Should contain stack lines with counts.
        assert!(collapsed.contains("<module>"));
        assert!(collapsed.contains("process"));
        assert!(collapsed.contains("parse"));
    }
}
