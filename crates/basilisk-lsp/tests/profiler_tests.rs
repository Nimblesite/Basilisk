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
