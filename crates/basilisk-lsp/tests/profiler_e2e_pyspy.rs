//! Real E2E tests that spawn Python processes and profile them with py-spy.
//!
//! These tests prove the ENTIRE profiler pipeline works end-to-end:
//! 1. Spawn a CPU-bound Python process
//! 2. Attach py-spy via `ProfileSessionManager`
//! 3. Collect real stack trace samples
//! 4. Aggregate into hotspot data
//! 5. Export to speedscope JSON and flamegraph SVG
//! 6. Generate LSP diagnostics
//! 7. Verify hotspots match the known CPU-bound code
//!
//! **Requires elevated privileges on macOS** (`sudo cargo test`).
//! On Linux with `ptrace_scope=0`, runs without elevation.
//! Tests are `#[ignore]`d by default — run explicitly with:
//!
//! ```sh
//! sudo cargo test -p basilisk-lsp --test profiler_e2e_pyspy -- --ignored
//! ```

#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    missing_docs,
    dead_code,
    unused_imports,
    clippy::needless_raw_string_hashes
)]

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use basilisk_lsp::profiler::aggregator::{HotspotConfig, ProfileData};
use basilisk_lsp::profiler::diagnostics::generate_diagnostics;
use basilisk_lsp::profiler::export::{export, ExportFormat};
use basilisk_lsp::profiler::ProfileSessionManager;

/// Path to Python 3 interpreter.
fn python_path() -> String {
    std::env::var("PYTHON")
        .or_else(|_| std::env::var("BASILISK_PYTHON"))
        .unwrap_or_else(|_| "python3".to_owned())
}

/// Spawn a CPU-bound Python script that burns CPU in a known function.
/// Returns the child process and the temp file path.
fn spawn_cpu_bound_python() -> (Child, std::path::PathBuf) {
    let dir = std::env::temp_dir().join("basilisk_profiler_e2e");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let script_path = dir.join("cpu_burner.py");

    let script = r#"
import time
import sys

def hot_function():
    """This function should appear as a hotspot."""
    total = 0
    for i in range(10_000_000):
        total += i * i
    return total

def warm_function():
    """This function should appear with moderate CPU usage."""
    total = 0
    for i in range(2_000_000):
        total += i
    return total

def cold_function():
    """This function should barely register."""
    time.sleep(0.01)
    return 42

def main():
    # Signal we're ready for profiling
    print("READY", flush=True)
    sys.stdout.flush()

    while True:
        hot_function()
        warm_function()
        cold_function()

if __name__ == "__main__":
    main()
"#;

    std::fs::write(&script_path, script).expect("write Python script");

    let child = Command::new(python_path())
        .arg(&script_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn Python process");

    (child, script_path)
}

/// Wait for the Python process to print "READY".
fn wait_for_ready(child: &mut Child) {
    use std::io::BufRead;
    let stdout = child.stdout.as_mut().expect("stdout");
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).expect("read stdout");
        assert!(bytes > 0, "Python process exited before printing READY");
        if line.trim() == "READY" {
            return;
        }
        assert!(
            std::time::Instant::now() <= deadline,
            "Timed out waiting for Python READY signal"
        );
    }
}

// ── Verification helpers for the full pipeline test ──────────────────────

/// Verify `hot_function` is identified as a hotspot with significant self time.
fn verify_hotspot_detection(result: &basilisk_lsp::profiler::StopResult) {
    let has_hot_function = result
        .hot_functions
        .iter()
        .any(|f| f.name == "hot_function");

    assert!(
        has_hot_function,
        "hot_function() MUST appear as a hotspot. Got: {:?}",
        result
            .hot_functions
            .iter()
            .map(|f| &f.name)
            .collect::<Vec<_>>()
    );

    let hot_fn = result
        .hot_functions
        .iter()
        .find(|f| f.name == "hot_function")
        .expect("hot_function should be in results");

    assert!(
        hot_fn.self_percentage > 20.0,
        "hot_function should have >20% self time (got {:.1}%)",
        hot_fn.self_percentage
    );

    assert!(
        !result.hot_lines.is_empty(),
        "should detect hot lines in the Python script"
    );
}

/// Verify speedscope JSON export has correct schema, frames, and profiles.
fn verify_speedscope_export(
    result: &basilisk_lsp::profiler::StopResult,
    session_id: &str,
    pid: u32,
    output_dir: &std::path::Path,
) {
    let speedscope = export(
        &result.data,
        ExportFormat::Speedscope,
        session_id,
        pid,
        result.duration,
        output_dir,
    )
    .expect("export speedscope");

    assert!(
        speedscope.path.exists(),
        "speedscope JSON file should exist"
    );

    let json_str = std::fs::read_to_string(&speedscope.path).expect("read speedscope");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("parse speedscope JSON");

    let schema = parsed.get("$schema").and_then(|v| v.as_str());
    assert_eq!(
        schema,
        Some("https://www.speedscope.app/file-format-schema.json"),
        "should have speedscope schema"
    );

    let profiles = parsed
        .get("profiles")
        .and_then(|v| v.as_array())
        .expect("profiles array");
    assert!(
        !profiles.is_empty(),
        "should have at least one thread profile"
    );

    let frames = parsed
        .get("shared")
        .and_then(|s| s.get("frames"))
        .and_then(|f| f.as_array())
        .expect("shared.frames");
    assert!(!frames.is_empty(), "should have collected frames");

    let frame_names: Vec<&str> = frames
        .iter()
        .filter_map(|f| f.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(
        frame_names.contains(&"hot_function"),
        "hot_function must appear in speedscope frames"
    );
}

/// Verify flamegraph SVG export is valid and contains `hot_function`.
fn verify_flamegraph_export(
    result: &basilisk_lsp::profiler::StopResult,
    session_id: &str,
    pid: u32,
    output_dir: &std::path::Path,
) {
    let flamegraph = export(
        &result.data,
        ExportFormat::Flamegraph,
        session_id,
        pid,
        result.duration,
        output_dir,
    )
    .expect("export flamegraph");

    assert!(flamegraph.path.exists(), "flamegraph SVG should exist");

    let svg = std::fs::read_to_string(&flamegraph.path).expect("read flamegraph");
    assert!(
        svg.starts_with("<svg") || svg.starts_with("<?xml"),
        "should be valid SVG"
    );
    assert!(
        svg.contains("hot_function"),
        "hot_function should appear in flamegraph SVG"
    );
}

/// Verify LSP diagnostics mention `hot_function`.
fn verify_lsp_diagnostics(result: &basilisk_lsp::profiler::StopResult) {
    let config = HotspotConfig {
        line_threshold_pct: 1.0,
        function_threshold_pct: 2.0,
        max_diagnostics_per_file: 20,
    };
    let diagnostics = generate_diagnostics(&result.data, &config);

    assert!(
        !diagnostics.is_empty(),
        "should generate diagnostics from real profile data"
    );

    let all_messages: Vec<String> = diagnostics
        .values()
        .flat_map(|diags| diags.iter().map(|d| d.message.clone()))
        .collect();
    assert!(
        all_messages.iter().any(|m| m.contains("hot_function")),
        "diagnostics should mention hot_function"
    );
}

// ── Real py-spy E2E tests ─────────────────────────────────────────────────

/// Full pipeline: spawn Python, attach py-spy, collect samples, verify hotspots.
///
/// This is THE test that proves the profiler actually works. It verifies:
/// - py-spy can attach to a real Python process
/// - Real stack traces are collected
/// - The hot function (`hot_function`) is identified as a hotspot
/// - Export to speedscope JSON produces valid data
/// - Export to flamegraph SVG produces renderable output
/// - LSP diagnostics are generated for hot lines
#[tokio::test]
#[ignore = "requires elevated privileges (sudo) on macOS"]
async fn e2e_profile_cpu_bound_python_and_find_hotspots() {
    let (mut child, _script_path) = spawn_cpu_bound_python();
    let pid = child.id();
    wait_for_ready(&mut child);
    tokio::time::sleep(Duration::from_millis(500)).await;

    let manager = ProfileSessionManager::new();
    let start_result = match manager.start(pid, Some(100), Some(false), None).await {
        Ok(r) => r,
        Err(err) => {
            let msg = format!("{err}");
            if msg.contains("ermission")
                || msg.contains("denied")
                || msg.contains("attach")
                || msg.contains("Failed to open process")
                || msg.contains("profiler-helper")
            {
                eprintln!("SKIP: py-spy permission denied (expected on macOS without root): {msg}");
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
            panic!("start profiling failed unexpectedly: {msg}");
        }
    };

    assert!(
        !start_result.session_id.is_empty(),
        "session ID must not be empty"
    );
    assert_eq!(start_result.pid, pid);
    assert!(
        start_result.python_version.starts_with("3."),
        "should detect Python 3.x, got: {}",
        start_result.python_version
    );

    tokio::time::sleep(Duration::from_secs(3)).await;

    let snapshot = manager
        .snapshot(&start_result.session_id)
        .await
        .expect("take snapshot");
    assert!(
        snapshot.total_samples > 10,
        "should have collected real samples (got {})",
        snapshot.total_samples
    );

    let result = manager
        .stop(&start_result.session_id)
        .await
        .expect("stop profiling");

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        result.total_samples > 50,
        "should have hundreds of samples after 3s at 100Hz (got {})",
        result.total_samples
    );
    assert!(
        result.duration > 2.0,
        "profiling should have run for >2s (got {:.1}s)",
        result.duration
    );

    let output_dir = std::env::temp_dir().join("basilisk_profiler_e2e_export");
    std::fs::create_dir_all(&output_dir).expect("create export dir");

    verify_hotspot_detection(&result);
    verify_speedscope_export(&result, &start_result.session_id, pid, &output_dir);
    verify_flamegraph_export(&result, &start_result.session_id, pid, &output_dir);
    verify_lsp_diagnostics(&result);

    let sessions = manager.list().await;
    assert!(
        sessions.is_empty(),
        "all sessions should be stopped after stop()"
    );
}

/// Test that profiling a non-existent PID returns a clear error.
#[tokio::test]
async fn e2e_profile_nonexistent_pid_returns_error() {
    let manager = ProfileSessionManager::new();
    let result = manager.start(999_999_999, None, None, None).await;
    assert!(result.is_err(), "profiling a dead PID should fail");
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("not found")
            || msg.contains("No such process")
            || msg.contains("attach")
            || msg.contains("Permission denied")
            || msg.contains("profiler-helper"),
        "error should indicate failure, got: {msg}"
    );
}

/// Test that stopping a nonexistent session returns an error.
#[tokio::test]
async fn e2e_stop_nonexistent_session_returns_error() {
    let manager = ProfileSessionManager::new();
    let result = manager.stop("nonexistent-session-id").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("nonexistent-session-id"),
        "error should contain session ID, got: {msg}"
    );
}

/// Test that snapshot of nonexistent session returns an error.
#[tokio::test]
async fn e2e_snapshot_nonexistent_session_returns_error() {
    let manager = ProfileSessionManager::new();
    let result = manager.snapshot("nonexistent-session-id").await;
    assert!(result.is_err());
}

/// Test memory profiling scripts generate valid Python code.
#[test]
fn e2e_memory_scripts_are_valid_python_syntax() {
    let python = python_path();

    let scripts = [
        (
            "start_tracemalloc",
            basilisk_lsp::profiler::memory::scripts::start_tracemalloc(25),
        ),
        (
            "take_snapshot",
            basilisk_lsp::profiler::memory::scripts::take_snapshot(500),
        ),
        (
            "diff_snapshot",
            basilisk_lsp::profiler::memory::scripts::diff_snapshot(500),
        ),
        (
            "gc_collect",
            basilisk_lsp::profiler::memory::scripts::gc_collect().to_owned(),
        ),
        (
            "objects_by_type",
            basilisk_lsp::profiler::memory::scripts::objects_by_type("dict", 50),
        ),
        (
            "walk_references",
            basilisk_lsp::profiler::memory::scripts::walk_references("dict", None, 5, 200),
        ),
    ];

    for (name, script) in &scripts {
        let check_script = format!(
            "import ast; ast.parse('''{}''')",
            script.replace('\'', "\\'").replace('\n', "\\n")
        );
        let output = Command::new(&python)
            .args(["-c", &check_script])
            .output()
            .unwrap_or_else(|e| panic!("failed to run python for {name}: {e}"));

        assert!(
            output.status.success(),
            "script '{name}' has invalid Python syntax:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

// ── Aggregation pipeline test ────────────────────────────────────────────

/// Ingest 10,000 mock samples with a known distribution into `ProfileData`.
fn build_realistic_profile_data() -> ProfileData {
    let mut data = ProfileData::default();
    let sample_weight = 0.01;

    for i in 0..10_000u64 {
        let traces = match i % 100 {
            0..60 => vec![
                make_frame("main", "/app/main.py", 10),
                make_frame("hot_function", "/app/compute.py", 42),
            ],
            60..85 => vec![
                make_frame("main", "/app/main.py", 10),
                make_frame("warm_function", "/app/compute.py", 80),
            ],
            85..90 => vec![
                make_frame("main", "/app/main.py", 10),
                make_frame("cold_function", "/app/utils.py", 15),
            ],
            _ => vec![
                make_frame("main", "/app/main.py", 10),
                make_frame("parse_config", "/app/config.py", 5),
            ],
        };
        data.ingest_mock_stack(&traces, sample_weight);
    }

    data
}

/// Verify hotspot ordering, export validity, and diagnostics on aggregated data.
fn verify_aggregation_results(data: &ProfileData, config: &HotspotConfig) {
    let hot_functions = data.hot_functions(config);
    let hot_lines = data.hot_lines(config);

    assert!(!hot_functions.is_empty(), "should detect hot functions");
    assert!(!hot_lines.is_empty(), "should detect hot lines");

    let mut by_self: Vec<_> = hot_functions.clone();
    by_self.sort_by(|a, b| {
        b.self_percentage
            .partial_cmp(&a.self_percentage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top_fn = &by_self[0];
    assert_eq!(
        top_fn.name,
        "hot_function",
        "hot_function should have the highest self-time. Got: {:?}",
        by_self
            .iter()
            .map(|f| (&f.name, f.self_percentage))
            .collect::<Vec<_>>()
    );
    assert!(
        top_fn.self_percentage > 50.0,
        "hot_function should have >50% self time (got {:.1}%)",
        top_fn.self_percentage
    );

    assert!(
        hot_functions.iter().any(|f| f.name == "warm_function"),
        "warm_function should appear as a hotspot"
    );
    assert!(
        hot_functions.iter().any(|f| f.name == "parse_config"),
        "parse_config (10%) should be above 2% function threshold"
    );
}

/// Test that the full aggregation pipeline handles real-world-like data.
/// Simulates 10,000 samples across 5 functions with realistic call stacks.
#[test]
fn e2e_aggregation_pipeline_with_realistic_data() {
    let data = build_realistic_profile_data();
    let config = HotspotConfig {
        line_threshold_pct: 1.0,
        function_threshold_pct: 2.0,
        max_diagnostics_per_file: 20,
    };

    assert_eq!(data.total_samples, 10_000, "should have 10K samples");
    verify_aggregation_results(&data, &config);

    let output_dir = std::env::temp_dir().join("basilisk_e2e_aggregation");
    std::fs::create_dir_all(&output_dir).unwrap();

    let export_result = export(
        &data,
        ExportFormat::Speedscope,
        "e2e-aggregation",
        12345,
        100.0,
        &output_dir,
    )
    .expect("export should succeed");

    assert!(export_result.path.exists());
    let json_str = std::fs::read_to_string(&export_result.path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.get("profiles").is_some());
    assert!(parsed.get("shared").is_some());

    let diagnostics = generate_diagnostics(&data, &config);
    assert!(!diagnostics.is_empty(), "should generate diagnostics");

    let total_diags: usize = diagnostics.values().map(Vec::len).sum();
    assert!(
        total_diags > 3,
        "should have diagnostics for multiple files (got {total_diags})"
    );
}

/// Helper to create a mock frame for the ingest function.
fn make_frame(name: &str, file: &str, line: i32) -> (String, String, i32) {
    (name.to_owned(), file.to_owned(), line)
}

/// Extension trait to ingest mock stacks into `ProfileData`.
trait ProfileDataExt {
    /// Ingest a single mock stack trace into the profile data.
    fn ingest_mock_stack(&mut self, frames: &[(String, String, i32)], weight: f64);
}

impl ProfileDataExt for ProfileData {
    fn ingest_mock_stack(&mut self, frames: &[(String, String, i32)], weight: f64) {
        use basilisk_lsp::profiler::aggregator::{FrameKey, FunctionStats, SpeedscopeFrame};

        self.total_samples += 1;

        for (idx, (name, file, line)) in frames.iter().enumerate() {
            *self
                .line_hits
                .entry(file.clone())
                .or_default()
                .entry(*line)
                .or_insert(0) += 1;

            let stats = self
                .function_stats
                .entry(file.clone())
                .or_default()
                .entry(name.clone())
                .or_insert_with(|| FunctionStats {
                    name: name.clone(),
                    file: file.clone(),
                    line: *line,
                    total_samples: 0,
                    self_samples: 0,
                });
            stats.total_samples += 1;
            if idx == frames.len() - 1 {
                stats.self_samples += 1;
            }

            let key = FrameKey {
                name: name.clone(),
                file: file.clone(),
                line: *line,
            };
            let frame_idx = self.frame_index.len();
            let _ = self.frame_index.entry(key).or_insert_with(|| {
                self.frames.push(SpeedscopeFrame {
                    name: name.clone(),
                    file: file.clone(),
                    line: *line,
                });
                frame_idx
            });
        }

        let stack_indices: Vec<usize> = frames
            .iter()
            .map(|(name, file, line)| {
                let key = FrameKey {
                    name: name.clone(),
                    file: file.clone(),
                    line: *line,
                };
                *self.frame_index.get(&key).unwrap_or(&0)
            })
            .collect();

        self.thread_stacks.entry(1).or_default().push(stack_indices);
        self.thread_weights.entry(1).or_default().push(weight);
    }
}
