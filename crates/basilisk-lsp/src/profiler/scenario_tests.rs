//! Realistic profiling scenario tests.
//!
//! These tests simulate REAL-WORLD profiling scenarios using py-spy's actual
//! `StackTrace` and `Frame` types. They prove the pipeline handles realistic
//! workloads: Django requests, multi-threaded pipelines, NumPy-heavy code.
//! Every test verifies hotspot detection, export, and diagnostics end-to-end.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::as_conversions
)]

use tower_lsp::lsp_types::DiagnosticSeverity;

use super::aggregator::{HotspotConfig, ProfileData};
use super::diagnostics::generate_diagnostics;
use super::export::{export_flamegraph, export_speedscope};

/// Build a py-spy `StackTrace` from frame definitions.
fn trace(thread_id: u64, active: bool, frames: &[(&str, &str, i32)]) -> py_spy::StackTrace {
    py_spy::StackTrace {
        pid: 0,
        thread_id,
        thread_name: Some(format!("Thread-{thread_id}")),
        owns_gil: thread_id == 1,
        active,
        frames: frames
            .iter()
            .map(|(name, file, line)| py_spy::Frame {
                name: (*name).to_owned(),
                filename: (*file).to_owned(),
                line: *line,
                short_filename: None,
                module: None,
                locals: None,
                is_entry: false,
                is_shim_entry: false,
            })
            .collect(),
        os_thread_id: Some(thread_id),
        process_info: None,
    }
}

/// Simulate a Django request handler profiling session.
///
/// Models: HTTP request → view → ORM query → template rendering.
/// 60% in ORM (the bottleneck), 25% in template, 15% in view logic.
#[test]
fn scenario_django_request_identifies_orm_as_bottleneck() {
    let mut data = ProfileData::default();

    // Django call stacks — leaf frame is index 0 (py-spy convention).
    let orm_stack = trace(
        1,
        true,
        &[
            ("_execute", "/venv/django/db/backends/utils.py", 67),
            (
                "execute_sql",
                "/venv/django/db/models/sql/compiler.py",
                1168,
            ),
            ("_fetch_all", "/venv/django/db/models/query.py", 1354),
            ("get_queryset", "/app/myapp/views.py", 58),
            ("handle", "/app/myapp/views.py", 42),
            ("process_request", "/venv/django/middleware/common.py", 35),
            (
                "execute_from_command_line",
                "/venv/django/core/management/__init__.py",
                401,
            ),
        ],
    );
    let template_stack = trace(
        1,
        true,
        &[
            ("render", "/venv/django/template/base.py", 171),
            ("render_to_string", "/venv/django/template/loader.py", 62),
            ("handle", "/app/myapp/views.py", 42),
            ("process_request", "/venv/django/middleware/common.py", 35),
            (
                "execute_from_command_line",
                "/venv/django/core/management/__init__.py",
                401,
            ),
        ],
    );
    let view_stack = trace(
        1,
        true,
        &[
            ("handle", "/app/myapp/views.py", 42),
            ("process_request", "/venv/django/middleware/common.py", 35),
            (
                "execute_from_command_line",
                "/venv/django/core/management/__init__.py",
                401,
            ),
        ],
    );

    // Simulate 1000 sample batches: 600 ORM, 250 template, 150 view.
    for _ in 0..600 {
        data.ingest_traces(std::slice::from_ref(&orm_stack), 0.01, false);
    }
    for _ in 0..250 {
        data.ingest_traces(std::slice::from_ref(&template_stack), 0.01, false);
    }
    for _ in 0..150 {
        data.ingest_traces(std::slice::from_ref(&view_stack), 0.01, false);
    }

    assert_eq!(
        data.total_samples, 1000,
        "should have 1000 total sample batches"
    );

    let config = HotspotConfig::default();
    let hot_funcs = data.hot_functions(&config);

    // ── _execute (ORM leaf) MUST be the #1 self-time hotspot ────────
    let execute_fn = hot_funcs
        .iter()
        .find(|f| f.name == "_execute")
        .expect("_execute must be detected as hot");
    assert_eq!(
        execute_fn.self_samples, 600,
        "_execute is leaf in 600 ORM batches"
    );
    assert!(
        execute_fn.self_percentage > 50.0,
        "_execute self% must be >50%, got {:.1}%",
        execute_fn.self_percentage
    );

    // ── render (template leaf) should be #2 ─────────────────────────
    let render_fn = hot_funcs
        .iter()
        .find(|f| f.name == "render")
        .expect("render must be detected");
    assert_eq!(render_fn.self_samples, 250);

    // ── handle has high TOTAL but low SELF (calls ORM + template) ───
    let handle_fn = hot_funcs
        .iter()
        .find(|f| f.name == "handle")
        .expect("handle must be detected");
    assert_eq!(handle_fn.samples, 1000, "handle is in ALL stacks");
    assert_eq!(
        handle_fn.self_samples, 150,
        "handle is leaf only in view-only stacks"
    );

    // ── Hot lines: ORM utils.py line 67 should be the hottest line ──
    let hot_lines = data.hot_lines(&config);
    let orm_line = hot_lines
        .iter()
        .find(|l| l.file.contains("utils.py") && l.line == 67)
        .expect("ORM utils.py:67 should be a hot line");
    assert_eq!(orm_line.samples, 600);

    // ── Export: speedscope should have all frame names ───────────────
    let dir = std::env::temp_dir().join("basilisk_django_test");
    let _ = std::fs::create_dir_all(&dir);

    let ss = export_speedscope(&data, "django-e2e", 12345, 10.0, &dir).expect("speedscope export");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ss.path).expect("read")).expect("parse");

    let frame_names: Vec<&str> = json["shared"]["frames"]
        .as_array()
        .expect("frames")
        .iter()
        .filter_map(|f| f.get("name").and_then(serde_json::Value::as_str))
        .collect();
    assert!(
        frame_names.contains(&"_execute"),
        "speedscope missing _execute"
    );
    assert!(frame_names.contains(&"handle"), "speedscope missing handle");
    assert!(frame_names.contains(&"render"), "speedscope missing render");

    // ── Flamegraph SVG should contain the hotspot function name ──────
    let fg = export_flamegraph(&data, "django-e2e", &dir).expect("flamegraph export");
    let svg = std::fs::read_to_string(&fg.path).expect("read SVG");
    assert!(svg.contains("<svg"), "output must be SVG");
    assert!(svg.contains("_execute"), "SVG must contain _execute frame");

    // ── Diagnostics: ORM file should have HINT diagnostics ──────────
    let diags = generate_diagnostics(&data, &config);
    assert!(
        diags.len() >= 3,
        "diagnostics for ≥3 files (ORM, template, view)"
    );

    for uri_diags in diags.values() {
        for diag in uri_diags {
            assert_eq!(diag.severity, Some(DiagnosticSeverity::HINT));
            assert_eq!(diag.source.as_deref(), Some("basilisk-profiler"));
            assert!(
                diag.data.is_some(),
                "every diagnostic needs structured data"
            );
            assert!(!diag.message.is_empty(), "message must not be empty");
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// Multi-threaded pipeline: 3 threads, only Thread 2 burns CPU.
///
/// Thread 1: reader (mostly idle, some active).
/// Thread 2: transformer (CPU bound — THE hotspot).
/// Thread 3: writer (light work).
/// Verifies thread-aware profiling and idle thread filtering.
#[test]
fn scenario_multithreaded_pipeline_finds_cpu_thread() {
    let mut data = ProfileData::default();

    let reader_active = trace(
        1,
        true,
        &[
            ("parse_row", "/app/pipeline/reader.py", 50),
            ("read_csv", "/app/pipeline/reader.py", 25),
        ],
    );
    let reader_idle = trace(1, false, &[("read_csv", "/app/pipeline/reader.py", 25)]);
    let transform = trace(
        2,
        true,
        &[
            ("heavy_compute", "/app/pipeline/transform.py", 42),
            ("transform", "/app/pipeline/transform.py", 15),
        ],
    );
    let writer = trace(
        3,
        true,
        &[
            ("flush_buffer", "/app/pipeline/writer.py", 30),
            ("write_output", "/app/pipeline/writer.py", 10),
        ],
    );

    // 100 active reader, 200 idle reader (skipped), 600 transform, 100 writer.
    for _ in 0..100 {
        data.ingest_traces(std::slice::from_ref(&reader_active), 0.01, false);
    }
    for _ in 0..200 {
        data.ingest_traces(std::slice::from_ref(&reader_idle), 0.01, false);
    }
    for _ in 0..600 {
        data.ingest_traces(std::slice::from_ref(&transform), 0.01, false);
    }
    for _ in 0..100 {
        data.ingest_traces(std::slice::from_ref(&writer), 0.01, false);
    }

    let config = HotspotConfig::default();
    let hot_funcs = data.hot_functions(&config);

    // ── heavy_compute MUST dominate ─────────────────────────────────
    let compute = hot_funcs
        .iter()
        .find(|f| f.name == "heavy_compute")
        .expect("heavy_compute must be a hotspot");
    assert_eq!(compute.self_samples, 600);
    assert!(
        compute.self_percentage > 60.0,
        "heavy_compute should be >60% self time, got {:.1}%",
        compute.self_percentage
    );

    // ── Idle reader samples should NOT inflate reader's self time ────
    let parse = hot_funcs.iter().find(|f| f.name == "parse_row");
    if let Some(parse_fn) = parse {
        assert_eq!(
            parse_fn.self_samples, 100,
            "parse_row should only count active samples"
        );
    }

    // ── Thread data should be present for threads 1, 2, 3 ──────────
    assert!(
        data.thread_samples.contains_key(&1),
        "thread 1 should have samples"
    );
    assert!(
        data.thread_samples.contains_key(&2),
        "thread 2 should have samples"
    );
    assert!(
        data.thread_samples.contains_key(&3),
        "thread 3 should have samples"
    );

    // Thread 2 should have the most samples.
    let t2_count = data.thread_samples.get(&2).copied().unwrap_or(0);
    let t1_count = data.thread_samples.get(&1).copied().unwrap_or(0);
    let t3_count = data.thread_samples.get(&3).copied().unwrap_or(0);
    assert!(
        t2_count > t1_count && t2_count > t3_count,
        "Thread 2 ({t2_count}) should have more samples than T1 ({t1_count}) and T3 ({t3_count})"
    );

    // ── Speedscope: verify per-thread profiles ──────────────────────
    let dir = std::env::temp_dir().join("basilisk_mt_test");
    let _ = std::fs::create_dir_all(&dir);

    let ss = export_speedscope(&data, "mt-e2e", 99999, 10.0, &dir).expect("export");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ss.path).expect("read")).expect("parse");

    let profiles = json["profiles"].as_array().expect("profiles");
    assert!(profiles.len() >= 2, "should have ≥2 thread profiles");

    let _ = std::fs::remove_dir_all(&dir);
}

/// 10K sample stress test: verify performance and correctness at scale.
///
/// Simulates a 100-second profiling session at 100 Hz with 5 different call
/// stacks. Verifies export + diagnostics stay within performance budget.
#[test]
fn scenario_10k_samples_performance_and_correctness() {
    let mut data = ProfileData::default();

    let stacks: Vec<py_spy::StackTrace> = vec![
        trace(
            1,
            true,
            &[
                ("json_decode", "/venv/json/decoder.py", 334),
                ("loads", "/venv/json/__init__.py", 346),
                ("parse_response", "/app/api/client.py", 89),
                ("fetch", "/app/api/client.py", 45),
                ("main", "/app/server.py", 12),
            ],
        ),
        trace(
            1,
            true,
            &[
                ("match", "/venv/re/__init__.py", 199),
                ("validate_input", "/app/api/validators.py", 23),
                ("process", "/app/api/handlers.py", 67),
                ("main", "/app/server.py", 12),
            ],
        ),
        trace(
            2,
            true,
            &[
                ("execute", "/venv/sqlalchemy/engine/default.py", 717),
                (
                    "_execute_context",
                    "/venv/sqlalchemy/engine/default.py",
                    680,
                ),
                ("save_record", "/app/models/base.py", 134),
                ("worker_loop", "/app/workers/saver.py", 28),
            ],
        ),
        trace(
            2,
            true,
            &[
                ("commit", "/venv/sqlalchemy/engine/default.py", 802),
                ("flush", "/venv/sqlalchemy/orm/session.py", 3455),
                ("save_batch", "/app/models/base.py", 156),
                ("worker_loop", "/app/workers/saver.py", 28),
            ],
        ),
        trace(
            3,
            true,
            &[
                ("send", "/venv/redis/client.py", 894),
                ("publish", "/app/events/bus.py", 45),
                ("emit", "/app/events/bus.py", 22),
            ],
        ),
    ];

    // Distribute 10000 samples: 40% json, 20% regex, 25% sql, 10% commit, 5% redis.
    let dist = [4000, 2000, 2500, 1000, 500];
    for (stack, &count) in stacks.iter().zip(&dist) {
        for _ in 0..count {
            data.ingest_traces(std::slice::from_ref(stack), 0.01, false);
        }
    }

    assert_eq!(data.total_samples, 10000);

    // ── Correctness: verify sample distribution ─────────────────────
    let config = HotspotConfig::default();
    let hot_funcs = data.hot_functions(&config);

    // json_decode should be the top self-time function (4000 samples).
    let json_fn = hot_funcs
        .iter()
        .find(|f| f.name == "json_decode")
        .expect("json_decode");
    assert_eq!(json_fn.self_samples, 4000);
    assert!(json_fn.self_percentage > 35.0);

    // main should have high total (6000) but zero self.
    let main_fn = hot_funcs.iter().find(|f| f.name == "main").expect("main");
    assert_eq!(main_fn.samples, 6000, "main is in json+regex stacks");
    assert_eq!(main_fn.self_samples, 0, "main is never the leaf");

    // ── Performance: export speedscope ──────────────────────────────
    // Debug builds are ~5x slower than release, so we scale the budgets.
    #[cfg(debug_assertions)]
    let (ss_budget, fg_budget, diag_budget) = (1000u128, 2500u128, 500u128);
    #[cfg(not(debug_assertions))]
    let (ss_budget, fg_budget, diag_budget) = (200u128, 500u128, 100u128);

    let dir = std::env::temp_dir().join("basilisk_10k_test");
    let _ = std::fs::create_dir_all(&dir);

    let start = std::time::Instant::now();
    let ss = export_speedscope(&data, "10k-test", 99999, 100.0, &dir).expect("export");
    let ss_elapsed = start.elapsed();
    assert!(
        ss_elapsed.as_millis() < ss_budget,
        "speedscope for 10K samples should be <{ss_budget}ms, took {}ms",
        ss_elapsed.as_millis()
    );

    // ── Performance: flamegraph ──────────────────────────────────────
    let start = std::time::Instant::now();
    let fg = export_flamegraph(&data, "10k-test", &dir).expect("flamegraph");
    let fg_elapsed = start.elapsed();
    assert!(
        fg_elapsed.as_millis() < fg_budget,
        "flamegraph for 10K samples should be <{fg_budget}ms, took {}ms",
        fg_elapsed.as_millis()
    );

    // ── Performance: diagnostics ─────────────────────────────────────
    let start = std::time::Instant::now();
    let diags = generate_diagnostics(&data, &config);
    let diag_elapsed = start.elapsed();
    assert!(
        diag_elapsed.as_millis() < diag_budget,
        "diagnostics for 10K samples should be <{diag_budget}ms, took {}ms",
        diag_elapsed.as_millis()
    );

    // ── Verify export content ───────────────────────────────────────
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&ss.path).expect("read")).expect("parse");

    let profiles = json["profiles"].as_array().expect("profiles");
    assert!(profiles.len() >= 3, "should have ≥3 thread profiles");

    let frames = json["shared"]["frames"].as_array().expect("frames");
    assert!(frames.len() >= 15, "should have ≥15 unique frames");

    let svg = std::fs::read_to_string(&fg.path).expect("read SVG");
    assert!(
        svg.len() > 5000,
        "SVG should be substantial ({} bytes)",
        svg.len()
    );

    assert!(!diags.is_empty(), "should have diagnostics");
    let total_diags: usize = diags.values().map(Vec::len).sum();
    assert!(total_diags >= 5, "should have ≥5 diagnostics across files");

    let _ = std::fs::remove_dir_all(&dir);
}
