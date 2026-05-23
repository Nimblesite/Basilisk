//! Export profiling data to speedscope JSON and flamegraph SVG.
//!
//! Converts aggregated `ProfileData` into formats that external viewers
//! (speedscope.app, browser) can consume.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tracing::info;

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
            let name = data.thread_names.get(&tid).map_or_else(
                || format!("Thread {tid}"),
                |n| format!("Thread {tid} ({n})"),
            );

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
    "samples".clone_into(&mut options.count_name);
    options.colors =
        inferno::flamegraph::color::Palette::Basic(inferno::flamegraph::color::BasicPalette::Hot);
    options.flame_chart = false;

    let mut svg_bytes: Vec<u8> = Vec::new();
    inferno::flamegraph::from_lines(&mut options, collapsed.lines(), &mut svg_bytes)
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

// ── Profile diff comparison ─────────────────────────────────────────────────

/// A function that changed hotness between two profile sessions.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDiffEntry {
    /// Function name.
    pub name: String,
    /// Source file path.
    pub file: String,
    /// Line number (1-based).
    pub line: i32,
    /// Percentage in profile A.
    pub pct_a: f64,
    /// Percentage in profile B.
    pub pct_b: f64,
    /// Absolute change in percentage points (B - A).
    pub delta: f64,
}

/// Result of comparing two profile sessions.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileDiffResult {
    /// Functions that got hotter (higher percentage in B).
    pub hotter: Vec<FunctionDiffEntry>,
    /// Functions that got cooler (lower percentage in B).
    pub cooler: Vec<FunctionDiffEntry>,
    /// Functions with no significant change.
    pub unchanged: Vec<FunctionDiffEntry>,
}

/// Significance threshold for profile diff (percentage points).
const DIFF_SIGNIFICANCE_THRESHOLD: f64 = 0.5;

/// Compare two profile sessions and export a JSON diff.
///
/// Functions are classified as hotter, cooler, or unchanged based on a
/// significance threshold of 0.5 percentage points.
///
/// # Errors
///
/// Returns an error string if serialization or file writing fails.
pub fn export_profile_diff(
    data_a: &ProfileData,
    data_b: &ProfileData,
    output_dir: &Path,
) -> Result<ExportResult, String> {
    let diff = compute_profile_diff(data_a, data_b);

    let json = serde_json::to_string_pretty(&diff)
        .map_err(|err| format!("Failed to serialize profile diff: {err}"))?;

    let path = output_dir.join("basilisk-profile-diff.json");
    std::fs::write(&path, json.as_bytes())
        .map_err(|err| format!("Failed to write diff file {}: {err}", path.display()))?;

    info!(path = %path.display(), "exported profile diff JSON");

    Ok(ExportResult {
        path,
        format: ExportFormat::Speedscope, // Re-using; it's JSON output
    })
}

/// Compute the diff between two profile data sets.
fn compute_profile_diff(data_a: &ProfileData, data_b: &ProfileData) -> ProfileDiffResult {
    let before_pcts = collect_function_pcts(data_a);
    let after_pcts = collect_function_pcts(data_b);

    let mut all_keys: Vec<_> = before_pcts
        .keys()
        .chain(after_pcts.keys())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .cloned()
        .collect();
    all_keys.sort();

    let mut hotter = Vec::new();
    let mut cooler = Vec::new();
    let mut unchanged = Vec::new();

    for key in all_keys {
        let (before_val, after_val) = (
            before_pcts.get(&key).copied().unwrap_or(0.0),
            after_pcts.get(&key).copied().unwrap_or(0.0),
        );
        let delta = after_val - before_val;

        let entry = FunctionDiffEntry {
            name: key.name.clone(),
            file: key.file.clone(),
            line: key.line,
            pct_a: before_val,
            pct_b: after_val,
            delta,
        };

        classify_diff_entry(entry, delta, &mut hotter, &mut cooler, &mut unchanged);
    }

    hotter.sort_by(|a, b| {
        b.delta
            .partial_cmp(&a.delta)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    cooler.sort_by(|a, b| {
        a.delta
            .partial_cmp(&b.delta)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    ProfileDiffResult {
        hotter,
        cooler,
        unchanged,
    }
}

/// Classify a diff entry into hotter, cooler, or unchanged.
fn classify_diff_entry(
    entry: FunctionDiffEntry,
    delta: f64,
    hotter: &mut Vec<FunctionDiffEntry>,
    cooler: &mut Vec<FunctionDiffEntry>,
    unchanged: &mut Vec<FunctionDiffEntry>,
) {
    if delta > DIFF_SIGNIFICANCE_THRESHOLD {
        hotter.push(entry);
    } else if delta < -DIFF_SIGNIFICANCE_THRESHOLD {
        cooler.push(entry);
    } else {
        unchanged.push(entry);
    }
}

/// Collect per-function self-sample percentages from profile data.
fn collect_function_pcts(data: &ProfileData) -> HashMap<super::aggregator::FrameKey, f64> {
    let total: u64 = data
        .function_stats
        .values()
        .flat_map(HashMap::values)
        .map(|s| s.self_samples)
        .sum();

    let total_f = f64::from(u32::try_from(total).unwrap_or(u32::MAX));

    let mut result = HashMap::new();
    for funcs in data.function_stats.values() {
        for stats in funcs.values() {
            let pct = if total > 0 {
                f64::from(u32::try_from(stats.self_samples).unwrap_or(u32::MAX)) / total_f * 100.0
            } else {
                0.0
            };
            let key = super::aggregator::FrameKey {
                name: stats.name.clone(),
                file: stats.file.clone(),
                line: stats.line,
            };
            let _ = result.insert(key, pct);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiler::aggregator::{FunctionStats, SpeedscopeFrame};

    /// Build a `FunctionStats` where `total_samples == self_samples`.
    /// Every diff-builder test in this module uses this shape.
    fn fn_stats(name: &str, file: &str, line: i32, samples: u64) -> FunctionStats {
        FunctionStats {
            name: name.to_owned(),
            file: file.to_owned(),
            line,
            total_samples: samples,
            self_samples: samples,
        }
    }

    /// Insert one function under `file` in the given `ProfileData`.
    fn insert_fn(data: &mut ProfileData, file: &str, name: &str, line: i32, samples: u64) {
        let _ = data
            .function_stats
            .entry(file.to_owned())
            .or_default()
            .insert(name.to_owned(), fn_stats(name, file, line, samples));
    }

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
        let _ = data
            .thread_stacks
            .insert(1, vec![vec![0, 1, 2], vec![0, 1]]);
        let _ = data.thread_weights.insert(1, vec![0.01, 0.01]);
        let _ = data.thread_names.insert(1, "MainThread".to_owned());

        data.total_samples = 2;

        data
    }

    #[test]
    fn speedscope_json_valid_structure() -> Result<(), String> {
        let data = make_test_data();
        let dir = std::env::temp_dir();

        let result = export_speedscope(&data, "test-001", 12345, 5.2, &dir)?;
        assert!(result.path.exists());
        assert_eq!(result.format, ExportFormat::Speedscope);

        // Parse and validate structure.
        let contents =
            std::fs::read_to_string(&result.path).map_err(|err| format!("read file: {err}"))?;
        let json: serde_json::Value =
            serde_json::from_str(&contents).map_err(|err| format!("parse JSON: {err}"))?;

        assert!(json.get("$schema").is_some());
        assert!(json.get("shared").is_some());
        assert!(json.get("profiles").is_some());

        let frames = json
            .get("shared")
            .and_then(|s| s.get("frames"))
            .and_then(serde_json::Value::as_array)
            .ok_or("missing shared.frames array")?;
        assert_eq!(frames.len(), 3);

        let profiles = json
            .get("profiles")
            .and_then(serde_json::Value::as_array)
            .ok_or("missing profiles array")?;
        assert_eq!(profiles.len(), 1); // one thread

        // Clean up.
        let _ = std::fs::remove_file(&result.path);
        Ok(())
    }

    #[test]
    fn flamegraph_svg_renders() -> Result<(), String> {
        let data = make_test_data();
        let dir = std::env::temp_dir();

        let result = export_flamegraph(&data, "test-002", &dir)?;
        assert!(result.path.exists());
        assert_eq!(result.format, ExportFormat::Flamegraph);

        let contents =
            std::fs::read_to_string(&result.path).map_err(|err| format!("read file: {err}"))?;
        assert!(contents.contains("<svg"), "should be SVG");

        // Clean up.
        let _ = std::fs::remove_file(&result.path);
        Ok(())
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

    #[test]
    fn profile_diff_classifies_functions() {
        let mut data_a = ProfileData {
            total_samples: 100,
            ..ProfileData::default()
        };
        insert_fn(&mut data_a, "a.py", "hot_fn", 1, 50);
        insert_fn(&mut data_a, "a.py", "other_fn", 10, 50);

        let mut data_b = ProfileData {
            total_samples: 100,
            ..ProfileData::default()
        };
        insert_fn(&mut data_b, "a.py", "hot_fn", 1, 80);
        insert_fn(&mut data_b, "a.py", "other_fn", 10, 20);

        let diff = compute_profile_diff(&data_a, &data_b);
        assert!(
            !diff.hotter.is_empty(),
            "hot_fn should be hotter in B (80% vs 50%)"
        );
        if let Some(first_hotter) = diff.hotter.first() {
            assert_eq!(first_hotter.name, "hot_fn");
            assert!(first_hotter.delta > 0.0);
        }
    }

    #[test]
    fn profile_diff_unchanged_when_same_pct() {
        let mut data = ProfileData {
            total_samples: 100,
            ..ProfileData::default()
        };
        insert_fn(&mut data, "a.py", "fn_a", 10, 100);
        let diff = compute_profile_diff(&data, &data);
        assert!(diff.hotter.is_empty());
        assert!(diff.cooler.is_empty());
        assert_eq!(diff.unchanged.len(), 1);
    }

    #[test]
    fn profile_diff_new_function_classified_hotter() {
        let data_a = ProfileData::default();
        let mut data_b = ProfileData {
            total_samples: 100,
            ..ProfileData::default()
        };
        insert_fn(&mut data_b, "new.py", "new_fn", 1, 100);
        let diff = compute_profile_diff(&data_a, &data_b);
        assert_eq!(diff.hotter.len(), 1);
        if let Some(first_hotter) = diff.hotter.first() {
            assert_eq!(first_hotter.name, "new_fn");
        }
    }

    #[test]
    fn export_profile_diff_writes_valid_json() -> Result<(), String> {
        let mut data_a = ProfileData {
            total_samples: 100,
            ..ProfileData::default()
        };
        insert_fn(&mut data_a, "a.py", "fn_a", 10, 100);
        let dir = std::env::temp_dir();
        let result = export_profile_diff(&data_a, &ProfileData::default(), &dir)?;
        assert!(result.path.exists());
        let contents =
            std::fs::read_to_string(&result.path).map_err(|err| format!("read: {err}"))?;
        let json: serde_json::Value =
            serde_json::from_str(&contents).map_err(|err| format!("parse: {err}"))?;
        assert!(json.get("hotter").is_some());
        assert!(json.get("cooler").is_some());
        assert!(json.get("unchanged").is_some());
        let _ = std::fs::remove_file(&result.path);
        Ok(())
    }

    #[test]
    fn speedscope_export_under_200ms_for_60k_samples() -> Result<(), String> {
        // Phase 6 target: speedscope export for 60K samples <200ms.
        let mut data = ProfileData {
            total_samples: 60_000,
            ..ProfileData::default()
        };

        // Build 60K stacks across 4 threads, 200 unique frames.
        for frame_idx in 0..200_u32 {
            data.frames.push(SpeedscopeFrame {
                name: format!("func_{frame_idx}"),
                file: format!("module_{}.py", frame_idx / 10),
                line: i32::try_from(frame_idx % 50 + 1).unwrap_or(1),
            });
        }

        for thread_id in 1..=4_u64 {
            let mut stacks = Vec::with_capacity(15_000);
            let mut weights = Vec::with_capacity(15_000);
            for sample in 0..15_000_u32 {
                let depth = (sample % 5) + 1;
                let stack: Vec<usize> = (0..depth)
                    .map(|d| usize::try_from((sample + d) % 200).unwrap_or(0))
                    .collect();
                stacks.push(stack);
                weights.push(0.01);
            }
            let _ = data.thread_stacks.insert(thread_id, stacks);
            let _ = data.thread_weights.insert(thread_id, weights);
            let _ = data
                .thread_names
                .insert(thread_id, format!("Thread-{thread_id}"));
        }

        let dir = std::env::temp_dir();
        let start = std::time::Instant::now();
        let result = export_speedscope(&data, "bench-60k", 99999, 600.0, &dir)?;
        let elapsed = start.elapsed();

        // Debug builds are ~5x slower than release.
        #[cfg(debug_assertions)]
        let timing_limit = 1000;
        #[cfg(not(debug_assertions))]
        let timing_limit = 200;

        assert!(
            elapsed.as_millis() < timing_limit,
            "speedscope export took {elapsed:?}, should be <{timing_limit}ms"
        );
        assert!(result.path.exists());

        let _ = std::fs::remove_file(&result.path);
        Ok(())
    }
}
