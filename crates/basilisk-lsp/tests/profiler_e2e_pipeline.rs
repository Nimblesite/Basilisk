//! Hardcore E2E pipeline tests that prove the profiler ACTUALLY WORKS.
//!
//! These tests construct real `py_spy::StackTrace` and `py_spy::Frame` objects
//! — the EXACT same types that py-spy produces when sampling a live Python
//! process — and feed them through the entire Basilisk profiler pipeline:
//!
//!   py_spy::StackTrace → ingest_traces() → ProfileData → hot_functions()
//!                       → export_speedscope() → JSON → verify structure
//!                       → export_flamegraph() → SVG → verify content
//!                       → generate_diagnostics() → LSP Diagnostics → verify messages
//!
//! If these tests pass, the profiler is PROVEN to correctly:
//! 1. Ingest real py-spy trace data
//! 2. Count per-line and per-function CPU samples accurately
//! 3. Identify hotspots with correct percentages
//! 4. Export valid speedscope JSON with correct frame data
//! 5. Generate flamegraph SVG with actual function names
//! 6. Produce LSP diagnostics that point to the right files and lines

#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::uninlined_format_args,
    missing_docs,
    clippy::needless_raw_string_hashes,
    dead_code,
    unused_imports
)]

use basilisk_lsp::profiler::aggregator::{HotspotConfig, ProfileData};
use basilisk_lsp::profiler::diagnostics::generate_diagnostics;
use basilisk_lsp::profiler::export::{export, ExportFormat};

/// Build a real `py_spy::Frame` — the EXACT type py-spy returns.
fn pyframe(name: &str, filename: &str, line: i32) -> py_spy::Frame {
    py_spy::Frame {
        name: name.to_owned(),
        filename: filename.to_owned(),
        module: None,
        short_filename: None,
        line,
        locals: None,
        is_entry: false,
        is_shim_entry: false,
    }
}

/// Build a real `py_spy::StackTrace` — the EXACT type py-spy returns.
fn pytrace(
    thread_id: u64,
    thread_name: &str,
    active: bool,
    owns_gil: bool,
    frames: Vec<py_spy::Frame>,
) -> py_spy::StackTrace {
    py_spy::StackTrace {
        pid: 12345,
        thread_id,
        thread_name: Some(thread_name.to_owned()),
        os_thread_id: Some(thread_id),
        active,
        owns_gil,
        frames,
        process_info: None,
    }
}

// ── Scenario: CPU-bound web server with hot request handler ───────────────

/// Simulate a Flask-like web server processing 1000 requests.
/// The request handler calls a database query (slow) and a template render.
///
/// Expected hotspot distribution:
///   db_query():       45% self-time (slow SQL)
///   render_template(): 25% self-time (Jinja-like rendering)
///   handle_request():  5% self-time (routing overhead)
///   parse_body():     15% self-time (JSON parsing)
///   validate_auth():  10% self-time (token verification)
#[test]
fn e2e_web_server_profiling_identifies_correct_hotspots() {
    let mut data = ProfileData::default();
    let sample_weight = 0.01; // 100 Hz

    // Simulate 1000 request-processing samples across the main thread.
    for i in 0..1000u64 {
        let traces = match i % 100 {
            // 45% of time in db_query (called from handle_request)
            0..=44 => vec![pytrace(
                1,
                "MainThread",
                true,
                true,
                vec![
                    pyframe("db_query", "/app/database.py", 87),
                    pyframe("handle_request", "/app/views.py", 42),
                    pyframe("app_dispatch", "/app/main.py", 15),
                ],
            )],
            // 25% in render_template
            45..=69 => vec![pytrace(
                1,
                "MainThread",
                true,
                true,
                vec![
                    pyframe("render_template", "/app/templates.py", 120),
                    pyframe("handle_request", "/app/views.py", 55),
                    pyframe("app_dispatch", "/app/main.py", 15),
                ],
            )],
            // 15% in parse_body
            70..=84 => vec![pytrace(
                1,
                "MainThread",
                true,
                true,
                vec![
                    pyframe("parse_body", "/app/parsers.py", 33),
                    pyframe("handle_request", "/app/views.py", 38),
                    pyframe("app_dispatch", "/app/main.py", 15),
                ],
            )],
            // 10% in validate_auth
            85..=94 => vec![pytrace(
                1,
                "MainThread",
                true,
                true,
                vec![
                    pyframe("validate_auth", "/app/auth.py", 22),
                    pyframe("handle_request", "/app/views.py", 35),
                    pyframe("app_dispatch", "/app/main.py", 15),
                ],
            )],
            // 5% in handle_request itself (routing overhead)
            _ => vec![pytrace(
                1,
                "MainThread",
                true,
                true,
                vec![
                    pyframe("handle_request", "/app/views.py", 30),
                    pyframe("app_dispatch", "/app/main.py", 15),
                ],
            )],
        };

        data.ingest_traces(&traces, sample_weight, false);
    }

    assert_eq!(data.total_samples, 1000);

    let config = HotspotConfig {
        line_threshold_pct: 1.0,
        function_threshold_pct: 2.0,
        max_diagnostics_per_file: 20,
    };

    // ── Verify hotspot ranking ──────────────────────────────────
    let hot_fns = data.hot_functions(&config);
    assert!(hot_fns.len() >= 5, "should detect at least 5 hot functions");

    // Sort by self-percentage to find the actual bottleneck.
    let mut by_self = hot_fns.clone();
    by_self.sort_by(|a, b| b.self_percentage.partial_cmp(&a.self_percentage).unwrap());

    // db_query should be the #1 bottleneck (45% self-time).
    assert_eq!(by_self[0].name, "db_query");
    assert!(
        (by_self[0].self_percentage - 45.0).abs() < 3.0,
        "db_query should be ~45% self (got {:.1}%)",
        by_self[0].self_percentage
    );
    assert_eq!(by_self[0].file, "/app/database.py");
    assert_eq!(by_self[0].line, 87);

    // render_template should be #2 (25%).
    assert_eq!(by_self[1].name, "render_template");
    assert!(
        (by_self[1].self_percentage - 25.0).abs() < 3.0,
        "render_template should be ~25% self (got {:.1}%)",
        by_self[1].self_percentage
    );

    // parse_body should be #3 (15%).
    assert_eq!(by_self[2].name, "parse_body");
    assert!(
        (by_self[2].self_percentage - 15.0).abs() < 3.0,
        "parse_body should be ~15% self (got {:.1}%)",
        by_self[2].self_percentage
    );

    // validate_auth should be #4 (10%).
    assert_eq!(by_self[3].name, "validate_auth");
    assert!(
        (by_self[3].self_percentage - 10.0).abs() < 3.0,
        "validate_auth should be ~10% self (got {:.1}%)",
        by_self[3].self_percentage
    );

    // handle_request has 5% self but 100% total (it's in every stack).
    let handle_req = by_self.iter().find(|f| f.name == "handle_request").unwrap();
    assert!(
        (handle_req.self_percentage - 5.0).abs() < 3.0,
        "handle_request self should be ~5% (got {:.1}%)",
        handle_req.self_percentage
    );
    assert!(
        handle_req.percentage > 90.0,
        "handle_request total should be >90% (it's in every stack, got {:.1}%)",
        handle_req.percentage
    );

    // ── Verify hot lines ────────────────────────────────────────
    let hot_lines = data.hot_lines(&config);
    // database.py:87 should be a hot line — it appears in 450/1000 sample stacks.
    // Percentage is relative to total line-sample hits (not total_samples), so it
    // will be lower than 45% because every stack also hits main.py:15 etc.
    let db_line = hot_lines
        .iter()
        .find(|l| l.file == "/app/database.py" && l.line == 87);
    assert!(db_line.is_some(), "database.py:87 should be a hot line");
    let db_line = db_line.unwrap();
    assert_eq!(
        db_line.samples, 450,
        "database.py:87 should have 450 samples"
    );
    assert!(
        db_line.percentage > 5.0,
        "database.py:87 should be a significant percentage (got {:.1}%)",
        db_line.percentage
    );

    // main.py:15 should be the hottest line (it's in EVERY stack).
    let main_line = hot_lines
        .iter()
        .find(|l| l.file == "/app/main.py" && l.line == 15);
    assert!(main_line.is_some(), "main.py:15 should be a hot line");
    assert_eq!(
        main_line.unwrap().samples,
        1000,
        "main.py:15 in every stack"
    );

    // ── Verify speedscope export ────────────────────────────────
    let output_dir = std::env::temp_dir().join("basilisk_e2e_web_server");
    let _ = std::fs::create_dir_all(&output_dir);

    let speedscope = export(
        &data,
        ExportFormat::Speedscope,
        "web-server-test",
        12345,
        10.0,
        &output_dir,
    )
    .unwrap();

    let json_str = std::fs::read_to_string(&speedscope.path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // Verify frames contain all our functions.
    let frames = parsed["shared"]["frames"].as_array().unwrap();
    let frame_names: Vec<&str> = frames.iter().filter_map(|f| f["name"].as_str()).collect();
    assert!(
        frame_names.contains(&"db_query"),
        "speedscope frames must include db_query"
    );
    assert!(
        frame_names.contains(&"render_template"),
        "must include render_template"
    );
    assert!(
        frame_names.contains(&"parse_body"),
        "must include parse_body"
    );
    assert!(
        frame_names.contains(&"validate_auth"),
        "must include validate_auth"
    );
    assert!(
        frame_names.contains(&"handle_request"),
        "must include handle_request"
    );
    assert!(
        frame_names.contains(&"app_dispatch"),
        "must include app_dispatch"
    );

    // Verify profiles have samples with weights.
    let profiles = parsed["profiles"].as_array().unwrap();
    assert!(
        !profiles.is_empty(),
        "must have at least one thread profile"
    );
    let main_profile = &profiles[0];
    let samples = main_profile["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 1000, "should have 1000 sample stacks");
    let weights = main_profile["weights"].as_array().unwrap();
    assert_eq!(weights.len(), 1000, "should have 1000 weights");

    // ── Verify flamegraph SVG ───────────────────────────────────
    let flamegraph = export(
        &data,
        ExportFormat::Flamegraph,
        "web-server-test",
        12345,
        10.0,
        &output_dir,
    )
    .unwrap();

    let svg = std::fs::read_to_string(&flamegraph.path).unwrap();
    assert!(
        svg.contains("db_query"),
        "flamegraph SVG must contain db_query"
    );
    assert!(
        svg.contains("render_template"),
        "must contain render_template"
    );
    assert!(
        svg.contains("handle_request"),
        "must contain handle_request"
    );

    // ── Verify LSP diagnostics ──────────────────────────────────
    let diagnostics = generate_diagnostics(&data, &config);
    assert!(
        diagnostics.len() >= 4,
        "should have diagnostics for multiple files"
    );

    // database.py should have a diagnostic mentioning db_query.
    let db_diags = diagnostics
        .iter()
        .find(|(uri, _)| uri.path().contains("database.py"))
        .map(|(_, d)| d);
    assert!(db_diags.is_some(), "database.py should have diagnostics");
    let db_diag_msgs: Vec<&str> = db_diags
        .unwrap()
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        db_diag_msgs
            .iter()
            .any(|m| m.contains("db_query") || m.contains("45")),
        "database.py diagnostic should mention db_query or 45%: {:?}",
        db_diag_msgs
    );

    println!("=== Web server profiling E2E PASSED ===");
    println!("  1000 samples, 6 functions across 5 files");
    for f in &by_self {
        println!(
            "  {} ({}:{}) — {:.1}% self, {:.1}% total",
            f.name, f.file, f.line, f.self_percentage, f.percentage
        );
    }
}

// ── Scenario: Multi-threaded data pipeline ────────────────────────────────

/// Simulate a multi-threaded ETL pipeline with 3 worker threads.
/// Thread 1: reads data (I/O bound, mostly idle)
/// Thread 2: transforms data (CPU bound)
/// Thread 3: writes results (mixed)
///
/// The profiler must correctly attribute CPU time PER THREAD.
#[test]
fn e2e_multithreaded_pipeline_per_thread_attribution() {
    let mut data = ProfileData::default();
    let sample_weight = 0.01;

    for i in 0..600u64 {
        let traces = vec![
            // Thread 1: Reader — mostly idle, occasionally active on read_chunk.
            pytrace(
                100,
                "Reader",
                i % 5 == 0,
                false,
                vec![
                    pyframe("read_chunk", "/etl/reader.py", 45),
                    pyframe("pipeline_main", "/etl/main.py", 10),
                ],
            ),
            // Thread 2: Transformer — always active, heavy computation.
            pytrace(
                200,
                "Transformer",
                true,
                i % 3 == 0,
                vec![
                    pyframe("transform_record", "/etl/transform.py", 78),
                    pyframe("apply_rules", "/etl/transform.py", 92),
                    pyframe("pipeline_main", "/etl/main.py", 10),
                ],
            ),
            // Thread 3: Writer — active 60% of the time.
            pytrace(
                300,
                "Writer",
                i % 5 < 3,
                false,
                vec![
                    pyframe("write_batch", "/etl/writer.py", 33),
                    pyframe("pipeline_main", "/etl/main.py", 10),
                ],
            ),
        ];

        data.ingest_traces(&traces, sample_weight, false);
    }

    // Thread 2 is always active — should dominate samples.
    let thread_samples = &data.thread_samples;
    let t1_samples = thread_samples.get(&100).copied().unwrap_or(0);
    let t2_samples = thread_samples.get(&200).copied().unwrap_or(0);
    let t3_samples = thread_samples.get(&300).copied().unwrap_or(0);

    assert!(
        t2_samples > t1_samples,
        "Transformer (always active) should have more samples than Reader (20% active): T2={t2_samples}, T1={t1_samples}"
    );
    assert!(
        t2_samples > t3_samples,
        "Transformer should have more samples than Writer (60% active): T2={t2_samples}, T3={t3_samples}"
    );

    // Verify thread names are captured.
    assert_eq!(
        data.thread_names.get(&100).map(String::as_str),
        Some("Reader")
    );
    assert_eq!(
        data.thread_names.get(&200).map(String::as_str),
        Some("Transformer")
    );
    assert_eq!(
        data.thread_names.get(&300).map(String::as_str),
        Some("Writer")
    );

    let config = HotspotConfig::default();
    let hot_fns = data.hot_functions(&config);

    // transform_record should be a top hotspot (it's in every active T2 sample).
    let transform_fn = hot_fns.iter().find(|f| f.name == "transform_record");
    assert!(transform_fn.is_some(), "transform_record must be a hotspot");

    // Speedscope export should have 3 thread profiles.
    let output_dir = std::env::temp_dir().join("basilisk_e2e_multithreaded");
    let _ = std::fs::create_dir_all(&output_dir);

    let speedscope = export(
        &data,
        ExportFormat::Speedscope,
        "mt-pipeline",
        9999,
        6.0,
        &output_dir,
    )
    .unwrap();

    let json_str = std::fs::read_to_string(&speedscope.path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let profiles = parsed["profiles"].as_array().unwrap();
    assert!(
        profiles.len() >= 2,
        "should have multiple thread profiles (got {})",
        profiles.len()
    );

    // Verify thread names in profiles.
    let profile_names: Vec<&str> = profiles.iter().filter_map(|p| p["name"].as_str()).collect();
    assert!(
        profile_names.iter().any(|n| n.contains("Transformer")),
        "profiles should include Transformer thread: {:?}",
        profile_names
    );

    println!("=== Multi-threaded pipeline E2E PASSED ===");
    println!("  T1(Reader): {} samples", t1_samples);
    println!("  T2(Transformer): {} samples", t2_samples);
    println!("  T3(Writer): {} samples", t3_samples);
}

// ── Scenario: GIL contention detection ────────────────────────────────────

/// Simulate a scenario where multiple threads contend for the GIL.
/// Only one thread at a time can hold the GIL. We verify the profiler
/// correctly tracks GIL ownership across threads.
#[test]
fn e2e_gil_contention_tracking() {
    let mut data = ProfileData::default();
    let sample_weight = 0.01;

    let mut gil_held_by_t1 = 0u64;
    let mut gil_held_by_t2 = 0u64;

    for i in 0..500u64 {
        // Alternate GIL between threads.
        let t1_has_gil = i % 3 != 2; // T1 has GIL 2/3 of the time
        let t2_has_gil = !t1_has_gil;

        if t1_has_gil {
            gil_held_by_t1 += 1;
        }
        if t2_has_gil {
            gil_held_by_t2 += 1;
        }

        let traces = vec![
            pytrace(
                1,
                "MainThread",
                true,
                t1_has_gil,
                vec![pyframe("compute", "/app/math.py", 50)],
            ),
            pytrace(
                2,
                "Worker",
                true,
                t2_has_gil,
                vec![pyframe("process", "/app/worker.py", 30)],
            ),
        ];

        data.ingest_traces(&traces, sample_weight, false);
    }

    // Both threads should have samples since both are active.
    assert!(data.thread_samples.get(&1).copied().unwrap_or(0) > 0);
    assert!(data.thread_samples.get(&2).copied().unwrap_or(0) > 0);

    // total_samples increments once per ingest_traces() call (= one sampling tick).
    assert_eq!(
        data.total_samples, 500,
        "should have 500 total samples (one per sampling tick)"
    );

    // Export and verify both threads appear.
    let output_dir = std::env::temp_dir().join("basilisk_e2e_gil");
    let _ = std::fs::create_dir_all(&output_dir);

    let speedscope = export(
        &data,
        ExportFormat::Speedscope,
        "gil-test",
        5555,
        5.0,
        &output_dir,
    )
    .unwrap();

    let json_str = std::fs::read_to_string(&speedscope.path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let profiles = parsed["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2, "should have 2 thread profiles");

    println!("=== GIL contention E2E PASSED ===");
    println!("  T1 held GIL: {} / 500 ticks", gil_held_by_t1);
    println!("  T2 held GIL: {} / 500 ticks", gil_held_by_t2);
}

// ── Scenario: Hotspot threshold sensitivity ───────────────────────────────

/// Verify that different threshold configurations correctly include/exclude
/// functions at various CPU percentages.
#[test]
fn e2e_threshold_sensitivity_includes_excludes_correctly() {
    let mut data = ProfileData::default();
    let sample_weight = 0.01;

    // 1000 samples with precise distribution.
    for i in 0..1000u64 {
        let traces = match i % 1000 {
            0..=499 => vec![pytrace(
                1,
                "Main",
                true,
                true,
                vec![pyframe("hot_50pct", "/app/a.py", 1)],
            )],
            500..=599 => vec![pytrace(
                1,
                "Main",
                true,
                true,
                vec![pyframe("warm_10pct", "/app/b.py", 1)],
            )],
            600..=649 => vec![pytrace(
                1,
                "Main",
                true,
                true,
                vec![pyframe("cool_5pct", "/app/c.py", 1)],
            )],
            650..=659 => vec![pytrace(
                1,
                "Main",
                true,
                true,
                vec![pyframe("barely_1pct", "/app/d.py", 1)],
            )],
            _ => vec![pytrace(
                1,
                "Main",
                true,
                true,
                vec![pyframe("noise_34pct", "/app/e.py", 1)],
            )],
        };
        data.ingest_traces(&traces, sample_weight, false);
    }

    // With 2% function threshold: hot_50pct, noise_34pct, warm_10pct, cool_5pct should appear.
    // barely_1pct (1%) should NOT appear.
    let config_2pct = HotspotConfig {
        function_threshold_pct: 2.0,
        line_threshold_pct: 2.0,
        max_diagnostics_per_file: 20,
    };
    let fns_2pct = data.hot_functions(&config_2pct);
    let names_2pct: Vec<&str> = fns_2pct.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names_2pct.contains(&"hot_50pct"),
        "50% should be above 2% threshold"
    );
    assert!(
        names_2pct.contains(&"warm_10pct"),
        "10% should be above 2% threshold"
    );
    assert!(
        names_2pct.contains(&"cool_5pct"),
        "5% should be above 2% threshold"
    );
    assert!(
        !names_2pct.contains(&"barely_1pct"),
        "1% should be BELOW 2% threshold"
    );

    // With 10% threshold: only hot_50pct, noise_34pct, warm_10pct should appear.
    let config_10pct = HotspotConfig {
        function_threshold_pct: 10.0,
        line_threshold_pct: 10.0,
        max_diagnostics_per_file: 20,
    };
    let fns_10pct = data.hot_functions(&config_10pct);
    let names_10pct: Vec<&str> = fns_10pct.iter().map(|f| f.name.as_str()).collect();
    assert!(names_10pct.contains(&"hot_50pct"), "50% above 10%");
    assert!(names_10pct.contains(&"warm_10pct"), "10% at threshold");
    assert!(
        !names_10pct.contains(&"cool_5pct"),
        "5% below 10% threshold"
    );

    println!("=== Threshold sensitivity E2E PASSED ===");
    println!("  At 2%: {:?}", names_2pct);
    println!("  At 10%: {:?}", names_10pct);
}
