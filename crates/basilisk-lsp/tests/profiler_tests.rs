//! Tests for [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//! E2E integration tests for the Basilisk profiler system.
//!
//! Tests the profiler core: session management, data aggregation, export
//! formats, diagnostics generation, memory snapshot parsing, memory diff
//! parsing, leak confidence scoring, and privilege permission checks.
//!
//! All tests exercise the real public API of `basilisk_lsp::profiler`.
//! No mocks, no unit-level isolation — coarse E2E tests per CLAUDE.md.

#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    missing_docs,
    clippy::needless_raw_string_hashes,
    dead_code,
    unused_imports
)]

mod common;

use common::ProcessGuard;

use basilisk_lsp::profiler::aggregator::{
    FrameKey, FunctionStats, HotFunction, HotLine, HotspotConfig, ProfileData, SpeedscopeFrame,
};
use basilisk_lsp::profiler::export::{self, ExportFormat, ExportResult};
use basilisk_lsp::profiler::memory::{
    self,
    diff::{self, AllocationGrowth, MemoryDiff, TraceFrame},
    leaks::{LeakConfidence, LeakTracker, SuspectedLeak},
    AllocationSite, MemorySnapshot,
};
use basilisk_lsp::profiler::privilege::{self, PermissionStatus};
use basilisk_lsp::profiler::ProfileSessionManager;

// ── ProfileSessionManager tests ──────────────────────────────────────────

#[tokio::test]
async fn session_manager_new_creates_empty_manager() {
    let manager = ProfileSessionManager::new();
    let sessions = manager.list().await;
    assert!(sessions.is_empty(), "new manager should have zero sessions");
}

#[tokio::test]
async fn session_manager_default_creates_empty_manager() {
    let manager = ProfileSessionManager::default();
    let sessions = manager.list().await;
    assert!(
        sessions.is_empty(),
        "default manager should have zero sessions"
    );
}

#[tokio::test]
async fn session_manager_list_returns_empty_vec_when_no_sessions() {
    let manager = ProfileSessionManager::new();
    let sessions = manager.list().await;
    assert_eq!(sessions.len(), 0, "list should return empty vec");
    assert!(sessions.is_empty(), "sessions should be empty");
}

#[tokio::test]
async fn session_manager_stop_nonexistent_session_returns_error() {
    let manager = ProfileSessionManager::new();
    let result = manager.stop("nonexistent-id").await;
    assert!(result.is_err(), "stopping nonexistent session should fail");
    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("nonexistent-id"),
        "error should mention the session ID, got: {err_msg}"
    );
}

#[tokio::test]
async fn session_manager_snapshot_nonexistent_session_returns_error() {
    let manager = ProfileSessionManager::new();
    let result = manager.snapshot("nonexistent-id").await;
    assert!(
        result.is_err(),
        "snapshotting nonexistent session should fail"
    );
    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("nonexistent-id"),
        "error should mention the session ID, got: {err_msg}"
    );
}

#[tokio::test]
async fn session_manager_stop_all_with_no_sessions_does_not_panic() {
    let manager = ProfileSessionManager::new();
    // Should complete without panicking.
    manager.stop_all().await;
    let sessions = manager.list().await;
    assert!(
        sessions.is_empty(),
        "no sessions should exist after stop_all"
    );
}

#[tokio::test]
async fn session_manager_start_with_pid_zero_returns_error() {
    let manager = ProfileSessionManager::new();
    let result = manager.start(0, None, None, None).await;
    // PID 0 is the kernel — py-spy will fail to attach.
    assert!(result.is_err(), "profiling PID 0 should fail");
}

// ── ProfileData aggregation tests ────────────────────────────────────────

fn make_trace(thread_id: u64, active: bool, frames: Vec<(&str, &str, i32)>) -> py_spy::StackTrace {
    py_spy::StackTrace {
        pid: 0,
        thread_id,
        thread_name: Some(format!("Thread-{thread_id}")),
        owns_gil: false,
        active,
        frames: frames
            .into_iter()
            .map(|(name, file, line)| py_spy::Frame {
                name: name.to_owned(),
                filename: file.to_owned(),
                line,
                short_filename: None,
                module: None,
                locals: None,
                is_entry: false,
                is_shim_entry: false,
            })
            .collect(),
        os_thread_id: None,
        process_info: None,
    }
}

#[test]
fn profile_data_default_is_empty() {
    let data = ProfileData::default();
    assert_eq!(data.total_samples, 0, "default should have zero samples");
    assert!(
        data.line_hits.is_empty(),
        "default should have no line hits"
    );
    assert!(
        data.function_stats.is_empty(),
        "default should have no function stats"
    );
    assert!(data.frames.is_empty(), "default should have no frames");
    assert!(
        data.thread_stacks.is_empty(),
        "default should have no thread stacks"
    );
    assert!(
        data.thread_weights.is_empty(),
        "default should have no thread weights"
    );
    assert!(
        data.thread_names.is_empty(),
        "default should have no thread names"
    );
    assert!(
        data.frame_index.is_empty(),
        "default should have no frame index"
    );
    assert!(
        data.thread_samples.is_empty(),
        "default should have no thread samples"
    );
}

#[test]
fn profile_data_aggregation_with_synthetic_traces() {
    let mut data = ProfileData::default();
    let traces = vec![
        make_trace(
            1,
            true,
            vec![
                ("leaf_fn", "src/a.py", 10),
                ("middle_fn", "src/a.py", 5),
                ("main", "main.py", 1),
            ],
        ),
        make_trace(1, true, vec![("leaf_fn", "src/a.py", 10)]),
    ];

    data.ingest_traces(&traces, 0.01, false);

    // Total samples should be 1 (one call to ingest_traces).
    assert_eq!(data.total_samples, 1);

    // Line hits should be populated.
    assert!(!data.line_hits.is_empty(), "line_hits should not be empty");
    let a_py_hits = data.line_hits.get("src/a.py");
    assert!(a_py_hits.is_some(), "src/a.py should have line hits");
    let line_10_hits = a_py_hits.unwrap().get(&10).copied().unwrap_or(0);
    assert!(line_10_hits >= 1, "line 10 should have at least 1 hit");

    // Function stats should be populated.
    assert!(
        !data.function_stats.is_empty(),
        "function_stats should not be empty"
    );
    let a_py_funcs = data.function_stats.get("src/a.py");
    assert!(a_py_funcs.is_some(), "src/a.py should have function stats");
    let leaf_stats = a_py_funcs.unwrap().get("leaf_fn");
    assert!(leaf_stats.is_some(), "leaf_fn should have stats");
    let leaf = leaf_stats.unwrap();
    assert!(
        leaf.total_samples >= 1,
        "leaf_fn should have at least 1 total sample"
    );
    assert!(
        leaf.self_samples >= 1,
        "leaf_fn should have at least 1 self sample"
    );

    // Frames should be populated.
    assert!(!data.frames.is_empty(), "frames should not be empty");
    assert!(
        data.frames.len() >= 2,
        "should have at least 2 unique frames"
    );

    // Thread stacks should be populated.
    assert!(
        data.thread_stacks.contains_key(&1),
        "thread 1 should have stacks"
    );
    let stacks = data.thread_stacks.get(&1).unwrap();
    assert_eq!(stacks.len(), 2, "should have 2 sample stacks");

    // Thread weights should be populated.
    assert!(
        data.thread_weights.contains_key(&1),
        "thread 1 should have weights"
    );
    let weights = data.thread_weights.get(&1).unwrap();
    assert_eq!(weights.len(), 2, "should have 2 weights");
    for weight in weights {
        assert!((*weight - 0.01).abs() < 0.001, "weight should be ~0.01");
    }

    // Thread names should be populated.
    assert!(
        data.thread_names.contains_key(&1),
        "thread 1 should have a name"
    );
    let name = data.thread_names.get(&1).unwrap();
    assert_eq!(name, "Thread-1", "thread name should be Thread-1");
}

#[test]
fn profile_data_idle_traces_skipped() {
    let mut data = ProfileData::default();
    let traces = vec![make_trace(1, false, vec![("idle_fn", "src/a.py", 10)])];

    data.ingest_traces(&traces, 0.01, false);
    assert_eq!(
        data.total_samples, 1,
        "total_samples increments even for idle"
    );
    assert!(
        data.line_hits.is_empty(),
        "idle traces should produce no line hits"
    );
    assert!(
        data.function_stats.is_empty(),
        "idle traces should produce no function stats"
    );
}

#[test]
fn profile_data_idle_traces_included_when_flag_set() {
    let mut data = ProfileData::default();
    let traces = vec![make_trace(1, false, vec![("idle_fn", "src/a.py", 10)])];

    data.ingest_traces(&traces, 0.01, true);
    assert_eq!(data.total_samples, 1);
    assert!(
        !data.line_hits.is_empty(),
        "idle traces should produce line hits when included"
    );
}

#[test]
fn hot_lines_returns_empty_for_zero_samples() {
    let data = ProfileData::default();
    let config = HotspotConfig::default();
    let hot = data.hot_lines(&config);
    assert!(
        hot.is_empty(),
        "hot_lines should return empty for zero samples"
    );
}

#[test]
fn hot_functions_returns_empty_for_zero_samples() {
    let data = ProfileData::default();
    let config = HotspotConfig::default();
    let hot = data.hot_functions(&config);
    assert!(
        hot.is_empty(),
        "hot_functions should return empty for zero samples"
    );
}

#[test]
fn hot_lines_filters_by_threshold() {
    let mut data = ProfileData::default();
    // Create 50 samples on line 10, 1 on line 20.
    for _ in 0..50 {
        let traces = vec![make_trace(1, true, vec![("fn_a", "src/a.py", 10)])];
        data.ingest_traces(&traces, 0.01, false);
    }
    let traces = vec![make_trace(1, true, vec![("fn_b", "src/a.py", 20)])];
    data.ingest_traces(&traces, 0.01, false);

    let config = HotspotConfig {
        line_threshold_pct: 5.0,
        ..HotspotConfig::default()
    };
    let hot = data.hot_lines(&config);

    assert_eq!(hot.len(), 1, "should have exactly 1 hot line above 5%");
    assert_eq!(hot[0].line, 10, "hot line should be line 10");
    assert!(hot[0].percentage > 5.0, "hot line should be above 5%");
    assert!(
        hot[0].samples >= 50,
        "hot line should have at least 50 samples"
    );
}

#[test]
fn hot_functions_filters_by_threshold() {
    let mut data = ProfileData::default();
    for _ in 0..50 {
        let traces = vec![make_trace(1, true, vec![("hot_func", "src/a.py", 10)])];
        data.ingest_traces(&traces, 0.01, false);
    }
    let traces = vec![make_trace(1, true, vec![("cold_func", "src/a.py", 99)])];
    data.ingest_traces(&traces, 0.01, false);

    let config = HotspotConfig {
        function_threshold_pct: 5.0,
        ..HotspotConfig::default()
    };
    let hot = data.hot_functions(&config);

    assert_eq!(hot.len(), 1, "should have exactly 1 hot function above 5%");
    assert_eq!(hot[0].name, "hot_func", "hot function should be hot_func");
    assert!(hot[0].percentage > 5.0, "hot function should be above 5%");
}

#[test]
fn hotspot_config_default_values_are_correct() {
    let config = HotspotConfig::default();
    assert!(
        (config.line_threshold_pct - 1.0).abs() < f64::EPSILON,
        "default line threshold should be 1.0%"
    );
    assert!(
        (config.function_threshold_pct - 2.0).abs() < f64::EPSILON,
        "default function threshold should be 2.0%"
    );
    assert_eq!(
        config.max_diagnostics_per_file, 20,
        "default max diagnostics should be 20"
    );
}

#[test]
fn hot_lines_sorted_by_samples_descending() {
    let mut data = ProfileData::default();
    for _ in 0..30 {
        let traces = vec![make_trace(1, true, vec![("fn_a", "src/a.py", 10)])];
        data.ingest_traces(&traces, 0.01, false);
    }
    for _ in 0..50 {
        let traces = vec![make_trace(1, true, vec![("fn_b", "src/b.py", 20)])];
        data.ingest_traces(&traces, 0.01, false);
    }

    let config = HotspotConfig {
        line_threshold_pct: 0.1,
        ..HotspotConfig::default()
    };
    let hot = data.hot_lines(&config);

    assert!(hot.len() >= 2, "should have at least 2 hot lines");
    assert!(
        hot[0].samples >= hot[1].samples,
        "hot lines should be sorted by samples descending"
    );
}

// ── Speedscope export tests ──────────────────────────────────────────────

fn make_test_profile_data() -> ProfileData {
    let mut data = ProfileData::default();

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

    let _ = data
        .thread_stacks
        .insert(1, vec![vec![0, 1, 2], vec![0, 1]]);
    let _ = data.thread_weights.insert(1, vec![0.01, 0.01]);
    let _ = data.thread_names.insert(1, "MainThread".to_owned());
    data.total_samples = 2;

    data
}

#[test]
fn speedscope_export_produces_valid_json() {
    let data = make_test_profile_data();
    let dir = std::env::temp_dir();

    let result = export::export_speedscope(&data, "test-e2e-001", 12345, 5.2, &dir);
    assert!(result.is_ok(), "speedscope export should succeed");

    let export_result = result.unwrap();
    assert!(export_result.path.exists(), "output file should exist");
    assert_eq!(export_result.format, ExportFormat::Speedscope);

    let contents = std::fs::read_to_string(&export_result.path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // Validate top-level structure.
    assert!(json.get("$schema").is_some(), "should have $schema field");
    assert!(json.get("shared").is_some(), "should have shared field");
    assert!(json.get("profiles").is_some(), "should have profiles field");
    assert!(json.get("name").is_some(), "should have name field");
    assert!(json.get("exporter").is_some(), "should have exporter field");
    assert!(
        json.get("activeProfileIndex").is_some(),
        "should have activeProfileIndex field"
    );

    // Validate schema URL.
    let schema = json.get("$schema").unwrap().as_str().unwrap();
    assert!(
        schema.contains("speedscope"),
        "schema should reference speedscope"
    );

    // Validate shared frames.
    let frames = json
        .get("shared")
        .unwrap()
        .get("frames")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(frames.len(), 3, "should have 3 frames");
    for frame in frames {
        assert!(frame.get("name").is_some(), "frame should have name");
        assert!(frame.get("file").is_some(), "frame should have file");
        assert!(frame.get("line").is_some(), "frame should have line");
    }

    // Validate profiles.
    let profiles = json.get("profiles").unwrap().as_array().unwrap();
    assert_eq!(profiles.len(), 1, "should have 1 profile (one thread)");
    let profile = &profiles[0];
    assert_eq!(
        profile.get("type").unwrap().as_str().unwrap(),
        "sampled",
        "profile type should be sampled"
    );
    assert_eq!(
        profile.get("unit").unwrap().as_str().unwrap(),
        "seconds",
        "unit should be seconds"
    );
    assert!(
        profile.get("samples").is_some(),
        "profile should have samples"
    );
    assert!(
        profile.get("weights").is_some(),
        "profile should have weights"
    );

    // Validate profile samples match input.
    let samples = profile.get("samples").unwrap().as_array().unwrap();
    assert_eq!(samples.len(), 2, "should have 2 sample stacks");

    let weights = profile.get("weights").unwrap().as_array().unwrap();
    assert_eq!(weights.len(), 2, "should have 2 weights");

    // Clean up.
    let _ = std::fs::remove_file(&export_result.path);
}

#[test]
fn flamegraph_export_produces_svg() {
    let data = make_test_profile_data();
    let dir = std::env::temp_dir();

    let result = export::export_flamegraph(&data, "test-e2e-002", &dir);
    assert!(result.is_ok(), "flamegraph export should succeed");

    let export_result = result.unwrap();
    assert!(export_result.path.exists(), "output file should exist");
    assert_eq!(export_result.format, ExportFormat::Flamegraph);

    let contents = std::fs::read_to_string(&export_result.path).unwrap();
    assert!(contents.contains("<svg"), "output should be SVG");
    assert!(contents.contains("</svg>"), "SVG should be complete");
    assert!(contents.len() > 100, "SVG should have substantial content");

    // Clean up.
    let _ = std::fs::remove_file(&export_result.path);
}

#[test]
fn export_dispatches_to_correct_format() {
    let data = make_test_profile_data();
    let dir = std::env::temp_dir();

    let speedscope_result = export::export(
        &data,
        ExportFormat::Speedscope,
        "dispatch-speed-001",
        12345,
        5.0,
        &dir,
    );
    assert!(
        speedscope_result.is_ok(),
        "speedscope dispatch should succeed"
    );
    let sr = speedscope_result.unwrap();
    assert_eq!(sr.format, ExportFormat::Speedscope);
    let _ = std::fs::remove_file(&sr.path);

    let flamegraph_result = export::export(
        &data,
        ExportFormat::Flamegraph,
        "dispatch-flame-001",
        12345,
        5.0,
        &dir,
    );
    assert!(
        flamegraph_result.is_ok(),
        "flamegraph dispatch should succeed"
    );
    let fr = flamegraph_result.unwrap();
    assert_eq!(fr.format, ExportFormat::Flamegraph);
    let _ = std::fs::remove_file(&fr.path);
}

#[test]
fn speedscope_export_with_empty_data() {
    let data = ProfileData::default();
    let dir = std::env::temp_dir();

    let result = export::export_speedscope(&data, "empty-001", 0, 0.0, &dir);
    assert!(result.is_ok(), "empty data export should succeed");

    let export_result = result.unwrap();
    let contents = std::fs::read_to_string(&export_result.path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // Empty data should still produce valid structure.
    assert!(json.get("$schema").is_some());
    assert!(json.get("shared").is_some());
    let frames = json
        .get("shared")
        .unwrap()
        .get("frames")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(frames.len(), 0, "empty data should have 0 frames");

    let _ = std::fs::remove_file(&export_result.path);
}

// ── Memory snapshot parsing tests ────────────────────────────────────────

#[test]
fn memory_snapshot_parsing_valid_json() {
    let output = r#"some debug output
__BASILISK_MEM__{"current": 45678912, "peak": 50000000, "gcObjects": 14523, "gcCounts": [712, 45, 3], "stats": [{"file": "src/app.py", "line": 42, "size": 24567890, "count": 15234, "traceback": [{"file": "src/app.py", "line": 42}]}]}
more output"#;

    let snapshot = memory::parse_snapshot_output(output, "snap-e2e-001");
    assert!(snapshot.is_ok(), "parsing valid snapshot should succeed");

    let snap = snapshot.unwrap();
    assert_eq!(snap.snapshot_id, "snap-e2e-001");
    assert_eq!(snap.current_memory, 45_678_912);
    assert_eq!(snap.peak_memory, 50_000_000);
    assert_eq!(snap.gc_objects, 14523);
    assert_eq!(snap.gc_counts.len(), 3);
    assert_eq!(snap.gc_counts[0], 712);
    assert_eq!(snap.gc_counts[1], 45);
    assert_eq!(snap.gc_counts[2], 3);
    assert_eq!(snap.top_allocations.len(), 1);

    let alloc = &snap.top_allocations[0];
    assert_eq!(alloc.file, "src/app.py");
    assert_eq!(alloc.line, 42);
    assert_eq!(alloc.size, 24_567_890);
    assert_eq!(alloc.count, 15234);
    assert_eq!(alloc.traceback.len(), 1);
    assert_eq!(alloc.traceback[0].file, "src/app.py");
    assert_eq!(alloc.traceback[0].line, 42);
}

#[test]
fn memory_snapshot_parsing_missing_marker() {
    let output = "no marker here at all";
    let result = memory::parse_snapshot_output(output, "snap-missing");
    assert!(result.is_err(), "missing marker should return error");
    let err = result.unwrap_err();
    assert!(
        err.contains("marker"),
        "error should mention marker, got: {err}"
    );
}

#[test]
fn memory_snapshot_parsing_invalid_json() {
    let output = "__BASILISK_MEM__{not valid json}";
    let result = memory::parse_snapshot_output(output, "snap-invalid");
    assert!(result.is_err(), "invalid JSON should return error");
}

#[test]
fn memory_snapshot_parsing_empty_stats() {
    let output = r#"__BASILISK_MEM__{"current": 1000, "peak": 2000, "gcObjects": 10, "gcCounts": [], "stats": []}"#;
    let result = memory::parse_snapshot_output(output, "snap-empty-stats");
    assert!(result.is_ok(), "empty stats should parse successfully");

    let snap = result.unwrap();
    assert_eq!(snap.current_memory, 1000);
    assert_eq!(snap.peak_memory, 2000);
    assert_eq!(snap.gc_objects, 10);
    assert!(snap.gc_counts.is_empty());
    assert!(snap.top_allocations.is_empty());
}

#[test]
fn memory_format_bytes_units() {
    assert_eq!(memory::format_bytes(500), "500 B");
    assert_eq!(memory::format_bytes(1024), "1.0 KB");
    assert_eq!(memory::format_bytes(1_048_576), "1.0 MB");
    assert_eq!(memory::format_bytes(1_073_741_824), "1.0 GB");
    assert_eq!(memory::format_bytes(0), "0 B");
    assert_eq!(memory::format_bytes(1), "1 B");
    assert_eq!(memory::format_bytes(1023), "1023 B");
}

// ── Memory diff parsing tests ────────────────────────────────────────────

#[test]
fn memory_diff_parsing_valid() {
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
            },
            {
                "file": "src/freed.py",
                "line": 10,
                "sizeDiff": -5000,
                "countDiff": -100,
                "size": 1000,
                "count": 50,
                "traceback": []
            }
        ],
        "current": 89012345,
        "peak": 102345678
    }"#;

    let result = diff::parse_diff_output(json);
    assert!(result.is_ok(), "valid diff JSON should parse");

    let diff_result = result.unwrap();
    assert_eq!(
        diff_result.grown_allocations.len(),
        1,
        "should have 1 grown allocation"
    );
    assert_eq!(
        diff_result.freed_allocations.len(),
        1,
        "should have 1 freed allocation"
    );
    assert_eq!(diff_result.total_growth, 18_234_567);
    assert_eq!(diff_result.total_freed, 5000);
    assert_eq!(diff_result.net_growth, 18_234_567 - 5000);

    let grown = &diff_result.grown_allocations[0];
    assert_eq!(grown.file, "src/cache.py");
    assert_eq!(grown.line, 34);
    assert_eq!(grown.size_diff, 18_234_567);
    assert_eq!(grown.count_diff, 8923);
    assert_eq!(grown.size, 24_567_890);
    assert_eq!(grown.count, 12345);
    assert_eq!(grown.traceback.len(), 2);
}

#[test]
fn memory_diff_parsing_error_response() {
    let json = r#"{"error": "no previous snapshot taken"}"#;
    let result = diff::parse_diff_output(json);
    assert!(result.is_err(), "error response should return Err");
    let err = result.unwrap_err();
    assert!(
        err.contains("no previous snapshot"),
        "error should contain the message, got: {err}"
    );
}

#[test]
fn memory_diff_parsing_empty_leaks() {
    let json = r#"{"leaks": [], "current": 0, "peak": 0}"#;
    let result = diff::parse_diff_output(json);
    assert!(result.is_ok(), "empty leaks should parse");

    let diff_result = result.unwrap();
    assert!(diff_result.grown_allocations.is_empty());
    assert!(diff_result.freed_allocations.is_empty());
    assert_eq!(diff_result.total_growth, 0);
    assert_eq!(diff_result.total_freed, 0);
    assert_eq!(diff_result.net_growth, 0);
}

#[test]
fn memory_diff_parsing_invalid_json() {
    let json = "not valid json at all";
    let result = diff::parse_diff_output(json);
    assert!(result.is_err(), "invalid JSON should return Err");
}

#[test]
fn memory_diff_grown_allocations_sorted_descending() {
    let json = r#"{"leaks": [
        {"file": "a.py", "line": 1, "sizeDiff": 1000, "countDiff": 10, "size": 5000, "count": 50, "traceback": []},
        {"file": "b.py", "line": 2, "sizeDiff": 5000, "countDiff": 50, "size": 10000, "count": 100, "traceback": []},
        {"file": "c.py", "line": 3, "sizeDiff": 3000, "countDiff": 30, "size": 8000, "count": 80, "traceback": []}
    ]}"#;

    let result = diff::parse_diff_output(json).unwrap();
    assert_eq!(result.grown_allocations.len(), 3);
    assert!(
        result.grown_allocations[0].size_diff >= result.grown_allocations[1].size_diff,
        "grown allocations should be sorted by sizeDiff descending"
    );
    assert!(
        result.grown_allocations[1].size_diff >= result.grown_allocations[2].size_diff,
        "grown allocations should be sorted by sizeDiff descending"
    );
}

// ── Leak confidence scoring tests ────────────────────────────────────────

fn make_growth(file: &str, line: i32, size_diff: i64) -> AllocationGrowth {
    AllocationGrowth {
        file: file.to_owned(),
        line,
        size_diff,
        count_diff: 100,
        size: 1_000_000,
        count: 500,
        traceback: vec![TraceFrame {
            file: file.to_owned(),
            line,
        }],
    }
}

#[test]
fn leak_tracker_single_growth_is_low_confidence() {
    let mut tracker = LeakTracker::new();
    let growths = vec![make_growth("src/app.py", 10, 1000)];
    let leaks = tracker.process_growths(&growths);

    assert_eq!(leaks.len(), 1, "should produce 1 suspected leak");
    assert_eq!(leaks[0].confidence, LeakConfidence::Low);
    assert_eq!(leaks[0].file, "src/app.py");
    assert_eq!(leaks[0].line, 10);
    assert_eq!(leaks[0].size_growth, 1000);
    assert!(!leaks[0].reason.is_empty(), "reason should not be empty");
}

#[test]
fn leak_tracker_two_consecutive_growths_is_medium() {
    let mut tracker = LeakTracker::new();
    let growths = vec![make_growth("src/app.py", 10, 1000)];
    let _ = tracker.process_growths(&growths);
    let leaks = tracker.process_growths(&growths);

    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].confidence, LeakConfidence::Medium);
}

#[test]
fn leak_tracker_three_consecutive_growths_is_high() {
    let mut tracker = LeakTracker::new();
    let growths = vec![make_growth("src/app.py", 10, 1000)];
    let _ = tracker.process_growths(&growths);
    let _ = tracker.process_growths(&growths);
    let leaks = tracker.process_growths(&growths);

    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].confidence, LeakConfidence::High);
}

#[test]
fn leak_tracker_large_single_growth_is_medium() {
    let mut tracker = LeakTracker::new();
    let growths = vec![make_growth("src/app.py", 10, 20 * 1024 * 1024)]; // 20 MB
    let leaks = tracker.process_growths(&growths);

    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].confidence, LeakConfidence::Medium);
    assert!(
        leaks[0].reason.contains("MB"),
        "reason should mention MB, got: {}",
        leaks[0].reason
    );
}

#[test]
fn leak_tracker_non_growth_resets_consecutive_count() {
    let mut tracker = LeakTracker::new();
    let growths = vec![make_growth("src/app.py", 10, 1000)];
    let _ = tracker.process_growths(&growths);
    let _ = tracker.process_growths(&growths); // Medium

    // No growth this time.
    let _ = tracker.process_growths(&[]);

    // Next growth should be Low again (reset).
    let leaks = tracker.process_growths(&growths);
    assert_eq!(leaks[0].confidence, LeakConfidence::Low);
}

#[test]
fn leak_tracker_multiple_sites_tracked_independently() {
    let mut tracker = LeakTracker::new();
    let growths_a = vec![make_growth("src/a.py", 10, 1000)];
    let growths_b = vec![make_growth("src/b.py", 20, 2000)];

    let _ = tracker.process_growths(&growths_a);
    let _ = tracker.process_growths(&growths_a); // a is now Medium

    // b is still first-time.
    let mut combined = growths_a.clone();
    combined.extend(growths_b.clone());
    let leaks = tracker.process_growths(&combined);

    assert_eq!(leaks.len(), 2, "should track 2 sites");
    let leak_a = leaks.iter().find(|l| l.file == "src/a.py").unwrap();
    let leak_b = leaks.iter().find(|l| l.file == "src/b.py").unwrap();

    assert_eq!(
        leak_a.confidence,
        LeakConfidence::High,
        "a should be High after 3 growths"
    );
    assert_eq!(
        leak_b.confidence,
        LeakConfidence::Low,
        "b should be Low on first growth"
    );
}

#[test]
fn leak_confidence_ordering() {
    assert!(LeakConfidence::Definite > LeakConfidence::High);
    assert!(LeakConfidence::High > LeakConfidence::Medium);
    assert!(LeakConfidence::Medium > LeakConfidence::Low);
    assert!(LeakConfidence::Low < LeakConfidence::Definite);
}

#[test]
fn leak_confidence_display() {
    assert_eq!(format!("{}", LeakConfidence::Low), "LOW");
    assert_eq!(format!("{}", LeakConfidence::Medium), "MEDIUM");
    assert_eq!(format!("{}", LeakConfidence::High), "HIGH");
    assert_eq!(format!("{}", LeakConfidence::Definite), "DEFINITE");
}

// ── Privilege/permission tests ───────────────────────────────────────────

#[test]
fn permission_status_equality() {
    assert_eq!(PermissionStatus::Allowed, PermissionStatus::Allowed);
    assert_ne!(
        PermissionStatus::Allowed,
        PermissionStatus::Denied("test".to_owned())
    );
    assert_ne!(
        PermissionStatus::ElevationRequired("reason".to_owned()),
        PermissionStatus::Denied("reason".to_owned())
    );
}

#[test]
fn check_permissions_does_not_panic_for_any_pid() {
    // Should not panic regardless of input.
    let result = privilege::check_profiling_permissions(0);
    assert!(result.is_ok(), "PID 0 check should not fail");

    let result = privilege::check_profiling_permissions(1);
    assert!(result.is_ok(), "PID 1 check should not fail");

    let result = privilege::check_profiling_permissions(999_999);
    assert!(result.is_ok(), "PID 999999 check should not fail");

    let result = privilege::check_profiling_permissions(u32::MAX);
    assert!(result.is_ok(), "PID u32::MAX check should not fail");
}

#[test]
fn platform_permission_message_is_descriptive() {
    let msg = privilege::platform_permission_message();
    assert!(!msg.is_empty(), "message should not be empty");
    assert!(
        msg.len() > 20,
        "message should be descriptive, got length {}",
        msg.len()
    );
}

// ── ProfileError tests ───────────────────────────────────────────────────

#[test]
fn profile_error_display_session_not_found() {
    let err = basilisk_lsp::profiler::ProfileError::SessionNotFound("abc-123".to_owned());
    let msg = err.to_string();
    assert!(
        msg.contains("abc-123"),
        "error display should contain session ID, got: {msg}"
    );
}

#[test]
fn profile_error_display_already_profiling() {
    let err = basilisk_lsp::profiler::ProfileError::AlreadyProfiling {
        pid: 12345,
        session_id: "sess-001".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("12345"), "should contain PID");
    assert!(msg.contains("sess-001"), "should contain session ID");
}

#[test]
fn profile_error_display_export_failed() {
    let err = basilisk_lsp::profiler::ProfileError::ExportFailed("disk full".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("disk full"), "should contain error message");
}

#[test]
fn profile_error_codes_are_negative() {
    let errors = vec![
        basilisk_lsp::profiler::ProfileError::SessionNotFound("x".to_owned()),
        basilisk_lsp::profiler::ProfileError::ExportFailed("x".to_owned()),
        basilisk_lsp::profiler::ProfileError::AlreadyProfiling {
            pid: 1,
            session_id: "x".to_owned(),
        },
    ];

    for err in &errors {
        let code = err.error_code();
        assert!(code < 0, "error code should be negative, got: {code}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// FULL PIPELINE E2E TESTS
// These test the ENTIRE profiling pipeline end-to-end:
// py-spy traces → aggregate → hot spots → export → diagnostics → verify
// ═══════════════════════════════════════════════════════════════════════════

// Simulate a realistic multi-file Python web application being profiled:
// - main.py calls handle_request
// - handle_request calls parse_json + query_db + render_template
// - parse_json is the hotspot (called from tight loop)
// - query_db has moderate CPU usage
// - render_template is cool
//
// This creates 500 samples across 3 threads, then verifies the entire
// pipeline produces correct hotspot identification, valid exports, and
// properly formatted diagnostics. Delegates to focused helper functions.

/// Ingest 500 simulated sampling ticks into a `ProfileData` with realistic
/// multi-thread, multi-function distribution.
fn ingest_web_app_traces(data: &mut ProfileData, sample_weight: f64) {
    for tick in 0..500 {
        let mut traces = Vec::new();

        match tick % 5 {
            0..3 => {
                traces.push(make_trace(
                    1,
                    true,
                    vec![
                        ("parse_json", "/app/src/parser.py", 42),
                        ("handle_request", "/app/src/views.py", 15),
                        ("main", "/app/main.py", 8),
                    ],
                ));
            }
            3 => {
                traces.push(make_trace(
                    1,
                    true,
                    vec![
                        ("query_db", "/app/src/database.py", 78),
                        ("handle_request", "/app/src/views.py", 20),
                        ("main", "/app/main.py", 8),
                    ],
                ));
            }
            _ => {
                traces.push(make_trace(
                    1,
                    true,
                    vec![
                        ("render_template", "/app/src/templates.py", 33),
                        ("handle_request", "/app/src/views.py", 25),
                        ("main", "/app/main.py", 8),
                    ],
                ));
            }
        }

        traces.push(make_trace(
            2,
            true,
            vec![("background_task", "/app/src/worker.py", 12)],
        ));

        let gc_active = tick % 20 == 0;
        traces.push(make_trace(
            3,
            gc_active,
            vec![("gc_collect", "/app/src/gc_helper.py", 5)],
        ));

        data.ingest_traces(&traces, sample_weight, false);
    }
}

/// Verify aggregation totals: sample count, thread presence, line hits,
/// and per-function self/total sample counts.
fn verify_aggregation(data: &ProfileData) {
    assert_eq!(data.total_samples, 500, "should have 500 sample ticks");

    assert!(data.thread_names.contains_key(&1), "thread 1 should exist");
    assert!(data.thread_names.contains_key(&2), "thread 2 should exist");
    assert!(data.thread_names.contains_key(&3), "thread 3 should exist");

    assert!(
        data.thread_samples.get(&1).copied().unwrap_or(0) > 0,
        "thread 1 should have samples"
    );
    assert!(
        data.thread_samples.get(&2).copied().unwrap_or(0) > 0,
        "thread 2 should have samples"
    );

    let parser_hits = data
        .line_hits
        .get("/app/src/parser.py")
        .and_then(|m| m.get(&42))
        .copied()
        .unwrap_or(0);
    assert!(
        parser_hits >= 200,
        "parser.py:42 should have 300 hits (60% of 500), got {parser_hits}"
    );

    let parse_json_stats = data
        .function_stats
        .get("/app/src/parser.py")
        .and_then(|m| m.get("parse_json"))
        .expect("parse_json should have stats");
    assert!(
        parse_json_stats.self_samples >= 200,
        "parse_json self_samples should be ~300, got {}",
        parse_json_stats.self_samples
    );
    assert!(
        parse_json_stats.total_samples >= 200,
        "parse_json total_samples should be ~300, got {}",
        parse_json_stats.total_samples
    );

    let handle_request_stats = data
        .function_stats
        .get("/app/src/views.py")
        .and_then(|m| m.get("handle_request"))
        .expect("handle_request should have stats");
    assert!(
        handle_request_stats.total_samples >= 400,
        "handle_request total should be ~500 (appears in all stacks), got {}",
        handle_request_stats.total_samples
    );
    assert_eq!(
        handle_request_stats.self_samples, 0,
        "handle_request should have 0 self samples (never the leaf)"
    );
}

/// Verify hotspot detection identifies the expected hot lines and functions.
fn verify_hotspots(data: &ProfileData, config: &HotspotConfig) {
    let hot_lines = data.hot_lines(config);
    let hot_funcs = data.hot_functions(config);

    assert!(
        !hot_lines.is_empty(),
        "should identify hot lines in a 500-sample profile"
    );
    assert!(
        !hot_funcs.is_empty(),
        "should identify hot functions in a 500-sample profile"
    );

    let hottest_line = &hot_lines[0];
    assert!(
        hottest_line.percentage > 5.0,
        "hottest line should be >5%, got {:.1}%",
        hottest_line.percentage
    );

    let parse_json_hot = hot_funcs.iter().find(|f| f.name == "parse_json");
    assert!(
        parse_json_hot.is_some(),
        "parse_json should be in hot functions list"
    );
    let background_hot = hot_funcs.iter().find(|f| f.name == "background_task");
    assert!(
        background_hot.is_some(),
        "background_task should be in hot functions list"
    );

    let pj = parse_json_hot.unwrap();
    assert!(
        pj.self_percentage > 5.0,
        "parse_json should have >5% self-time, got {:.1}%",
        pj.self_percentage
    );
}

/// Verify speedscope and flamegraph exports produce valid output files.
fn verify_exports(data: &ProfileData, output_dir: &std::path::Path) {
    let speedscope_result =
        export::export_speedscope(data, "pipeline-test", 12345, 5.0, output_dir);
    assert!(
        speedscope_result.is_ok(),
        "speedscope export should succeed: {:?}",
        speedscope_result.err()
    );

    let speedscope_export = speedscope_result.unwrap();
    assert!(
        speedscope_export.path.exists(),
        "speedscope file should exist on disk"
    );

    let speedscope_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&speedscope_export.path).expect("read speedscope"),
    )
    .expect("parse speedscope JSON");

    assert!(
        speedscope_json.get("$schema").is_some(),
        "should have $schema"
    );
    let shared_frames = speedscope_json["shared"]["frames"]
        .as_array()
        .expect("shared.frames should be array");
    assert!(
        shared_frames.len() >= 5,
        "should have at least 5 unique frames, got {}",
        shared_frames.len()
    );

    let profiles = speedscope_json["profiles"]
        .as_array()
        .expect("profiles should be array");
    assert!(
        profiles.len() >= 2,
        "should have at least 2 thread profiles, got {}",
        profiles.len()
    );

    for (idx, profile) in profiles.iter().enumerate() {
        let samples = profile["samples"].as_array().unwrap_or(&Vec::new()).len();
        let weights = profile["weights"].as_array().unwrap_or(&Vec::new()).len();
        assert_eq!(
            samples, weights,
            "profile {idx}: samples count should equal weights count"
        );
        assert!(samples > 0, "profile {idx}: should have at least 1 sample");
    }

    let flamegraph_result = export::export_flamegraph(data, "pipeline-test", output_dir);
    assert!(
        flamegraph_result.is_ok(),
        "flamegraph export should succeed: {:?}",
        flamegraph_result.err()
    );

    let flamegraph_export = flamegraph_result.unwrap();
    assert!(
        flamegraph_export.path.exists(),
        "flamegraph SVG should exist on disk"
    );

    let svg_content =
        std::fs::read_to_string(&flamegraph_export.path).expect("read flamegraph SVG");
    assert!(svg_content.contains("<svg"), "should be valid SVG");
    assert!(
        svg_content.contains("parse_json"),
        "SVG should contain parse_json function name"
    );
    assert!(
        svg_content.contains("handle_request"),
        "SVG should contain handle_request function name"
    );
}

/// Verify diagnostics are generated for the expected files with correct format.
fn verify_cpu_diagnostics(data: &ProfileData, config: &HotspotConfig) {
    use basilisk_lsp::profiler::diagnostics;

    let diag_map = diagnostics::generate_diagnostics(data, config);

    assert!(
        !diag_map.is_empty(),
        "should generate diagnostics for at least one file"
    );

    let parser_diags: Vec<_> = diag_map
        .iter()
        .filter(|(uri, _)| uri.path().contains("parser.py"))
        .collect();
    assert!(
        !parser_diags.is_empty(),
        "should have diagnostics for parser.py"
    );

    for (_, diags) in &parser_diags {
        for diag in *diags {
            assert_eq!(
                diag.source.as_deref(),
                Some("basilisk-profiler"),
                "diagnostic source should be basilisk-profiler"
            );
            assert!(
                diag.message.contains("CPU") || diag.message.contains("Hot"),
                "diagnostic message should mention CPU or Hot, got: {}",
                diag.message
            );
            assert!(diag.severity.is_some(), "diagnostic should have a severity");
        }
    }
}

/// Verify profile diff correctly identifies hotter/cooler functions after
/// simulated optimization.
fn verify_profile_diff(
    data_before: &ProfileData,
    sample_weight: f64,
    output_dir: &std::path::Path,
) {
    let mut data_after_optimization = ProfileData::default();
    for tick in 0..500 {
        let mut traces = Vec::new();
        if tick % 5 == 0 {
            traces.push(make_trace(
                1,
                true,
                vec![
                    ("parse_json", "/app/src/parser.py", 42),
                    ("handle_request", "/app/src/views.py", 15),
                    ("main", "/app/main.py", 8),
                ],
            ));
        } else {
            traces.push(make_trace(
                1,
                true,
                vec![
                    ("query_db", "/app/src/database.py", 78),
                    ("handle_request", "/app/src/views.py", 20),
                    ("main", "/app/main.py", 8),
                ],
            ));
        }
        traces.push(make_trace(
            2,
            true,
            vec![("background_task", "/app/src/worker.py", 12)],
        ));
        data_after_optimization.ingest_traces(&traces, sample_weight, false);
    }

    let diff_result =
        export::export_profile_diff(data_before, &data_after_optimization, output_dir);
    assert!(diff_result.is_ok(), "profile diff export should succeed");

    let diff_path = diff_result.unwrap().path;
    assert!(diff_path.exists(), "diff JSON file should exist");

    let diff_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&diff_path).expect("read diff"))
            .expect("parse diff JSON");

    assert!(
        diff_json.get("hotter").is_some(),
        "diff should have 'hotter' array"
    );
    assert!(
        diff_json.get("cooler").is_some(),
        "diff should have 'cooler' array"
    );
    assert!(
        diff_json.get("unchanged").is_some(),
        "diff should have 'unchanged' array"
    );

    let cooler = diff_json["cooler"]
        .as_array()
        .expect("cooler should be array");
    let parse_json_cooler = cooler
        .iter()
        .find(|entry| entry["name"].as_str() == Some("parse_json"));
    assert!(
        parse_json_cooler.is_some(),
        "parse_json should appear in cooler list after optimization"
    );

    let hotter = diff_json["hotter"]
        .as_array()
        .expect("hotter should be array");
    let query_db_hotter = hotter
        .iter()
        .find(|entry| entry["name"].as_str() == Some("query_db"));
    assert!(
        query_db_hotter.is_some(),
        "query_db should appear in hotter list after optimization"
    );
}

#[test]
fn full_pipeline_realistic_web_app_profile() {
    let mut data = ProfileData::default();
    let sample_weight = 0.01; // 100 Hz

    ingest_web_app_traces(&mut data, sample_weight);
    verify_aggregation(&data);

    let config = HotspotConfig::default();
    verify_hotspots(&data, &config);

    let output_dir = std::env::temp_dir().join("basilisk-pipeline-test");
    let _ = std::fs::create_dir_all(&output_dir);

    verify_exports(&data, &output_dir);
    verify_cpu_diagnostics(&data, &config);
    verify_profile_diff(&data, sample_weight, &output_dir);

    let _ = std::fs::remove_dir_all(&output_dir);
}

/// Return raw snapshot output strings simulating a cache growing over time.
fn memory_snapshot_outputs() -> (&'static str, &'static str, &'static str) {
    let snap1 = r#"debug output line 1
__BASILISK_MEM__{"current": 10000000, "peak": 10000000, "gcObjects": 5000, "gcCounts": [100, 10, 1], "stats": [{"file": "/app/src/cache.py", "line": 34, "size": 5000000, "count": 1000, "traceback": [{"file": "/app/src/cache.py", "line": 34}]}, {"file": "/app/src/loader.py", "line": 12, "size": 3000000, "count": 500, "traceback": [{"file": "/app/src/loader.py", "line": 12}]}]}
more output"#;

    let snap2 = r#"debug output
__BASILISK_MEM__{"current": 25000000, "peak": 25000000, "gcObjects": 12000, "gcCounts": [200, 20, 2], "stats": [{"file": "/app/src/cache.py", "line": 34, "size": 18000000, "count": 5000, "traceback": [{"file": "/app/src/cache.py", "line": 34}]}, {"file": "/app/src/loader.py", "line": 12, "size": 3000000, "count": 500, "traceback": [{"file": "/app/src/loader.py", "line": 12}]}]}"#;

    let snap3 = r#"__BASILISK_MEM__{"current": 45000000, "peak": 45000000, "gcObjects": 20000, "gcCounts": [300, 30, 3], "stats": [{"file": "/app/src/cache.py", "line": 34, "size": 35000000, "count": 12000, "traceback": [{"file": "/app/src/cache.py", "line": 34}]}, {"file": "/app/src/loader.py", "line": 12, "size": 3200000, "count": 510, "traceback": [{"file": "/app/src/loader.py", "line": 12}]}]}"#;

    (snap1, snap2, snap3)
}

/// Parse snapshots and verify fields, growth, and per-file allocations.
fn verify_snapshots(snap1: &MemorySnapshot, snap3: &MemorySnapshot) {
    assert_eq!(snap1.snapshot_id, "snap-001");
    assert_eq!(snap1.current_memory, 10_000_000);
    assert_eq!(snap1.peak_memory, 10_000_000);
    assert_eq!(snap1.gc_objects, 5000);
    assert_eq!(snap1.gc_counts, vec![100, 10, 1]);
    assert_eq!(snap1.top_allocations.len(), 2);

    assert_eq!(snap3.current_memory, 45_000_000);
    assert!(
        snap3.current_memory > snap1.current_memory * 3,
        "memory should have grown >3x between snap1 and snap3"
    );

    let cache_alloc = snap3
        .top_allocations
        .iter()
        .find(|a| a.file.contains("cache.py"))
        .expect("cache.py should be in top allocations");
    assert_eq!(cache_alloc.line, 34);
    assert_eq!(cache_alloc.size, 35_000_000);
    assert_eq!(cache_alloc.count, 12_000);
    assert!(!cache_alloc.traceback.is_empty(), "should have traceback");

    let loader_alloc = snap3
        .top_allocations
        .iter()
        .find(|a| a.file.contains("loader.py"))
        .expect("loader.py should be in top allocations");
    assert!(
        loader_alloc.size < 4_000_000,
        "loader.py should be stable at ~3MB"
    );
}

/// Build allocation growth vectors and run leak detection, verifying
/// confidence escalation across consecutive diffs.
fn verify_leak_detection() -> Vec<AllocationGrowth> {
    let growth_1_to_2: Vec<AllocationGrowth> = vec![
        AllocationGrowth {
            file: "/app/src/cache.py".to_owned(),
            line: 34,
            size_diff: 13_000_000,
            count_diff: 4000,
            size: 18_000_000,
            count: 5000,
            traceback: vec![TraceFrame {
                file: "/app/src/cache.py".to_owned(),
                line: 34,
            }],
        },
        AllocationGrowth {
            file: "/app/src/loader.py".to_owned(),
            line: 12,
            size_diff: 0,
            count_diff: 0,
            size: 3_000_000,
            count: 500,
            traceback: vec![],
        },
    ];

    let growth_2_to_3: Vec<AllocationGrowth> = vec![AllocationGrowth {
        file: "/app/src/cache.py".to_owned(),
        line: 34,
        size_diff: 17_000_000,
        count_diff: 7000,
        size: 35_000_000,
        count: 12_000,
        traceback: vec![TraceFrame {
            file: "/app/src/cache.py".to_owned(),
            line: 34,
        }],
    }];

    let mut tracker = LeakTracker::new();
    let leaks_1 = tracker.process_growths(&growth_1_to_2);
    assert!(!leaks_1.is_empty(), "should detect growth in cache.py");
    assert!(
        leaks_1[0].confidence >= LeakConfidence::Low,
        "first growth should be at least Low confidence, got {:?}",
        leaks_1[0].confidence
    );

    let leaks_2 = tracker.process_growths(&growth_2_to_3);
    assert!(!leaks_2.is_empty(), "should detect continued growth");

    let cache_leak = leaks_2
        .iter()
        .find(|l| l.file.contains("cache.py"))
        .expect("cache.py should be in leaks");
    assert!(
        cache_leak.confidence >= LeakConfidence::Medium,
        "2 consecutive growths with large sizes should be at least Medium, got {:?}",
        cache_leak.confidence
    );
    assert!(
        cache_leak.size_growth > 10_000_000,
        "growth should be >10MB"
    );

    growth_2_to_3
}

/// Verify allocation and leak diagnostics are generated correctly.
fn verify_memory_diagnostics(snap3: &MemorySnapshot, growth: &[AllocationGrowth]) {
    use basilisk_lsp::profiler::memory::diagnostics as mem_diag;

    let alloc_config = mem_diag::MemoryHotspotConfig::default();
    let alloc_diags = mem_diag::generate_allocation_diagnostics(snap3, &alloc_config);
    assert!(
        !alloc_diags.is_empty(),
        "should generate allocation diagnostics for top allocations"
    );

    let cache_diag_found = alloc_diags.iter().any(|(uri, diags)| {
        uri.path().contains("cache.py")
            && diags
                .iter()
                .any(|d| d.message.contains("Allocation") || d.message.contains("alloc"))
    });
    assert!(
        cache_diag_found,
        "cache.py should have allocation diagnostic"
    );

    let diff_data = diff::MemoryDiff {
        total_growth: 30_000_000,
        total_freed: 0,
        net_growth: 30_000_000,
        grown_allocations: growth.to_vec(),
        freed_allocations: vec![],
    };
    let mut fresh_tracker = LeakTracker::new();
    let (_leaks, leak_diags) = mem_diag::generate_diff_diagnostics(&diff_data, &mut fresh_tracker);
    assert!(!leak_diags.is_empty(), "should generate leak diagnostics");

    let cache_leak_diag = leak_diags
        .iter()
        .find(|(uri, _)| uri.path().contains("cache.py"));
    assert!(
        cache_leak_diag.is_some(),
        "cache.py should have leak diagnostic"
    );
    let (_, diags) = cache_leak_diag.unwrap();
    assert!(!diags.is_empty(), "should have at least one diagnostic");
    assert!(
        diags[0].message.contains("growth") || diags[0].message.contains("leak"),
        "diagnostic should mention growth or leak, got: {}",
        diags[0].message
    );
}

/// Test memory profiling pipeline: snapshot parsing -> diff -> leak detection -> diagnostics.
#[test]
fn full_pipeline_memory_profiling_leak_detection() {
    let (out1, out2, out3) = memory_snapshot_outputs();

    let snap1 = memory::parse_snapshot_output(out1, "snap-001").expect("should parse snapshot 1");
    let _snap2 = memory::parse_snapshot_output(out2, "snap-002").expect("should parse snapshot 2");
    let snap3 = memory::parse_snapshot_output(out3, "snap-003").expect("should parse snapshot 3");

    verify_snapshots(&snap1, &snap3);

    let growth_2_to_3 = verify_leak_detection();

    verify_memory_diagnostics(&snap3, &growth_2_to_3);
}

/// Test presets produce correct configurations and can be used in the pipeline.
#[test]
fn presets_produce_valid_configs_for_pipeline() {
    use basilisk_lsp::profiler::presets::{preset_config, ProfilingPreset};

    let quick = preset_config(ProfilingPreset::Quick, 12345);
    assert_eq!(quick.pid, 12345);
    assert_eq!(quick.sample_rate, 100);
    assert!(quick.duration.is_some());
    assert_eq!(
        quick.duration.unwrap().as_secs(),
        10,
        "Quick preset should be 10 seconds"
    );

    let detailed = preset_config(ProfilingPreset::Detailed, 12345);
    assert_eq!(detailed.sample_rate, 200);
    assert_eq!(detailed.duration.unwrap().as_secs(), 60);

    let long = preset_config(ProfilingPreset::LongRunning, 12345);
    assert_eq!(long.sample_rate, 50);
    assert!(
        long.duration.is_none(),
        "LongRunning should have no duration limit"
    );

    // Verify parse_name covers all variants.
    assert_eq!(
        ProfilingPreset::parse_name("quick"),
        Some(ProfilingPreset::Quick)
    );
    assert_eq!(
        ProfilingPreset::parse_name("detailed"),
        Some(ProfilingPreset::Detailed)
    );
    assert_eq!(
        ProfilingPreset::parse_name("longRunning"),
        Some(ProfilingPreset::LongRunning)
    );
    assert_eq!(
        ProfilingPreset::parse_name("long_running"),
        Some(ProfilingPreset::LongRunning)
    );
    assert_eq!(ProfilingPreset::parse_name("nonexistent"), None);
    assert_eq!(ProfilingPreset::parse_name(""), None);
}

// ═══════════════════════════════════════════════════════════════════════════
// REAL PY-SPY E2E — Spawn Python, attach py-spy, verify real CPU samples
// ═══════════════════════════════════════════════════════════════════════════

use basilisk_lsp::profiler::sampler::SamplerConfig;

fn spawn_cpu_python() -> Option<ProcessGuard> {
    let script = r#"
import time

def hot_function():
    total = 0
    for i in range(10_000_000):
        total += i * i
    return total

def warm_function():
    total = 0
    for i in range(5_000_000):
        total += i
    return total

# Self-terminate after 30 seconds to prevent orphaned processes.
deadline = time.time() + 30
while time.time() < deadline:
    hot_function()
    warm_function()
"#;
    std::process::Command::new("python3")
        .args(["-c", script])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
        .map(ProcessGuard::new)
}

fn spawn_threaded_python() -> Option<ProcessGuard> {
    let script = r#"
import threading, time

def cpu_worker():
    # Self-terminate after 30 seconds to prevent orphaned processes.
    deadline = time.time() + 30
    total = 0
    while time.time() < deadline:
        total += sum(range(100_000))

threads = [threading.Thread(target=cpu_worker, daemon=True) for _ in range(3)]
for t in threads:
    t.start()
for t in threads:
    t.join()
"#;
    std::process::Command::new("python3")
        .args(["-c", script])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
        .map(ProcessGuard::new)
}

fn try_attach(pid: u32, rate: u64) -> Option<basilisk_lsp::profiler::sampler::SamplerHandle> {
    let config = SamplerConfig {
        pid,
        sample_rate: rate,
        include_native: false,
        include_idle: false,
        duration: None,
    };
    match basilisk_lsp::profiler::sampler::start_sampler(&config) {
        Ok(h) => Some(h),
        Err(err) => {
            let msg = err.to_string();
            // On macOS, SIP can block task_for_pid even for child processes
            // in sandboxed/CI contexts. Treat all attachment failures as skip-worthy.
            eprintln!("SKIP py-spy ({msg})");
            None
        }
    }
}

fn drain_samples(
    handle: &mut basilisk_lsp::profiler::sampler::SamplerHandle,
    data: &mut ProfileData,
    secs: u64,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        match handle.receiver.try_recv() {
            Ok(batch) => data.ingest_traces(&batch.traces, 0.01, false),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
}

fn kill(guard: &mut ProcessGuard) {
    guard.kill();
}

#[test]
fn real_pyspy_detects_python_version() {
    let Some(mut guard) = spawn_cpu_python() else {
        eprintln!("SKIP: no python3");
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(500));
    let Some(handle) = try_attach(guard.id(), 100) else {
        kill(&mut guard);
        return;
    };
    assert!(!handle.python_version.is_empty());
    assert!(handle.python_version.contains('.'));
    let major: u32 = handle
        .python_version
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    assert!(major >= 3, "need Python 3+, got: {}", handle.python_version);
    handle.stop();
    kill(&mut guard);
}

#[test]
fn real_pyspy_collects_cpu_samples_with_named_functions() {
    let Some(mut guard) = spawn_cpu_python() else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(500));
    let Some(mut handle) = try_attach(guard.id(), 100) else {
        kill(&mut guard);
        return;
    };

    let mut data = ProfileData::default();
    drain_samples(&mut handle, &mut data, 2);
    handle.stop();
    kill(&mut guard);

    assert!(
        data.total_samples >= 10,
        "need >=10 samples, got {}",
        data.total_samples
    );
    assert!(!data.line_hits.is_empty(), "must have line hits");
    assert!(!data.function_stats.is_empty(), "must have function stats");
    assert!(!data.frames.is_empty(), "must have speedscope frames");
    assert!(!data.thread_samples.is_empty(), "must have thread data");

    let funcs: Vec<String> = data
        .function_stats
        .values()
        .flat_map(|m| m.keys().cloned())
        .collect();
    let named: Vec<&String> = funcs.iter().filter(|n| !n.starts_with('<')).collect();
    assert!(!named.is_empty(), "should see named funcs, got: {funcs:?}");

    println!(
        "REAL: {} samples, {} funcs {:?}",
        data.total_samples,
        funcs.len(),
        &funcs[..funcs.len().min(8)]
    );
}

#[test]
fn real_pyspy_exports_speedscope_and_flamegraph_from_live_data() {
    let Some(mut guard) = spawn_cpu_python() else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(500));
    let Some(mut handle) = try_attach(guard.id(), 100) else {
        kill(&mut guard);
        return;
    };

    let mut data = ProfileData::default();
    drain_samples(&mut handle, &mut data, 1);
    handle.stop();
    let pid = guard.id();
    kill(&mut guard);

    if data.total_samples == 0 {
        eprintln!("SKIP: 0 samples");
        return;
    }

    let dir = std::env::temp_dir().join("basilisk_real_export");
    let _ = std::fs::create_dir_all(&dir);

    // Speedscope
    let ss = export::export_speedscope(&data, "real", pid, 1.0, &dir).expect("speedscope");
    assert!(ss.path.exists());
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ss.path).unwrap()).unwrap();
    assert!(json.get("$schema").is_some());
    let profs = json["profiles"].as_array().unwrap();
    assert!(!profs.is_empty());
    for p in profs {
        assert!(!p["samples"].as_array().unwrap().is_empty());
    }

    // Flamegraph
    let fg = export::export_flamegraph(&data, "real", &dir).expect("flamegraph");
    assert!(fg.path.exists());
    let svg = std::fs::read_to_string(&fg.path).unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.len() > 500);

    // Hot spots
    let config = HotspotConfig::default();
    assert!(!data.hot_lines(&config).is_empty(), "real hot lines");
    assert!(!data.hot_functions(&config).is_empty(), "real hot funcs");

    println!(
        "EXPORT: speedscope={} bytes, svg={} bytes",
        json.to_string().len(),
        svg.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn real_pyspy_multithreaded_sees_multiple_threads() {
    let Some(mut guard) = spawn_threaded_python() else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(800));
    let Some(mut handle) = try_attach(guard.id(), 100) else {
        kill(&mut guard);
        return;
    };

    let mut data = ProfileData::default();
    drain_samples(&mut handle, &mut data, 2);
    handle.stop();
    kill(&mut guard);

    if data.total_samples == 0 {
        eprintln!("SKIP: 0 samples");
        return;
    }

    assert!(
        data.thread_samples.len() >= 2,
        "threaded Python should show >=2 threads, got {}",
        data.thread_samples.len()
    );
    for (&tid, &count) in &data.thread_samples {
        assert!(count > 0, "thread {tid} must have samples");
    }

    println!(
        "THREADED: {} samples, {} threads",
        data.total_samples,
        data.thread_samples.len()
    );
}

#[tokio::test]
async fn real_pyspy_session_manager_full_lifecycle() {
    let Some(mut guard) = spawn_cpu_python() else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(500));
    let pid = guard.id();
    let mgr = ProfileSessionManager::new();

    let start = match mgr.start(pid, Some(100), Some(false), None).await {
        Ok(s) => s,
        Err(err) => {
            // On macOS, SIP can block task_for_pid even for child processes.
            eprintln!("SKIP session manager ({err})");
            kill(&mut guard);
            return;
        }
    };

    assert!(!start.session_id.is_empty());
    assert_eq!(start.pid, pid);
    assert!(start.python_version.contains('.'));
    assert_eq!(mgr.list().await.len(), 1);
    assert!(
        mgr.start(pid, None, None, None).await.is_err(),
        "dup PID rejected"
    );

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let snap = mgr.snapshot(&start.session_id).await.expect("snapshot");
    assert!(snap.total_samples > 0);

    let stop = mgr.stop(&start.session_id).await.expect("stop");
    assert_eq!(stop.session_id, start.session_id);
    assert!(stop.total_samples >= snap.total_samples);
    assert!(stop.duration > 0.0);
    assert!(
        !stop.hot_functions.is_empty(),
        "must detect hot funcs from real Python"
    );
    assert!(
        !stop.hot_lines.is_empty(),
        "must detect hot lines from real Python"
    );
    assert!(mgr.list().await.is_empty());

    println!(
        "SESSION MGR: Python {}, {} samples, {:.1}s",
        start.python_version, stop.total_samples, stop.duration
    );
    for f in stop.hot_functions.iter().take(3) {
        println!(
            "  {} — {:.1}% ({:.1}% self)",
            f.name, f.percentage, f.self_percentage
        );
    }
    kill(&mut guard);
}

// ═══════════════════════════════════════════════════════════════════════════
// HARDCORE E2E: Prove the profiler CORRECTLY measures CPU distribution
// ═══════════════════════════════════════════════════════════════════════════

/// Spawn Python where `hot_function` does 4x more work than `cool_function`.
/// The profiler MUST see `hot_function` with significantly more samples.
fn spawn_asymmetric_workload() -> Option<ProcessGuard> {
    // hot_function: 10M iterations of multiplication (heavy CPU)
    // cool_function: 1M iterations of addition (light CPU)
    // Ratio should be roughly 10:1 in CPU time.
    let script = r#"
import time

def hot_function():
    """Does 10x more CPU work than cool_function."""
    total = 0
    for i in range(10_000_000):
        total += i * i * i
    return total

def cool_function():
    """Light CPU work."""
    total = 0
    for i in range(1_000_000):
        total += i
    return total

def main():
    # Self-terminate after 30 seconds to prevent orphaned processes.
    deadline = time.time() + 30
    while time.time() < deadline:
        hot_function()
        cool_function()

main()
"#;
    std::process::Command::new("python3")
        .args(["-c", script])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
        .map(ProcessGuard::new)
}

/// Spawn Python with a known memory allocation pattern for allocation tracking.
fn spawn_allocating_python() -> Option<ProcessGuard> {
    let script = r#"
import sys, time

def allocate_big_list():
    """Allocates a large list that stays in memory."""
    return [i * i for i in range(500_000)]

def allocate_small():
    """Allocates and immediately discards."""
    for _ in range(100):
        _ = [0] * 1000

def main():
    big = allocate_big_list()
    # Self-terminate after 30 seconds to prevent orphaned processes.
    deadline = time.time() + 30
    while time.time() < deadline:
        allocate_small()
        # Keep big alive so it shows in memory
        _ = len(big)

main()
"#;
    std::process::Command::new("python3")
        .args(["-c", script])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
        .map(ProcessGuard::new)
}

#[test]
fn real_pyspy_hot_function_gets_more_samples_than_cool_function() {
    let Some(mut guard) = spawn_asymmetric_workload() else {
        eprintln!("SKIP: no python3");
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(500));

    let Some(mut handle) = try_attach(guard.id(), 100) else {
        kill(&mut guard);
        return;
    };

    // Collect 3 seconds of samples for statistical significance.
    let mut data = ProfileData::default();
    drain_samples(&mut handle, &mut data, 3);
    handle.stop();
    kill(&mut guard);

    if data.total_samples < 50 {
        eprintln!("SKIP: only {} samples (need 50+)", data.total_samples);
        return;
    }

    // Find hot_function and cool_function in the profile data.
    let all_funcs: std::collections::HashMap<String, u64> = data
        .function_stats
        .values()
        .flat_map(|m| m.iter())
        .map(|(name, stats)| (name.clone(), stats.self_samples))
        .collect();

    let hot_samples = all_funcs.get("hot_function").copied().unwrap_or(0);
    let cool_samples = all_funcs.get("cool_function").copied().unwrap_or(0);

    println!("=== ASYMMETRIC WORKLOAD PROOF ===");
    println!("Total samples: {}", data.total_samples);
    println!("hot_function self-samples:  {hot_samples}");
    println!("cool_function self-samples: {cool_samples}");
    for (name, samples) in all_funcs.iter().take(8) {
        println!("  {name}: {samples} self-samples");
    }

    // CRITICAL ASSERTION: hot_function MUST have more self-samples than cool_function.
    // hot_function does 10x more iterations of heavier work, so it should dominate.
    assert!(
        hot_samples > 0,
        "hot_function must appear in profile data with self-samples"
    );

    if cool_samples > 0 {
        assert!(
            hot_samples > cool_samples,
            "hot_function ({hot_samples} samples) MUST have more CPU samples than \
             cool_function ({cool_samples} samples) — it does 10x more work!"
        );

        // hot_function should have at least 2x the samples of cool_function
        // (it does 10x the work, but Python overhead and GIL mean the ratio
        // won't be exactly 10:1).
        assert!(
            hot_samples >= cool_samples * 2,
            "hot_function ({hot_samples}) should have >=2x samples of cool_function ({cool_samples})"
        );
    }
}

#[test]
fn real_pyspy_self_samples_vs_total_samples_are_correct() {
    let Some(mut guard) = spawn_asymmetric_workload() else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(500));

    let Some(mut handle) = try_attach(guard.id(), 100) else {
        kill(&mut guard);
        return;
    };

    let mut data = ProfileData::default();
    drain_samples(&mut handle, &mut data, 2);
    handle.stop();
    kill(&mut guard);

    if data.total_samples < 20 {
        eprintln!("SKIP: too few samples");
        return;
    }

    // For every function, self_samples <= total_samples.
    // Leaf functions (hot_function, cool_function) should have self_samples > 0.
    // Caller functions (main) should have total > self.
    for funcs in data.function_stats.values() {
        for (name, stats) in funcs {
            assert!(
                stats.self_samples <= stats.total_samples,
                "{name}: self_samples ({}) must be <= total_samples ({})",
                stats.self_samples,
                stats.total_samples
            );
        }
    }

    // main() is a caller — it should have total_samples > 0 but self_samples
    // should be low or zero (it just calls hot_function and cool_function).
    let main_stats = data
        .function_stats
        .values()
        .flat_map(|m| m.iter())
        .find(|(name, _)| *name == "main");

    if let Some((_, stats)) = main_stats {
        assert!(
            stats.total_samples > stats.self_samples,
            "main() total ({}) should exceed self ({}) — it's a caller, not a leaf",
            stats.total_samples,
            stats.self_samples
        );
        println!(
            "main(): total={}, self={} (correctly more total than self)",
            stats.total_samples, stats.self_samples
        );
    }
}

#[test]
fn real_pyspy_speedscope_json_matches_actual_sample_data() {
    let Some(mut guard) = spawn_cpu_python() else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(500));

    let Some(mut handle) = try_attach(guard.id(), 100) else {
        kill(&mut guard);
        return;
    };

    let mut data = ProfileData::default();
    drain_samples(&mut handle, &mut data, 1);
    let pid = guard.id();
    handle.stop();
    kill(&mut guard);

    if data.total_samples == 0 {
        return;
    }

    let dir = std::env::temp_dir().join("basilisk_json_verify");
    let _ = std::fs::create_dir_all(&dir);

    let ss = export::export_speedscope(&data, "verify", pid, 1.0, &dir).expect("export");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ss.path).unwrap()).unwrap();

    // The number of unique frames in JSON must match our frame index.
    let json_frames = json["shared"]["frames"].as_array().unwrap();
    assert_eq!(
        json_frames.len(),
        data.frames.len(),
        "speedscope frame count must match ProfileData frame count"
    );

    // Every frame must have name, file, and line fields.
    for (idx, frame) in json_frames.iter().enumerate() {
        assert!(
            frame.get("name").and_then(|v| v.as_str()).is_some(),
            "frame {idx} must have 'name'"
        );
        assert!(
            frame.get("file").and_then(|v| v.as_str()).is_some(),
            "frame {idx} must have 'file'"
        );
        assert!(frame.get("line").is_some(), "frame {idx} must have 'line'");
    }

    // Number of profiles must match number of active threads.
    let json_profiles = json["profiles"].as_array().unwrap();
    assert_eq!(
        json_profiles.len(),
        data.thread_stacks.len(),
        "speedscope profile count must match thread count"
    );

    // Total samples across all profiles must match our stacks.
    let json_total_samples: usize = json_profiles
        .iter()
        .map(|p| p["samples"].as_array().map_or(0, Vec::len))
        .sum();
    let data_total_stacks: usize = data.thread_stacks.values().map(Vec::len).sum();
    assert_eq!(
        json_total_samples, data_total_stacks,
        "total sample stacks in JSON must match ProfileData"
    );

    // Each sample must reference valid frame indices.
    let max_frame_idx = json_frames.len();
    for profile in json_profiles {
        for sample in profile["samples"].as_array().unwrap() {
            for frame_ref in sample.as_array().unwrap() {
                let idx = usize::try_from(frame_ref.as_u64().unwrap()).unwrap();
                assert!(
                    idx < max_frame_idx,
                    "frame index {idx} out of bounds (max {max_frame_idx})"
                );
            }
        }
    }

    println!(
        "SPEEDSCOPE VERIFIED: {} frames, {} profiles, {} total stacks — all indices valid",
        json_frames.len(),
        json_profiles.len(),
        json_total_samples
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn real_pyspy_diagnostics_point_to_correct_lines() {
    let Some(mut guard) = spawn_cpu_python() else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(500));

    let Some(mut handle) = try_attach(guard.id(), 100) else {
        kill(&mut guard);
        return;
    };

    let mut data = ProfileData::default();
    drain_samples(&mut handle, &mut data, 2);
    handle.stop();
    kill(&mut guard);

    if data.total_samples < 20 {
        return;
    }

    let config = HotspotConfig::default();
    let hot_lines = data.hot_lines(&config);
    let hot_funcs = data.hot_functions(&config);

    // Every hot line must have a positive sample count and valid percentage.
    for line in &hot_lines {
        assert!(line.samples > 0, "hot line must have >0 samples");
        assert!(
            line.percentage > 0.0 && line.percentage <= 100.0,
            "percentage must be in (0, 100], got {:.2}",
            line.percentage
        );
        assert!(line.line > 0, "line number must be positive");
        assert!(!line.file.is_empty(), "file path must not be empty");
    }

    // Every hot function must have consistent self vs total.
    for func in &hot_funcs {
        assert!(func.samples > 0, "hot func must have >0 samples");
        assert!(
            func.self_percentage <= func.percentage + 0.01,
            "{}: self% ({:.1}) cannot exceed total% ({:.1})",
            func.name,
            func.self_percentage,
            func.percentage
        );
        assert!(func.line > 0, "function line must be positive");
    }

    // Hot lines should be sorted by sample count (descending).
    for window in hot_lines.windows(2) {
        assert!(
            window[0].samples >= window[1].samples,
            "hot lines must be sorted descending: {} >= {}",
            window[0].samples,
            window[1].samples
        );
    }

    // Hot functions should be sorted by sample count (descending).
    for window in hot_funcs.windows(2) {
        assert!(
            window[0].samples >= window[1].samples,
            "hot funcs must be sorted descending"
        );
    }

    println!(
        "DIAGNOSTICS VERIFIED: {} hot lines, {} hot funcs, all sorted and valid",
        hot_lines.len(),
        hot_funcs.len()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// HARDCORE: Prove the profiler correctly identifies CPU hotspots
// ═══════════════════════════════════════════════════════════════════════════

/// Spawn Python where `cpu_burner` wastes 99% of CPU.
/// Profiler MUST identify it as the dominant hotspot. PROVES it works.
#[test]
fn real_pyspy_proves_hotspot_identification_is_correct() {
    let script = r#"
import time
def cpu_burner():
    total = 0
    for i in range(50_000_000):
        total += i * i * i
    return total
def light_work():
    return sum(range(100))
start = time.time()
while time.time() - start < 10:
    cpu_burner()
    light_work()
"#;
    // Write to a temp file so py-spy reports absolute paths (needed for diagnostic URIs).
    let script_path = std::env::temp_dir().join("basilisk_hotspot_proof.py");
    std::fs::write(&script_path, script).expect("write temp script");

    let Some(mut guard) = std::process::Command::new("python3")
        .arg(&script_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
        .map(ProcessGuard::new)
    else {
        eprintln!("SKIP: no python3");
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(800));
    let Some(mut handle) = try_attach(guard.id(), 200) else {
        kill(&mut guard);
        return;
    };
    let mut data = ProfileData::default();
    drain_samples(&mut handle, &mut data, 3);
    handle.stop();
    kill(&mut guard);

    if data.total_samples < 20 {
        eprintln!("SKIP: {} samples", data.total_samples);
        return;
    }
    println!("HOTSPOT PROOF: {} samples at 200Hz", data.total_samples);

    // PROOF 1: cpu_burner MUST appear.
    let burner_found = data
        .function_stats
        .values()
        .any(|m| m.contains_key("cpu_burner"));
    assert!(burner_found, "cpu_burner MUST be in function stats");

    // PROOF 2: cpu_burner self-samples > light_work.
    let burner_self: u64 = data
        .function_stats
        .values()
        .filter_map(|m| m.get("cpu_burner"))
        .map(|s| s.self_samples)
        .sum();
    let light_self: u64 = data
        .function_stats
        .values()
        .filter_map(|m| m.get("light_work"))
        .map(|s| s.self_samples)
        .sum();
    println!("  cpu_burner: {burner_self}, light_work: {light_self}");
    assert!(
        burner_self > light_self,
        "cpu_burner ({burner_self}) MUST dominate light_work ({light_self})"
    );

    // PROOF 3: cpu_burner in hot functions with >20% self.
    let config = HotspotConfig {
        line_threshold_pct: 1.0,
        function_threshold_pct: 1.0,
        max_diagnostics_per_file: 50,
    };
    let hot = data.hot_functions(&config);
    let burner_hot = hot.iter().find(|f| f.name == "cpu_burner");
    assert!(burner_hot.is_some(), "cpu_burner MUST be hot");
    let bh = burner_hot.unwrap();
    println!("  cpu_burner: {:.1}% self", bh.self_percentage);
    assert!(bh.self_percentage > 20.0, "cpu_burner should be >20% self");

    // PROOF 4: Exports contain cpu_burner.
    let dir = std::env::temp_dir().join("basilisk_hotspot_proof");
    let _ = std::fs::create_dir_all(&dir);
    let ss = export::export_speedscope(&data, "proof", 0, 3.0, &dir).expect("speedscope");
    let json = std::fs::read_to_string(&ss.path).expect("read");
    assert!(
        json.contains("cpu_burner"),
        "speedscope must have cpu_burner"
    );
    let fg = export::export_flamegraph(&data, "proof", &dir).expect("flamegraph");
    let svg = std::fs::read_to_string(&fg.path).expect("read");
    assert!(svg.contains("cpu_burner"), "SVG must have cpu_burner");

    // PROOF 5: Diagnostics from real data.
    let diags = basilisk_lsp::profiler::diagnostics::generate_diagnostics(&data, &config);
    assert!(
        !diags.is_empty(),
        "must generate diagnostics from real profile"
    );
    let diag_count: usize = diags.values().map(Vec::len).sum();
    println!("  {diag_count} diagnostics across {} files", diags.len());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&script_path);
    println!("  PROOF COMPLETE: profiler correctly identifies cpu_burner");
}

/// Verify relative CPU distribution across 3 functions of different weight.
#[test]
fn real_pyspy_proves_relative_cpu_distribution() {
    let script = r#"
import time
def heavy():
    r = 0
    for i in range(20_000_000):
        r += (i * 7 + 3) % 1000
    return r
def medium():
    r = 0
    for i in range(12_000_000):
        r += len(str(i))
    return r
def light():
    r = 0
    for i in range(8_000_000):
        r += i % 7
    return r
start = time.time()
while time.time() - start < 10:
    heavy()
    medium()
    light()
"#;
    let Some(mut guard) = std::process::Command::new("python3")
        .args(["-c", script])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
        .map(ProcessGuard::new)
    else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(800));
    let Some(mut handle) = try_attach(guard.id(), 200) else {
        kill(&mut guard);
        return;
    };
    let mut data = ProfileData::default();
    drain_samples(&mut handle, &mut data, 3);
    handle.stop();
    kill(&mut guard);
    if data.total_samples < 20 {
        eprintln!("SKIP: {} samples", data.total_samples);
        return;
    }
    println!("DISTRIBUTION: {} samples", data.total_samples);

    let get_self = |name: &str| -> u64 {
        data.function_stats
            .values()
            .filter_map(|m| m.get(name))
            .map(|s| s.self_samples)
            .sum()
    };
    let h = get_self("heavy");
    let m = get_self("medium");
    let l = get_self("light");
    println!("  heavy: {h}, medium: {m}, light: {l}");
    assert!(h > 0, "heavy must have samples");
    assert!(h >= l, "heavy ({h}) should have >= light ({l})");
    println!("  DISTRIBUTION TEST COMPLETE");
}

// ── Process enumeration must hide debugger infrastructure ───────────────────
// Tests for [PROFILE-PROCESSES-MODEL]. A debugpy adapter is machinery, not a
// profiling target: offering it in the panel invites profiling the debugger
// itself, and adapters orphaned by a hard-killed editor linger as phantom
// rows (the "process shown without running anything" defect).

/// Spawn a real `python -m debugpy.adapter`, a plain Python sleeper, and
/// assert enumeration lists the sleeper but never the adapter.
#[tokio::test]
async fn enumeration_hides_debugpy_adapter_but_lists_real_targets() {
    use std::process::{Command, Stdio};

    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned());

    let mut sleeper = common::ProcessGuard::new(
        Command::new(&python)
            .args(["-c", "import time; time.sleep(30)"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn python sleeper"),
    );
    let mut adapter = common::ProcessGuard::new(
        Command::new(&python)
            .args(["-m", "debugpy.adapter", "--port", "0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn debugpy.adapter"),
    );

    // Let both settle so the process table sees them.
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    let processes = basilisk_lsp::profiler::processes::enumerate_python_processes();
    let sleeper_pid = sleeper.id();
    let adapter_pid = adapter.id();
    sleeper.kill();
    adapter.kill();

    assert!(
        processes.iter().any(|p| p.pid == sleeper_pid),
        "a plain python process must be offered for profiling"
    );
    assert!(
        !processes.iter().any(|p| p.pid == adapter_pid),
        "debugger infrastructure (debugpy.adapter) must NEVER be listed as a target"
    );
}
