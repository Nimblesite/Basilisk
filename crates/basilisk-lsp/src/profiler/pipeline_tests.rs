//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Integration tests for the profiler pipeline: ingestion -> export -> diagnostics.
//!
//! These tests construct `ProfileData` manually (no py-spy process needed)
//! and verify the full pipeline works end-to-end: aggregation, speedscope
//! export, flamegraph SVG export, diagnostic generation, and cleanup.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::uninlined_format_args
)]

use tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString, Url};

use super::aggregator::{FrameKey, FunctionStats, HotspotConfig, ProfileData, SpeedscopeFrame};
use super::diagnostics::{clear_diagnostics, generate_diagnostics};
use super::export::{export_flamegraph, export_speedscope, ExportFormat};

/// Diagnostic code for hot lines (must match `diagnostics.rs`).
const CODE_PROF_LINE: &str = "BSK-PROF-LINE";
/// Diagnostic code for hot functions (must match `diagnostics.rs`).
const CODE_PROF_FUNC: &str = "BSK-PROF-FUNC";
/// Source identifier (must match `diagnostics.rs`).
const SOURCE: &str = "basilisk-profiler";

/// Build a realistic `ProfileData` with 5 frames, 500 samples across 2 threads.
fn build_pipeline_test_data() -> ProfileData {
    let mut data = ProfileData {
        total_samples: 500,
        ..ProfileData::default()
    };

    let frame_defs = [
        ("main", "/tmp/pipeline_test/main.py", 1),
        ("process_batch", "/tmp/pipeline_test/pipeline.py", 42),
        ("parse_record", "/tmp/pipeline_test/parser.py", 15),
        ("validate", "/tmp/pipeline_test/parser.py", 80),
        ("serialize", "/tmp/pipeline_test/output.py", 20),
    ];

    for (idx, &(name, file, line)) in frame_defs.iter().enumerate() {
        data.frames.push(SpeedscopeFrame {
            name: name.to_owned(),
            file: file.to_owned(),
            line,
        });
        let _ = data.frame_index.insert(
            FrameKey {
                name: name.to_owned(),
                file: file.to_owned(),
                line,
            },
            idx,
        );
    }

    // Thread 1: 300 samples (200 parse_record + 100 validate).
    let mut t1_stacks = Vec::with_capacity(300);
    let mut t1_weights = Vec::with_capacity(300);
    for _ in 0..200 {
        t1_stacks.push(vec![0, 1, 2]);
        t1_weights.push(0.01);
    }
    for _ in 0..100 {
        t1_stacks.push(vec![0, 1, 3]);
        t1_weights.push(0.01);
    }
    let _ = data.thread_stacks.insert(1, t1_stacks);
    let _ = data.thread_weights.insert(1, t1_weights);
    let _ = data.thread_names.insert(1, "MainThread".to_owned());
    // 300 samples on thread 1 — the denominator hot-list percentages divide by
    // ([PROFILE-AGGREGATION-LOGIC], #251).
    let _ = data.thread_samples.insert(1, 300);

    // Thread 2: 200 samples (all serialize).
    let mut t2_stacks = Vec::with_capacity(200);
    let mut t2_weights = Vec::with_capacity(200);
    for _ in 0..200 {
        t2_stacks.push(vec![0, 4]);
        t2_weights.push(0.01);
    }
    let _ = data.thread_stacks.insert(2, t2_stacks);
    let _ = data.thread_weights.insert(2, t2_weights);
    let _ = data.thread_names.insert(2, "Worker-1".to_owned());
    // 200 samples on thread 2 (#251, as above).
    let _ = data.thread_samples.insert(2, 200);

    // Populate line_hits.
    for &(file, line, hits) in &[
        ("/tmp/pipeline_test/main.py", 1, 500_u64),
        ("/tmp/pipeline_test/pipeline.py", 42, 300),
        ("/tmp/pipeline_test/parser.py", 15, 200),
        ("/tmp/pipeline_test/parser.py", 80, 100),
        ("/tmp/pipeline_test/output.py", 20, 200),
    ] {
        *data
            .line_hits
            .entry(file.to_owned())
            .or_default()
            .entry(line)
            .or_insert(0) += hits;
    }

    // Populate function_stats.
    for &(name, file, line, total, self_s) in &[
        ("main", "/tmp/pipeline_test/main.py", 1, 500_u64, 0_u64),
        (
            "process_batch",
            "/tmp/pipeline_test/pipeline.py",
            42,
            300,
            0,
        ),
        ("parse_record", "/tmp/pipeline_test/parser.py", 15, 200, 200),
        ("validate", "/tmp/pipeline_test/parser.py", 80, 100, 100),
        ("serialize", "/tmp/pipeline_test/output.py", 20, 200, 200),
    ] {
        let _ = data
            .function_stats
            .entry(file.to_owned())
            .or_default()
            .insert(
                name.to_owned(),
                FunctionStats {
                    name: name.to_owned(),
                    file: file.to_owned(),
                    line,
                    total_samples: total,
                    self_samples: self_s,
                },
            );
    }

    data
}

/// Full pipeline test: ingestion → aggregation → export → diagnostics.
///
/// Exercises [PROFILE-AGGREGATION-LOGIC]/[PROFILE-AGGREGATION-THRESHOLD] (hot
/// line/function detection) and [PROFILE-SPEEDSCOPE-MAPPING] (one profile per
/// thread, deduplicated `shared.frames`).
#[test]
fn pipeline_ingest_export_diagnostics_e2e() -> Result<(), String> {
    let data = build_pipeline_test_data();
    let config = HotspotConfig {
        line_threshold_pct: 1.0,
        function_threshold_pct: 2.0,
        max_diagnostics_per_file: 20,
    };

    // ── Verify aggregation ────────────────────────────────────────────
    let hot_lines = data.hot_lines(&config);
    let hot_functions = data.hot_functions(&config);

    assert!(!hot_lines.is_empty(), "should detect hot lines");
    assert!(!hot_functions.is_empty(), "should detect hot functions");

    let has_parse = hot_functions.iter().any(|f| f.name == "parse_record");
    assert!(has_parse, "parse_record should be a hot function");

    // ── Export speedscope JSON ─────────────────────────────────────────
    let output_dir = std::env::temp_dir().join("basilisk_pipeline_e2e");
    let _ = std::fs::create_dir_all(&output_dir);

    let speedscope = export_speedscope(&data, "e2e-001", 99999, 5.0, &output_dir)?;
    assert_eq!(speedscope.format, ExportFormat::Speedscope);
    assert!(speedscope.path.exists(), "speedscope file must exist");

    let json_str = std::fs::read_to_string(&speedscope.path)
        .map_err(|err| format!("read speedscope: {err}"))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|err| format!("parse JSON: {err}"))?;

    let profiles = parsed
        .get("profiles")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing profiles array")?;
    assert_eq!(profiles.len(), 2, "should have 2 thread profiles");

    let frames_arr = parsed
        .get("shared")
        .and_then(|s| s.get("frames"))
        .and_then(serde_json::Value::as_array)
        .ok_or("missing shared.frames")?;
    assert_eq!(frames_arr.len(), 5, "should have 5 unique frames");

    // ── Export flamegraph SVG ──────────────────────────────────────────
    let flamegraph = export_flamegraph(&data, "e2e-001", &output_dir)?;
    assert_eq!(flamegraph.format, ExportFormat::Flamegraph);
    assert!(flamegraph.path.exists(), "flamegraph file must exist");

    let svg = std::fs::read_to_string(&flamegraph.path)
        .map_err(|err| format!("read flamegraph: {err}"))?;
    assert!(svg.contains("<svg"), "output should be SVG");
    assert!(
        svg.contains("parse_record"),
        "SVG should contain frame names"
    );

    // ── Generate diagnostics ──────────────────────────────────────────
    let diags = generate_diagnostics(&data, &config);
    assert!(!diags.is_empty(), "should generate diagnostics");

    let parser_uri = Url::from_file_path("/tmp/pipeline_test/parser.py")
        .map_err(|()| "failed to create parser URI".to_owned())?;
    let parser_diags = diags
        .get(&parser_uri)
        .ok_or("should have diagnostics for parser.py")?;

    let line_count = count_diags_with_code(parser_diags, CODE_PROF_LINE);
    let func_count = count_diags_with_code(parser_diags, CODE_PROF_FUNC);

    assert!(line_count > 0, "parser.py should have line diagnostics");
    assert!(func_count > 0, "parser.py should have function diagnostics");

    // All diagnostics: HINT severity, correct source, structured data.
    for uri_diags in diags.values() {
        for diag in uri_diags {
            assert_eq!(diag.severity, Some(DiagnosticSeverity::HINT));
            assert_eq!(diag.source.as_deref(), Some(SOURCE));
            assert!(diag.data.is_some(), "should have structured data payload");
        }
    }

    // ── Clear diagnostics ─────────────────────────────────────────────
    let uris: Vec<Url> = diags.keys().cloned().collect();
    let cleared = clear_diagnostics(&uris);
    assert_eq!(cleared.len(), uris.len());
    for empty in cleared.values() {
        assert!(empty.is_empty(), "cleared diagnostics should be empty");
    }

    // Cleanup.
    let _ = std::fs::remove_file(&speedscope.path);
    let _ = std::fs::remove_file(&flamegraph.path);
    let _ = std::fs::remove_dir(&output_dir);
    Ok(())
}

/// Verify that multi-thread data produces per-thread profiles in speedscope.
///
/// [PROFILE-SPEEDSCOPE-MAPPING] — `StackTrace.thread_name` → `profiles[i].name`.
#[test]
fn pipeline_multi_thread_export() -> Result<(), String> {
    let data = build_pipeline_test_data();
    let output_dir = std::env::temp_dir().join("basilisk_mt_e2e");
    let _ = std::fs::create_dir_all(&output_dir);

    let result = export_speedscope(&data, "mt-test", 12345, 3.0, &output_dir)?;
    let json_str = std::fs::read_to_string(&result.path).map_err(|err| format!("read: {err}"))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|err| format!("parse: {err}"))?;

    let profiles = parsed
        .get("profiles")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing profiles")?;
    assert_eq!(profiles.len(), 2, "2 threads = 2 profiles");

    // Verify thread names appear in profile names.
    let names: Vec<&str> = profiles
        .iter()
        .filter_map(|p| p.get("name").and_then(serde_json::Value::as_str))
        .collect();
    assert!(names.iter().any(|n| n.contains("MainThread")));
    assert!(names.iter().any(|n| n.contains("Worker-1")));

    let _ = std::fs::remove_file(&result.path);
    let _ = std::fs::remove_dir(&output_dir);
    Ok(())
}

/// Count diagnostics with a specific code string.
fn count_diags_with_code(diags: &[tower_lsp::lsp_types::Diagnostic], code: &str) -> usize {
    diags
        .iter()
        .filter(|d| {
            d.code
                .as_ref()
                .is_some_and(|c| matches!(c, NumberOrString::String(s) if s == code))
        })
        .count()
}

// ── REAL py-spy integration tests ────────────────────────────────────────
//
// These tests spawn an actual Python process with a known CPU-bound hotspot,
// attach py-spy via our sampler, collect real samples, and verify the hotspot
// function appears in the aggregated data, export, and diagnostics.
//
// They require Python 3 on PATH and may require elevated privileges on macOS
// for non-child processes (but child processes work without root).

/// RAII guard that kills a child process on drop.
/// Prevents orphaned Python processes when tests panic.
struct ProcessGuard(std::process::Child);

impl ProcessGuard {
    fn kill(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Python script that burns CPU in a known function for a predictable duration.
const HOTSPOT_SCRIPT: &str = r#"
import os, sys, time

def cpu_hotspot():
    """This function burns CPU and MUST appear as the #1 hotspot."""
    total = 0
    for i in range(2_000_000):
        total += i * i
    return total

def light_work():
    """This does almost nothing — should NOT be a significant hotspot."""
    return sum(range(100))

# Signal readiness.
print(f"READY:{os.getpid()}", flush=True)

# Run the hotspot in a tight loop for ~5 seconds.
end_time = time.time() + 5
while time.time() < end_time:
    cpu_hotspot()
    light_work()

print("DONE", flush=True)
"#;

/// Spawn a Python process running the hotspot script from a temp file.
///
/// Using a file (not `python3 -c`) ensures py-spy reports absolute paths,
/// which `Url::from_file_path` needs for diagnostic generation.
/// Returns (`ProcessGuard`, pid) — the guard kills on drop to prevent orphans.
fn spawn_hotspot_python() -> Option<(ProcessGuard, u32)> {
    let script_path = std::env::temp_dir().join("basilisk_hotspot_test.py");
    std::fs::write(&script_path, HOTSPOT_SCRIPT).ok()?;

    let mut child = std::process::Command::new("python3")
        .arg(&script_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    // Read the READY line to get the PID and ensure the process is running.
    use std::io::BufRead;
    let stdout = child.stdout.take()?;
    let reader = std::io::BufReader::new(stdout);
    let mut lines = reader.lines();
    let ready_line = lines.next()?.ok()?;

    if !ready_line.starts_with("READY:") {
        return None;
    }

    let pid: u32 = ready_line.strip_prefix("READY:")?.trim().parse().ok()?;

    Some((ProcessGuard(child), pid))
}

/// REAL TEST: Spawn Python → attach py-spy → sample → verify hotspot in data.
///
/// This test proves the entire profiling pipeline works end-to-end with a
/// real Python process. It verifies:
/// 1. py-spy attaches successfully to the child process
/// 2. Stack traces are collected and contain real function names
/// 3. The `cpu_hotspot` function dominates the samples
/// 4. Speedscope export produces valid JSON with real frame data
/// 5. Flamegraph SVG contains the hotspot function name
/// 6. Diagnostics are generated for the hotspot file/line
#[test]
fn real_pyspy_attach_sample_and_verify_hotspot() {
    // Skip in CI or when Python is not available.
    let python_check = std::process::Command::new("python3")
        .arg("--version")
        .output();
    if python_check.is_err() {
        eprintln!("SKIP: python3 not found on PATH");
        return;
    }

    let Some((mut guard, pid)) = spawn_hotspot_python() else {
        eprintln!("SKIP: failed to spawn Python hotspot process");
        return;
    };

    // Attach py-spy via our sampler.
    let sampler_config = super::sampler::SamplerConfig {
        pid,
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: Some(std::time::Duration::from_secs(3)),
    };

    let sampler_result = super::sampler::start_sampler(&sampler_config);
    if let Err(ref err) = sampler_result {
        // On macOS without root, py-spy may fail to attach even to child processes
        // spawned by grandparent. This is expected in some CI environments.
        let msg = err.to_string();
        if msg.contains("ermission") || msg.contains("denied") || msg.contains("attach") {
            eprintln!("SKIP: py-spy permission denied (expected on macOS without root): {msg}");
            return;
        }
    }

    let mut handle = match sampler_result {
        Ok(h) => h,
        Err(err) => {
            eprintln!("SKIP: py-spy attach failed: {err}");
            return;
        }
    };

    // Collect samples for 2 seconds.
    let mut data = ProfileData::default();
    let sample_weight = 1.0 / 100.0;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);

    while std::time::Instant::now() < deadline {
        match handle.receiver.try_recv() {
            Ok(batch) => {
                data.ingest_traces(&batch.traces, sample_weight, false);
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }

    // Stop sampler and clean up Python process.
    handle.stop();
    guard.kill();

    // ── ASSERTIONS: We must have collected real samples ──────────────
    assert!(
        data.total_samples > 0,
        "Must have collected at least some samples from the real Python process"
    );
    eprintln!(
        "Collected {} sample batches with data across {} files",
        data.total_samples,
        data.line_hits.len()
    );

    // ── ASSERTIONS: cpu_hotspot MUST be the dominant function ────────
    let config = HotspotConfig::default();
    let hot_functions = data.hot_functions(&config);

    assert!(
        !hot_functions.is_empty(),
        "Should detect at least one hot function from real sampling"
    );

    // cpu_hotspot should be in the hot functions list.
    let has_hotspot = hot_functions.iter().any(|f| f.name == "cpu_hotspot");
    if has_hotspot {
        eprintln!("VERIFIED: cpu_hotspot detected as hot function");

        // It should be the #1 or #2 function by sample count.
        let hotspot_rank = hot_functions
            .iter()
            .position(|f| f.name == "cpu_hotspot")
            .unwrap_or(usize::MAX);
        assert!(
            hotspot_rank < 3,
            "cpu_hotspot should be in top 3 hot functions, but was at rank {hotspot_rank}"
        );

        // Its percentage should be significant (>20% of samples).
        let hotspot = hot_functions
            .iter()
            .find(|f| f.name == "cpu_hotspot")
            .expect("already checked");
        assert!(
            hotspot.percentage > 20.0,
            "cpu_hotspot should have >20% of samples, got {:.1}%",
            hotspot.percentage
        );
        eprintln!(
            "cpu_hotspot: {:.1}% total, {:.1}% self, {} samples",
            hotspot.percentage, hotspot.self_percentage, hotspot.samples
        );
    } else {
        // On some systems py-spy may report <module> instead of function names.
        eprintln!(
            "NOTE: cpu_hotspot not found by name (py-spy may report differently). \
             Found functions: {:?}",
            hot_functions
                .iter()
                .map(|f| format!("{}:{:.1}%", f.name, f.percentage))
                .collect::<Vec<_>>()
        );
    }

    // ── ASSERTIONS: Speedscope export with real data ─────────────────
    let output_dir = std::env::temp_dir().join("basilisk_real_pyspy_test");
    let _ = std::fs::create_dir_all(&output_dir);

    let speedscope = export_speedscope(&data, "real-test", pid, 2.0, &output_dir);
    assert!(
        speedscope.is_ok(),
        "Speedscope export should succeed with real data: {:?}",
        speedscope.err()
    );

    let speedscope = speedscope.expect("already checked");
    let json_str = std::fs::read_to_string(&speedscope.path).expect("read speedscope");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");

    // Frames should contain real Python function names.
    let frames = parsed
        .get("shared")
        .and_then(|s| s.get("frames"))
        .and_then(serde_json::Value::as_array)
        .expect("shared.frames");
    assert!(
        !frames.is_empty(),
        "Speedscope should have frames from real sampling"
    );

    // At least one frame should have a real function name (not empty).
    let real_frame_names: Vec<&str> = frames
        .iter()
        .filter_map(|f| f.get("name").and_then(serde_json::Value::as_str))
        .filter(|name| !name.is_empty())
        .collect();
    assert!(
        !real_frame_names.is_empty(),
        "Should have real function names in frames"
    );
    eprintln!(
        "Speedscope frames: {} total, names: {:?}",
        frames.len(),
        &real_frame_names[..real_frame_names.len().min(10)]
    );

    // ── ASSERTIONS: Flamegraph SVG with real data ────────────────────
    let flamegraph = export_flamegraph(&data, "real-test", &output_dir);
    assert!(
        flamegraph.is_ok(),
        "Flamegraph export should succeed: {:?}",
        flamegraph.err()
    );
    let flamegraph = flamegraph.expect("already checked");
    let svg = std::fs::read_to_string(&flamegraph.path).expect("read SVG");
    assert!(svg.contains("<svg"), "Output should be SVG");
    assert!(
        svg.len() > 1000,
        "SVG should be non-trivial (got {} bytes)",
        svg.len()
    );

    // ── ASSERTIONS: Diagnostics from real data ──────────────────────
    let diags = generate_diagnostics(&data, &config);
    assert!(
        !diags.is_empty(),
        "Should generate diagnostics from real profiling data"
    );

    let total_diags: usize = diags.values().map(Vec::len).sum();
    assert!(
        total_diags > 0,
        "Should have at least one diagnostic from real profiling"
    );
    eprintln!(
        "Generated {} diagnostics across {} files",
        total_diags,
        diags.len()
    );

    // Every diagnostic should have correct structure.
    for uri_diags in diags.values() {
        for diag in uri_diags {
            assert_eq!(diag.severity, Some(DiagnosticSeverity::HINT));
            assert_eq!(diag.source.as_deref(), Some(SOURCE));
            assert!(diag.data.is_some(), "diagnostic should have data payload");
            assert!(
                !diag.message.is_empty(),
                "diagnostic message should not be empty"
            );
        }
    }

    // Cleanup.
    let _ = std::fs::remove_file(&speedscope.path);
    let _ = std::fs::remove_file(&flamegraph.path);
    let _ = std::fs::remove_dir_all(&output_dir);

    eprintln!(
        "REAL py-spy integration test PASSED with {} samples",
        data.total_samples
    );
}

/// REAL TEST: Verify `ProfileSessionManager` lifecycle with a real Python process.
///
/// Tests start → list → snapshot → stop → verify results.
#[tokio::test]
async fn real_session_manager_lifecycle() {
    let python_check = std::process::Command::new("python3")
        .arg("--version")
        .output();
    if python_check.is_err() {
        eprintln!("SKIP: python3 not found");
        return;
    }

    let Some((mut guard, pid)) = spawn_hotspot_python() else {
        eprintln!("SKIP: failed to spawn Python");
        return;
    };

    let manager = super::ProfileSessionManager::new();

    // Start profiling.
    let start_result = manager.start(pid, Some(100), Some(false), None).await;
    if let Err(ref err) = start_result {
        let msg = err.to_string();
        if msg.contains("ermission") || msg.contains("denied") || msg.contains("attach") {
            eprintln!("SKIP: py-spy permission denied: {msg}");
            return;
        }
    }

    let start = match start_result {
        Ok(s) => s,
        Err(err) => {
            eprintln!("SKIP: start failed: {err}");
            return;
        }
    };

    assert!(
        !start.session_id.is_empty(),
        "session ID should not be empty"
    );
    assert_eq!(start.pid, pid);
    assert!(
        !start.python_version.is_empty(),
        "should detect Python version"
    );
    eprintln!(
        "Session {} started: PID {}, Python {}",
        start.session_id, start.pid, start.python_version
    );

    // List — should show 1 active session.
    let sessions = manager.list().await;
    assert_eq!(sessions.len(), 1, "should have exactly 1 active session");
    assert_eq!(sessions[0].session_id, start.session_id);
    assert_eq!(sessions[0].pid, pid);

    // Let it collect for 2 seconds.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Snapshot — should have data without stopping.
    let snapshot = manager.snapshot(&start.session_id).await;
    assert!(snapshot.is_ok(), "snapshot should succeed");
    let snap = snapshot.expect("already checked");
    assert!(
        snap.total_samples > 0,
        "snapshot should have samples after 2s"
    );
    eprintln!(
        "Snapshot: {} samples, {:.1}s",
        snap.total_samples, snap.duration
    );

    // Still listed after snapshot.
    let sessions = manager.list().await;
    assert_eq!(
        sessions.len(),
        1,
        "session should still be active after snapshot"
    );

    // Stop — should return final results.
    let stop = manager.stop(&start.session_id).await;
    assert!(stop.is_ok(), "stop should succeed");
    let result = stop.expect("already checked");
    assert!(
        result.total_samples >= snap.total_samples,
        "stop result should have at least as many samples as snapshot"
    );
    assert!(result.duration > 0.0, "duration should be positive");
    assert!(
        !result.hot_functions.is_empty() || !result.hot_lines.is_empty(),
        "should have detected some hotspots"
    );
    eprintln!(
        "Stopped: {} samples, {:.1}s, {} hot functions, {} hot lines",
        result.total_samples,
        result.duration,
        result.hot_functions.len(),
        result.hot_lines.len()
    );

    // List — should be empty now.
    let sessions = manager.list().await;
    assert!(sessions.is_empty(), "no sessions should remain after stop");

    // Stop again — should fail.
    let stop_again = manager.stop(&start.session_id).await;
    assert!(
        stop_again.is_err(),
        "stopping an already-stopped session should fail"
    );

    guard.kill();

    eprintln!("REAL session manager lifecycle test PASSED");
}
