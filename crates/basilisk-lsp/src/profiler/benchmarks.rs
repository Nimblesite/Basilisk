//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Performance benchmarks for the profiler pipeline.
//!
//! Validates that key operations meet latency targets for 60K-sample profiles:
//! - Diagnostic generation: <100ms
//! - Speedscope JSON export: <200ms
//! - Flamegraph SVG export: <500ms
//!
//! Uses `std::time::Instant` for wall-clock timing in standard `#[test]` functions.

use std::time::Instant;

use super::aggregator::{FunctionStats, HotspotConfig, ProfileData, SpeedscopeFrame};
use super::diagnostics::generate_diagnostics;
use super::export::{export_flamegraph, export_speedscope};

/// Timing multiplier: debug builds are ~5x slower than release.
#[cfg(debug_assertions)]
const TIMING_MULTIPLIER: u128 = 5;
#[cfg(not(debug_assertions))]
const TIMING_MULTIPLIER: u128 = 1;

/// Build a large `ProfileData` with 60K samples, 200 unique frames, 4 threads.
fn build_large_profile() -> ProfileData {
    let mut data = ProfileData {
        total_samples: 60_000,
        ..ProfileData::default()
    };

    // 200 unique frames across 20 modules.
    for frame_idx in 0..200_u32 {
        data.frames.push(SpeedscopeFrame {
            name: format!("func_{frame_idx}"),
            file: format!("/tmp/bench/module_{}.py", frame_idx / 10),
            line: i32::try_from(frame_idx % 50 + 1).unwrap_or(1),
        });
    }

    // 4 threads, 15K samples each with varying stack depths.
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
        // The per-thread sample count percentages divide by (#251).
        let _ = data.thread_samples.insert(thread_id, 15_000);
    }

    // 100 files with 50 hot lines each + function stats.
    for file_idx in 0..100_u32 {
        let file = format!("/tmp/bench/module_{file_idx:03}.py");
        let line_map = data.line_hits.entry(file.clone()).or_default();
        for line in 1..=50 {
            let _ = line_map.insert(line, u64::from(file_idx + 1) * 12);
        }
        let func_map = data.function_stats.entry(file.clone()).or_default();
        let _ = func_map.insert(
            format!("func_{file_idx}"),
            FunctionStats {
                name: format!("func_{file_idx}"),
                file,
                line: 1,
                total_samples: u64::from(file_idx + 1) * 50,
                self_samples: u64::from(file_idx + 1) * 25,
            },
        );
    }

    data
}

#[test]
fn bench_diagnostic_generation_under_100ms() {
    let data = build_large_profile();
    let config = HotspotConfig::default();

    let start = Instant::now();
    let diags = generate_diagnostics(&data, &config);
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 100 * TIMING_MULTIPLIER,
        "diagnostic generation took {elapsed:?}, target <{}ms",
        100 * TIMING_MULTIPLIER
    );
    assert!(
        !diags.is_empty(),
        "should produce diagnostics from 60K-sample profile"
    );
}

#[test]
fn bench_speedscope_export_under_200ms() -> Result<(), String> {
    let data = build_large_profile();
    let output_dir = std::env::temp_dir().join("basilisk_bench_speedscope");
    let _ = std::fs::create_dir_all(&output_dir);

    let start = Instant::now();
    let result = export_speedscope(&data, "bench-60k", 99999, 600.0, &output_dir)?;
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 200 * TIMING_MULTIPLIER,
        "speedscope export took {elapsed:?}, target <{}ms",
        200 * TIMING_MULTIPLIER
    );
    assert!(result.path.exists());

    let _ = std::fs::remove_file(&result.path);
    let _ = std::fs::remove_dir(&output_dir);
    Ok(())
}

#[test]
fn bench_flamegraph_svg_under_500ms() -> Result<(), String> {
    let data = build_large_profile();
    let output_dir = std::env::temp_dir().join("basilisk_bench_flamegraph");
    let _ = std::fs::create_dir_all(&output_dir);

    let start = Instant::now();
    let result = export_flamegraph(&data, "bench-60k-fg", &output_dir)?;
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 500 * TIMING_MULTIPLIER,
        "flamegraph SVG export took {elapsed:?}, target <{}ms",
        500 * TIMING_MULTIPLIER
    );
    assert!(result.path.exists());

    let svg =
        std::fs::read_to_string(&result.path).map_err(|err| format!("read flamegraph: {err}"))?;
    assert!(svg.contains("<svg"), "output should be valid SVG");

    let _ = std::fs::remove_file(&result.path);
    let _ = std::fs::remove_dir(&output_dir);
    Ok(())
}
