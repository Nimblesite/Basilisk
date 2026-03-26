//! Integration tests for the profiler pipeline: ingestion → export → diagnostics.
//!
//! These tests construct `ProfileData` manually (no py-spy process needed)
//! and verify the full pipeline works end-to-end: aggregation, speedscope
//! export, flamegraph SVG export, diagnostic generation, and cleanup.

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
