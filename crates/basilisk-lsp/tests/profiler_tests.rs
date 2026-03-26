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

// ═══════════════════════════════════════════════════════════════════════════
// FULL PIPELINE E2E TESTS
// These test the ENTIRE profiling pipeline end-to-end:
// py-spy traces → aggregate → hot spots → export → diagnostics → verify
// ═══════════════════════════════════════════════════════════════════════════

/// Simulate a realistic multi-file Python web application being profiled:
/// - main.py calls handle_request
/// - handle_request calls parse_json + query_db + render_template
/// - parse_json is the hotspot (called from tight loop)
/// - query_db has moderate CPU usage
/// - render_template is cool
///
/// This creates 500 samples across 3 threads, then verifies the entire
/// pipeline produces correct hotspot identification, valid exports, and
/// properly formatted diagnostics.
#[test]
fn full_pipeline_realistic_web_app_profile() {
    use basilisk_lsp::profiler::diagnostics;

    let mut data = ProfileData::default();
    let sample_weight = 0.01; // 100 Hz

    // Simulate 500 sampling ticks with realistic distribution.
    for tick in 0..500 {
        let mut traces = Vec::new();

        // Thread 1 (MainThread): handle_request → parse_json (hot path).
        // parse_json is the leaf 60% of the time.
        if tick % 5 < 3 {
            traces.push(make_trace(
                1,
                true,
                vec![
                    ("parse_json", "/app/src/parser.py", 42),
                    ("handle_request", "/app/src/views.py", 15),
                    ("main", "/app/main.py", 8),
                ],
            ));
        } else if tick % 5 == 3 {
            // query_db is the leaf 20% of the time.
            traces.push(make_trace(
                1,
                true,
                vec![
                    ("query_db", "/app/src/database.py", 78),
                    ("handle_request", "/app/src/views.py", 20),
                    ("main", "/app/main.py", 8),
                ],
            ));
        } else {
            // render_template is the leaf 20% of the time.
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

        // Thread 2 (Worker): background_task (always active).
        traces.push(make_trace(
            2,
            true,
            vec![("background_task", "/app/src/worker.py", 12)],
        ));

        // Thread 3 (GC): idle most of the time, active 5%.
        let gc_active = tick % 20 == 0;
        traces.push(make_trace(
            3,
            gc_active,
            vec![("gc_collect", "/app/src/gc_helper.py", 5)],
        ));

        data.ingest_traces(&traces, sample_weight, false);
    }

    // ─── Verify aggregation results ──────────────────────────────
    assert_eq!(data.total_samples, 500, "should have 500 sample ticks");

    // Thread names.
    assert!(data.thread_names.contains_key(&1), "thread 1 should exist");
    assert!(data.thread_names.contains_key(&2), "thread 2 should exist");
    assert!(data.thread_names.contains_key(&3), "thread 3 should exist");

    // Thread sample counts.
    assert!(
        data.thread_samples.get(&1).copied().unwrap_or(0) > 0,
        "thread 1 should have samples"
    );
    assert!(
        data.thread_samples.get(&2).copied().unwrap_or(0) > 0,
        "thread 2 should have samples"
    );

    // Line hits: parser.py:42 should be the hottest line.
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

    // Function stats: parse_json should have the most self_samples.
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

    // handle_request should have high total but low self (it's a caller).
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

    // ─── Verify hot spots ────────────────────────────────────────
    let config = HotspotConfig::default();
    let hot_lines = data.hot_lines(&config);
    let hot_funcs = data.hot_functions(&config);

    assert!(
        !hot_lines.is_empty(),
        "should identify hot lines in a 500-sample profile"
    );
    assert!(
        !hot_funcs.is_empty(),
        "should identify hot functions in a 500-sample profile"
    );

    // parse_json's line should be the hottest.
    let hottest_line = &hot_lines[0];
    assert_eq!(
        hottest_line.file, "/app/src/parser.py",
        "hottest line should be in parser.py"
    );
    assert_eq!(hottest_line.line, 42, "hottest line should be line 42");
    assert!(
        hottest_line.percentage > 10.0,
        "hottest line should be >10%, got {:.1}%",
        hottest_line.percentage
    );

    // parse_json or background_task should be the hottest function.
    let hottest_func = &hot_funcs[0];
    assert!(
        hottest_func.name == "parse_json" || hottest_func.name == "background_task",
        "hottest function should be parse_json or background_task, got {}",
        hottest_func.name
    );

    // ─── Verify speedscope export ────────────────────────────────
    let output_dir = std::env::temp_dir().join("basilisk-pipeline-test");
    let _ = std::fs::create_dir_all(&output_dir);

    let speedscope_result =
        export::export_speedscope(&data, "pipeline-test", 12345, 5.0, &output_dir);
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

    // Validate speedscope structure.
    assert!(
        speedscope_json.get("$schema").is_some(),
        "should have $schema"
    );
    let shared_frames = speedscope_json["shared"]["frames"]
        .as_array()
        .expect("shared.frames should be array");
    assert!(
        shared_frames.len() >= 5,
        "should have at least 5 unique frames (parse_json, query_db, render_template, handle_request, main, background_task, gc_collect), got {}",
        shared_frames.len()
    );

    let profiles = speedscope_json["profiles"]
        .as_array()
        .expect("profiles should be array");
    assert!(
        profiles.len() >= 2,
        "should have at least 2 thread profiles (thread 1 + thread 2), got {}",
        profiles.len()
    );

    // Verify each profile has samples and weights.
    for (idx, profile) in profiles.iter().enumerate() {
        let samples = profile["samples"]
            .as_array()
            .unwrap_or(&Vec::new())
            .len();
        let weights = profile["weights"]
            .as_array()
            .unwrap_or(&Vec::new())
            .len();
        assert_eq!(
            samples, weights,
            "profile {idx}: samples count should equal weights count"
        );
        assert!(
            samples > 0,
            "profile {idx}: should have at least 1 sample"
        );
    }

    // ─── Verify flamegraph SVG export ────────────────────────────
    let flamegraph_result = export::export_flamegraph(&data, "pipeline-test", &output_dir);
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

    // ─── Verify diagnostics generation ───────────────────────────
    let diag_map = diagnostics::generate_diagnostics(&data, &config);

    assert!(
        !diag_map.is_empty(),
        "should generate diagnostics for at least one file"
    );

    // Find diagnostics for parser.py.
    let parser_diags: Vec<_> = diag_map
        .iter()
        .filter(|(uri, _)| uri.path().contains("parser.py"))
        .collect();
    assert!(
        !parser_diags.is_empty(),
        "should have diagnostics for parser.py"
    );

    // Check diagnostic message format.
    for (_, diags) in &parser_diags {
        for diag in diags.iter() {
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
            assert!(
                diag.severity.is_some(),
                "diagnostic should have a severity"
            );
        }
    }

    // ─── Verify profile diff between two sessions ────────────────
    let mut data_after_optimization = ProfileData::default();
    // After "optimizing" parse_json, it's now only 20% instead of 60%.
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
        export::export_profile_diff(&data, &data_after_optimization, &output_dir);
    assert!(
        diff_result.is_ok(),
        "profile diff export should succeed"
    );

    let diff_path = diff_result.unwrap().path;
    assert!(diff_path.exists(), "diff JSON file should exist");

    let diff_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&diff_path).expect("read diff"),
    )
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

    // parse_json should be cooler after optimization.
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

    // query_db should be hotter (now gets more relative time).
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

    // ─── Cleanup ─────────────────────────────────────────────────
    let _ = std::fs::remove_dir_all(&output_dir);
}

/// Test memory profiling pipeline: snapshot parsing → diff → leak detection → diagnostics.
#[test]
fn full_pipeline_memory_profiling_leak_detection() {
    use basilisk_lsp::profiler::memory::diagnostics as mem_diag;

    // Simulate 3 memory snapshots showing a cache growing over time.
    let snapshot_output_1 = r#"debug output line 1
__BASILISK_MEM__{"current": 10000000, "peak": 10000000, "gcObjects": 5000, "gcCounts": [100, 10, 1], "stats": [{"file": "/app/src/cache.py", "line": 34, "size": 5000000, "count": 1000, "traceback": [{"file": "/app/src/cache.py", "line": 34}]}, {"file": "/app/src/loader.py", "line": 12, "size": 3000000, "count": 500, "traceback": [{"file": "/app/src/loader.py", "line": 12}]}]}
more output"#;

    let snapshot_output_2 = r#"debug output
__BASILISK_MEM__{"current": 25000000, "peak": 25000000, "gcObjects": 12000, "gcCounts": [200, 20, 2], "stats": [{"file": "/app/src/cache.py", "line": 34, "size": 18000000, "count": 5000, "traceback": [{"file": "/app/src/cache.py", "line": 34}]}, {"file": "/app/src/loader.py", "line": 12, "size": 3000000, "count": 500, "traceback": [{"file": "/app/src/loader.py", "line": 12}]}]}"#;

    let snapshot_output_3 = r#"__BASILISK_MEM__{"current": 45000000, "peak": 45000000, "gcObjects": 20000, "gcCounts": [300, 30, 3], "stats": [{"file": "/app/src/cache.py", "line": 34, "size": 35000000, "count": 12000, "traceback": [{"file": "/app/src/cache.py", "line": 34}]}, {"file": "/app/src/loader.py", "line": 12, "size": 3200000, "count": 510, "traceback": [{"file": "/app/src/loader.py", "line": 12}]}]}"#;

    // Parse all 3 snapshots.
    let snap1 = memory::parse_snapshot_output(snapshot_output_1, "snap-001")
        .expect("should parse snapshot 1");
    let snap2 = memory::parse_snapshot_output(snapshot_output_2, "snap-002")
        .expect("should parse snapshot 2");
    let snap3 = memory::parse_snapshot_output(snapshot_output_3, "snap-003")
        .expect("should parse snapshot 3");

    // Verify snapshot 1 fields.
    assert_eq!(snap1.snapshot_id, "snap-001");
    assert_eq!(snap1.current_memory, 10_000_000);
    assert_eq!(snap1.peak_memory, 10_000_000);
    assert_eq!(snap1.gc_objects, 5000);
    assert_eq!(snap1.gc_counts, vec![100, 10, 1]);
    assert_eq!(snap1.top_allocations.len(), 2);

    // Verify snapshot 3 shows significant growth.
    assert_eq!(snap3.current_memory, 45_000_000);
    assert!(
        snap3.current_memory > snap1.current_memory * 3,
        "memory should have grown >3x between snap1 and snap3"
    );

    // cache.py should be the top allocator in snap3.
    let cache_alloc = snap3
        .top_allocations
        .iter()
        .find(|a| a.file.contains("cache.py"))
        .expect("cache.py should be in top allocations");
    assert_eq!(cache_alloc.line, 34);
    assert_eq!(cache_alloc.size, 35_000_000);
    assert_eq!(cache_alloc.count, 12_000);
    assert!(!cache_alloc.traceback.is_empty(), "should have traceback");

    // loader.py should have stable allocation (not leaking).
    let loader_alloc = snap3
        .top_allocations
        .iter()
        .find(|a| a.file.contains("loader.py"))
        .expect("loader.py should be in top allocations");
    assert!(
        loader_alloc.size < 4_000_000,
        "loader.py should be stable at ~3MB"
    );

    // ─── Simulate diff detection ─────────────────────────────────
    // Create growth entries for the leak tracker.
    let growth_1_to_2: Vec<AllocationGrowth> = vec![
        AllocationGrowth {
            file: "/app/src/cache.py".to_owned(),
            line: 34,
            size_growth: 13_000_000, // 5MB → 18MB
            count_growth: 4000,
            current_size: 18_000_000,
            current_count: 5000,
            traceback: vec![TraceFrame {
                file: "/app/src/cache.py".to_owned(),
                line: 34,
            }],
        },
        AllocationGrowth {
            file: "/app/src/loader.py".to_owned(),
            line: 12,
            size_growth: 0,
            count_growth: 0,
            current_size: 3_000_000,
            current_count: 500,
            traceback: vec![],
        },
    ];

    let growth_2_to_3: Vec<AllocationGrowth> = vec![AllocationGrowth {
        file: "/app/src/cache.py".to_owned(),
        line: 34,
        size_growth: 17_000_000, // 18MB → 35MB
        count_growth: 7000,
        current_size: 35_000_000,
        current_count: 12_000,
        traceback: vec![TraceFrame {
            file: "/app/src/cache.py".to_owned(),
            line: 34,
        }],
    }];

    // Run leak detection across diffs.
    let mut tracker = LeakTracker::new();
    let leaks_1 = tracker.process_growths(&growth_1_to_2);
    assert!(!leaks_1.is_empty(), "should detect growth in cache.py");
    assert_eq!(
        leaks_1[0].confidence,
        LeakConfidence::Low,
        "first growth should be Low confidence"
    );

    let leaks_2 = tracker.process_growths(&growth_2_to_3);
    assert!(!leaks_2.is_empty(), "should detect continued growth");

    // cache.py should now be Medium confidence (2 consecutive growths).
    let cache_leak = leaks_2
        .iter()
        .find(|l| l.file.contains("cache.py"))
        .expect("cache.py should be in leaks");
    assert_eq!(
        cache_leak.confidence,
        LeakConfidence::Medium,
        "2 consecutive growths should be Medium"
    );
    assert!(
        cache_leak.size_growth > 10_000_000,
        "growth should be >10MB"
    );

    // ─── Generate memory diagnostics ─────────────────────────────
    let alloc_diags = mem_diag::generate_alloc_diagnostics(&snap3.top_allocations, 1_000_000);
    assert!(
        !alloc_diags.is_empty(),
        "should generate allocation diagnostics for allocations >1MB"
    );

    // Check that cache.py gets an allocation diagnostic.
    let cache_diag_found = alloc_diags.iter().any(|(uri, diags)| {
        uri.path().contains("cache.py")
            && diags
                .iter()
                .any(|d| d.message.contains("Allocation") && d.message.contains("MB"))
    });
    assert!(
        cache_diag_found,
        "cache.py should have allocation diagnostic mentioning MB"
    );

    // Generate leak diagnostics.
    let leak_diags = mem_diag::generate_leak_diagnostics(&leaks_2);
    assert!(
        !leak_diags.is_empty(),
        "should generate leak diagnostics"
    );

    // Check diagnostic severity matches confidence.
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
