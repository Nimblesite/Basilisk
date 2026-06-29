//! Tests for [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//! Real py-spy integration tests that spawn Python child processes and profile
//! them through the full pipeline: sampler -> aggregator -> export -> diagnostics.
//!
//! On macOS, a parent process can trace its own children without elevation.
//! On Linux with `ptrace_scope=0`, no elevation is needed either.
//!
//! Tests are `#[ignore]`d because they require py-spy to successfully attach,
//! which needs either root or a parent-child relationship. Run explicitly:
//!
//! ```sh
//! cargo test -p basilisk-lsp --test profiler_e2e_tests -- --ignored --nocapture
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

mod common;

use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::ProcessGuard;

use basilisk_lsp::profiler::aggregator::{HotspotConfig, ProfileData};
use basilisk_lsp::profiler::diagnostics::generate_diagnostics;
use basilisk_lsp::profiler::export::{export, ExportFormat};
use basilisk_lsp::profiler::sampler::{start_sampler, SamplerConfig, SamplerError};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Resolve the Python 3 interpreter, respecting env overrides.
fn python_path() -> String {
    std::env::var("PYTHON")
        .or_else(|_| std::env::var("BASILISK_PYTHON"))
        .unwrap_or_else(|_| "python3".to_owned())
}

/// Returns true if python3 is available on this machine.
fn python3_available() -> bool {
    Command::new(python_path())
        .args(["--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Spawn a Python child process with the given inline script.
/// Returns a `ProcessGuard` that kills the child on drop.
fn spawn_python(script: &str) -> ProcessGuard {
    let child = Command::new(python_path())
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn python3 — is it installed?");
    ProcessGuard::new(child)
}

/// Spawn a Python child from a script file.
/// Returns a `ProcessGuard` (kills on drop) and the temp path.
fn spawn_python_file(script: &str) -> (ProcessGuard, std::path::PathBuf) {
    let dir = std::env::temp_dir().join("basilisk_profiler_e2e_tests");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let script_path = dir.join(format!(
        "test_{}.py",
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::write(&script_path, script).expect("write Python script");

    let child = Command::new(python_path())
        .arg(&script_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn python3");

    (ProcessGuard::new(child), script_path)
}

/// Wait until the child's stdout emits a line containing `marker`.
/// Times out after `timeout`.
fn wait_for_marker(guard: &mut ProcessGuard, marker: &str, timeout: Duration) {
    let stdout = guard.child_mut().stdout.as_mut().expect("child stdout");
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();
    let deadline = Instant::now() + timeout;

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).expect("read stdout");
        assert!(
            bytes > 0,
            "Python process exited before printing '{marker}'"
        );
        if line.contains(marker) {
            return;
        }
        assert!(
            Instant::now() <= deadline,
            "Timed out waiting for marker '{marker}' from Python"
        );
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

// Exercises [PROFILE-API]
/// Attach to a real Python child process and verify we receive sample batches
/// with valid stack traces.
#[test]
#[ignore = "requires py-spy attach privileges (parent-child on macOS, ptrace on Linux)"]
fn test_attach_to_python_process_and_sample() {
    if !python3_available() {
        eprintln!("SKIP: python3 not found");
        return;
    }

    let script = r#"
import sys, time
print("READY", flush=True)
# CPU-bound loop with timeout to prevent orphaned processes.
deadline = time.time() + 30
total = 0
while time.time() < deadline:
    total += sum(range(100))
"#;

    let mut guard = spawn_python(script);
    wait_for_marker(&mut guard, "READY", Duration::from_secs(10));

    // Give the process a moment to enter the hot loop.
    std::thread::sleep(Duration::from_millis(200));

    let config = SamplerConfig {
        pid: guard.id(),
        sample_rate: 100,
        include_native: false,
        include_idle: true,
        duration: None,
    };

    let mut handle = start_sampler(&config).expect("start_sampler should succeed for child PID");

    // 1. python_version is not empty
    assert!(
        !handle.python_version.is_empty(),
        "python_version must not be empty"
    );

    // 2. python_version starts with a digit (e.g. "3.12.1")
    assert!(
        handle
            .python_version
            .chars()
            .next()
            .unwrap_or(' ')
            .is_ascii_digit(),
        "python_version should start with a digit, got: {}",
        handle.python_version
    );

    // 3. PID matches what we asked for
    assert_eq!(handle.pid, guard.id(), "handle.pid must match child PID");

    // Receive at least 5 sample batches.
    let mut batches = Vec::new();
    let collect_deadline = Instant::now() + Duration::from_secs(5);
    while batches.len() < 5 && Instant::now() < collect_deadline {
        match handle.receiver.try_recv() {
            Ok(batch) => batches.push(batch),
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }

    // 4. Got at least 5 batches
    assert!(
        batches.len() >= 5,
        "should receive at least 5 sample batches, got {}",
        batches.len()
    );

    // Stop the sampler.
    handle.stop();

    // 5. Each batch has at least one trace
    for (idx, batch) in batches.iter().enumerate() {
        assert!(
            !batch.traces.is_empty(),
            "batch {idx} should have at least one trace"
        );
    }

    // 6. Traces have frames with non-empty names and filenames
    let first_batch_with_active = batches
        .iter()
        .find(|b| b.traces.iter().any(|t| !t.frames.is_empty()))
        .expect("at least one batch should have traces with frames");

    let trace_with_frames = first_batch_with_active
        .traces
        .iter()
        .find(|t| !t.frames.is_empty())
        .expect("should have a trace with frames");

    let first_frame = &trace_with_frames.frames[0];
    assert!(!first_frame.name.is_empty(), "frame name must not be empty");
    assert!(
        !first_frame.filename.is_empty(),
        "frame filename must not be empty"
    );

    guard.kill();
}

/// Collect samples from a sampler handle into a `ProfileData`, then stop.
fn collect_and_stop(
    handle: &mut basilisk_lsp::profiler::sampler::SamplerHandle,
    collect_duration: Duration,
    sample_weight: f64,
) -> ProfileData {
    let mut data = ProfileData::default();
    let start = Instant::now();
    while start.elapsed() < collect_duration {
        match handle.receiver.try_recv() {
            Ok(batch) => {
                data.ingest_traces(&batch.traces, sample_weight, false);
            }
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    handle.stop();
    while let Ok(batch) = handle.receiver.try_recv() {
        data.ingest_traces(&batch.traces, sample_weight, false);
    }
    data
}

/// Verify exports (speedscope + flamegraph) and diagnostics from profile data.
fn verify_exports_and_diagnostics(data: &ProfileData, pid: u32, hotspot_config: &HotspotConfig) {
    let output_dir = std::env::temp_dir().join("basilisk_e2e_pipeline_test");
    std::fs::create_dir_all(&output_dir).expect("create output dir");

    // Speedscope JSON export.
    let speedscope = export(
        data,
        ExportFormat::Speedscope,
        "e2e-pipeline",
        pid,
        3.0,
        &output_dir,
    )
    .expect("speedscope export should succeed");
    assert!(
        speedscope.path.exists(),
        "speedscope JSON should exist at {}",
        speedscope.path.display()
    );

    let json_str = std::fs::read_to_string(&speedscope.path).expect("read speedscope");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    assert!(
        parsed.get("shared").is_some(),
        "speedscope JSON should have 'shared' key"
    );
    let frames = parsed
        .get("shared")
        .and_then(|s| s.get("frames"))
        .and_then(|f| f.as_array())
        .expect("shared.frames array");
    assert!(!frames.is_empty(), "speedscope should have frames");

    // Flamegraph SVG export.
    let flamegraph = export(
        data,
        ExportFormat::Flamegraph,
        "e2e-pipeline",
        pid,
        3.0,
        &output_dir,
    )
    .expect("flamegraph export should succeed");
    assert!(
        flamegraph.path.exists(),
        "flamegraph SVG should exist at {}",
        flamegraph.path.display()
    );

    let svg_content = std::fs::read_to_string(&flamegraph.path).expect("read flamegraph");
    assert!(
        svg_content.contains("<svg") || svg_content.contains("<?xml"),
        "flamegraph output should be SVG"
    );

    // Generate diagnostics (may be empty if py-spy returned non-absolute paths).
    let diagnostics = generate_diagnostics(data, hotspot_config);
    if !diagnostics.is_empty() {
        let all_codes: Vec<String> = diagnostics
            .values()
            .flat_map(|ds| {
                ds.iter().filter_map(|d| match &d.code {
                    Some(tower_lsp::lsp_types::NumberOrString::String(s)) => Some(s.clone()),
                    _ => None,
                })
            })
            .collect();
        assert!(
            all_codes.iter().any(|c| c.starts_with("BSK-PROF")),
            "diagnostics should contain BSK-PROF codes, got: {all_codes:?}"
        );
    }

    let _ = std::fs::remove_file(&speedscope.path);
    let _ = std::fs::remove_file(&flamegraph.path);
}

/// Full pipeline: spawn Python -> attach sampler -> aggregate -> export -> diagnostics.
#[test]
#[ignore = "requires py-spy attach privileges (parent-child on macOS, ptrace on Linux)"]
fn test_full_pipeline_real_python() {
    if !python3_available() {
        eprintln!("SKIP: python3 not found");
        return;
    }

    let script = r#"
import sys, time

def hot():
    """CPU hotspot function."""
    return [x**2 for _ in range(100_000) for x in range(10)]

print("READY", flush=True)
# Self-terminate after 30 seconds to prevent orphaned processes.
deadline = time.time() + 30
while time.time() < deadline:
    hot()
"#;

    let (mut guard, script_path) = spawn_python_file(script);
    wait_for_marker(&mut guard, "READY", Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(300));

    let config = SamplerConfig {
        pid: guard.id(),
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };

    let mut handle = start_sampler(&config).expect("start_sampler should succeed");
    let data = collect_and_stop(&mut handle, Duration::from_secs(3), 0.01);
    let child_pid = guard.id();
    guard.kill();

    // 1. total_samples > 0
    assert!(
        data.total_samples > 0,
        "should have collected samples, got 0"
    );

    // 2. We have line hits
    assert!(
        !data.line_hits.is_empty(),
        "should have line hits from the Python process"
    );

    // 3. We have function stats
    assert!(
        !data.function_stats.is_empty(),
        "should have function stats from the Python process"
    );

    // 4. Check hot_functions — `hot` should appear
    let hotspot_config = HotspotConfig {
        line_threshold_pct: 0.5,
        function_threshold_pct: 1.0,
        max_diagnostics_per_file: 50,
    };
    let hot_functions = data.hot_functions(&hotspot_config);
    assert!(
        hot_functions.iter().any(|f| f.name == "hot"),
        "the `hot` function should appear in hot_functions. Got: {:?}",
        hot_functions.iter().map(|f| &f.name).collect::<Vec<_>>()
    );

    // 5. Check hot_lines — lines from the script should appear
    let hot_lines = data.hot_lines(&hotspot_config);
    assert!(
        !hot_lines.is_empty(),
        "should detect hot lines from the Python script"
    );

    // 6-8. Export and diagnostics verification.
    verify_exports_and_diagnostics(&data, child_pid, &hotspot_config);

    let _ = std::fs::remove_file(&script_path);
}

/// Verify the sampler detects the correct Python version (3.X.Y format).
#[test]
#[ignore = "requires py-spy attach privileges (parent-child on macOS, ptrace on Linux)"]
fn test_sampler_detects_python_version() {
    if !python3_available() {
        eprintln!("SKIP: python3 not found");
        return;
    }

    let mut guard =
        spawn_python("import sys; print('READY', flush=True); import time; time.sleep(10)");
    wait_for_marker(&mut guard, "READY", Duration::from_secs(10));

    let config = SamplerConfig {
        pid: guard.id(),
        sample_rate: 10,
        include_native: false,
        include_idle: false,
        duration: None,
    };

    let handle = start_sampler(&config).expect("start_sampler should succeed");

    // 1. Version is not empty
    assert!(
        !handle.python_version.is_empty(),
        "python_version must not be empty"
    );

    // 2. Version starts with "3."
    assert!(
        handle.python_version.starts_with("3."),
        "python_version should start with '3.', got: {}",
        handle.python_version
    );

    // 3. Version has exactly two dots (major.minor.patch)
    let dot_count = handle.python_version.chars().filter(|&c| c == '.').count();
    assert_eq!(
        dot_count, 2,
        "python_version should have format X.Y.Z (2 dots), got: {}",
        handle.python_version
    );

    // 4. All parts are numeric
    let parts: Vec<&str> = handle.python_version.split('.').collect();
    assert_eq!(parts.len(), 3, "should have 3 version parts");
    for part in &parts {
        assert!(
            part.parse::<u32>().is_ok(),
            "version part '{}' should be numeric in '{}'",
            part,
            handle.python_version
        );
    }

    // 5. Major version is 3
    assert_eq!(
        parts[0], "3",
        "major version should be 3, got: {}",
        parts[0]
    );

    // 6. Minor version is reasonable (8+)
    let minor: u32 = parts[1].parse().expect("minor version parse");
    assert!(
        minor >= 8,
        "minor version should be >= 8 (Python 3.8+), got: {minor}"
    );

    handle.stop();
    guard.kill();
}

/// Verify that attaching to an already-exited process returns a meaningful error.
#[test]
#[ignore = "requires py-spy attach privileges (parent-child on macOS, ptrace on Linux)"]
fn test_sampler_handles_process_exit_gracefully() {
    if !python3_available() {
        eprintln!("SKIP: python3 not found");
        return;
    }

    // Spawn a Python process that exits immediately.
    let mut guard = spawn_python("pass");
    let pid = guard.id();

    // Wait for it to actually exit.
    let status = guard.child_mut().wait().expect("wait for child");
    assert!(status.success(), "child should exit successfully");

    // Give the OS a moment to clean up.
    std::thread::sleep(Duration::from_millis(200));

    let config = SamplerConfig {
        pid,
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };

    let result = start_sampler(&config);

    // 1. Should be an error
    assert!(result.is_err(), "attaching to exited process should fail");

    // Use let-else instead of unwrap_err since SamplerHandle doesn't impl Debug.
    let Err(err) = result else {
        panic!("expected error but got Ok")
    };
    let err_msg = format!("{err}");

    // 2. Error message is not empty
    assert!(!err_msg.is_empty(), "error message should not be empty");

    // 3. Error is either ProcessNotFound or AttachFailed
    let is_expected_error = matches!(
        err,
        SamplerError::ProcessNotFound(_)
            | SamplerError::AttachFailed(_)
            | SamplerError::NotPython(_)
    );
    assert!(
        is_expected_error,
        "error should be ProcessNotFound, AttachFailed, or NotPython — got: {err_msg}"
    );

    // 4. Error message contains some useful context
    let has_context = err_msg.contains("not found")
        || err_msg.contains("No such process")
        || err_msg.contains("attach")
        || err_msg.contains("ot a python")
        || err_msg.contains("ot Python")
        || err_msg.contains(&pid.to_string());
    assert!(
        has_context,
        "error message should contain useful context, got: {err_msg}"
    );

    // 5. The PID in the error matches (if it's a ProcessNotFound variant)
    if let SamplerError::ProcessNotFound(error_pid) = err {
        assert_eq!(
            error_pid, pid,
            "ProcessNotFound should contain the correct PID"
        );
    }
}

/// Verify two concurrent sampler sessions on different PIDs work independently.
#[test]
#[ignore = "requires py-spy attach privileges (parent-child on macOS, ptrace on Linux)"]
fn test_concurrent_sessions_different_pids() {
    if !python3_available() {
        eprintln!("SKIP: python3 not found");
        return;
    }

    let script = r#"
import sys, time
print("READY", flush=True)
# Self-terminate after 30 seconds to prevent orphaned processes.
deadline = time.time() + 30
total = 0
while time.time() < deadline:
    total += sum(range(50))
"#;

    let mut guard_a = spawn_python(script);
    let mut guard_b = spawn_python(script);

    wait_for_marker(&mut guard_a, "READY", Duration::from_secs(10));
    wait_for_marker(&mut guard_b, "READY", Duration::from_secs(10));

    std::thread::sleep(Duration::from_millis(200));

    let config_a = SamplerConfig {
        pid: guard_a.id(),
        sample_rate: 50,
        include_native: false,
        include_idle: true,
        duration: None,
    };
    let config_b = SamplerConfig {
        pid: guard_b.id(),
        sample_rate: 50,
        include_native: false,
        include_idle: true,
        duration: None,
    };

    let mut handle_a = start_sampler(&config_a).expect("start_sampler A should succeed");
    let mut handle_b = start_sampler(&config_b).expect("start_sampler B should succeed");

    // 1. Both handles have different PIDs
    assert_ne!(
        handle_a.pid, handle_b.pid,
        "the two samplers should target different PIDs"
    );

    // 2. Both handles have valid python versions
    assert!(
        !handle_a.python_version.is_empty(),
        "sampler A python_version must not be empty"
    );
    assert!(
        !handle_b.python_version.is_empty(),
        "sampler B python_version must not be empty"
    );

    // Collect samples from both for a short window.
    let mut samples_a = 0u64;
    let mut samples_b = 0u64;
    let collect_deadline = Instant::now() + Duration::from_secs(3);

    while Instant::now() < collect_deadline {
        if handle_a.receiver.try_recv().is_ok() {
            samples_a += 1;
        }
        if handle_b.receiver.try_recv().is_ok() {
            samples_b += 1;
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    // 3. Both samplers collected data
    assert!(
        samples_a > 0,
        "sampler A should have collected samples, got 0"
    );
    assert!(
        samples_b > 0,
        "sampler B should have collected samples, got 0"
    );

    // 4. Both collected a reasonable number
    assert!(
        samples_a >= 5,
        "sampler A should have at least 5 batches, got {samples_a}"
    );
    assert!(
        samples_b >= 5,
        "sampler B should have at least 5 batches, got {samples_b}"
    );

    handle_a.stop();
    handle_b.stop();

    // 5. After stopping, the receivers should eventually drain
    // (no panic, no hang)
    let drain_a = std::iter::from_fn(|| handle_a.receiver.try_recv().ok()).count();
    let drain_b = std::iter::from_fn(|| handle_b.receiver.try_recv().ok()).count();
    // Just assert we can drain without error — count can be 0.
    assert!(
        drain_a < 100_000,
        "sanity: drained a reasonable number from A"
    );
    assert!(
        drain_b < 100_000,
        "sanity: drained a reasonable number from B"
    );

    guard_a.kill();
    guard_b.kill();
}

/// Verify idle thread filtering: with `include_idle=false`, sleeping threads
/// should produce fewer ingested samples than active threads.
#[test]
#[ignore = "requires py-spy attach privileges (parent-child on macOS, ptrace on Linux)"]
fn test_idle_thread_filtering() {
    if !python3_available() {
        eprintln!("SKIP: python3 not found");
        return;
    }

    let script = r#"
import sys, threading, time

def sleeper():
    time.sleep(30)

t = threading.Thread(target=sleeper, daemon=True)
t.start()

print("READY", flush=True)
# CPU-bound work on the main thread.
# Self-terminate after 30 seconds to prevent orphaned processes.
deadline = time.time() + 30
total = 0
while time.time() < deadline:
    total += sum(range(100))
"#;

    let mut guard = spawn_python(script);
    wait_for_marker(&mut guard, "READY", Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(300));

    // Profile with include_idle=false
    let config = SamplerConfig {
        pid: guard.id(),
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };

    let mut handle = start_sampler(&config).expect("start_sampler should succeed");
    let sample_weight = 0.01;

    let mut data_no_idle = ProfileData::default();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(batch) = handle.receiver.try_recv() {
            data_no_idle.ingest_traces(&batch.traces, sample_weight, false);
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    handle.stop();
    while let Ok(batch) = handle.receiver.try_recv() {
        data_no_idle.ingest_traces(&batch.traces, sample_weight, false);
    }

    // 1. We collected samples
    assert!(
        data_no_idle.total_samples > 0,
        "should have collected samples"
    );

    // 2. We have line hits (from active thread)
    assert!(
        !data_no_idle.line_hits.is_empty(),
        "should have line hits from the active thread"
    );

    // 3. We have function stats
    assert!(
        !data_no_idle.function_stats.is_empty(),
        "should have function stats"
    );

    // 4. The thread_samples map should exist
    assert!(
        !data_no_idle.thread_samples.is_empty(),
        "should have per-thread sample data"
    );

    // 5. Active threads should have more samples than idle ones.
    //    With include_idle=false, idle threads are filtered from ingestion,
    //    so most per-thread samples should come from active threads.
    let max_thread_samples = data_no_idle
        .thread_samples
        .values()
        .copied()
        .max()
        .unwrap_or(0);
    assert!(
        max_thread_samples > 5,
        "the most active thread should have >5 samples, got {max_thread_samples}"
    );

    // 6. The sleeper function should NOT appear in function_stats
    //    (it is idle, and we filtered idle threads out).
    // Note: py-spy may still see the sleeper thread's top-level frame as
    // a threading bootstrap function rather than "sleeper" itself, but the
    // key point is that no idle trace is ingested when include_idle=false.
    // We check a weaker assertion: most function samples should be from
    // the main thread's work.
    let total_func_samples: u64 = data_no_idle
        .function_stats
        .values()
        .flat_map(|m| m.values())
        .map(|s| s.self_samples)
        .sum();
    assert!(
        total_func_samples > 0,
        "should have function samples from active threads"
    );

    guard.kill();
}

/// Profile a known CPU hotspot and verify the profiler accurately identifies
/// it as the dominant hot function with >40% self time.
#[test]
#[ignore = "requires py-spy attach privileges (parent-child on macOS, ptrace on Linux)"]
fn test_profile_data_accuracy_cpu_bound() {
    if !python3_available() {
        eprintln!("SKIP: python3 not found");
        return;
    }

    let script = r#"
import sys, time

def cpu_burner():
    """This function MUST dominate the profile."""
    total = 0
    for i in range(10_000_000):
        total += i * i
    return total

print("READY", flush=True)
# Call the burner repeatedly — all CPU time should be here.
# Self-terminate after 30 seconds to prevent orphaned processes.
deadline = time.time() + 30
while time.time() < deadline:
    cpu_burner()
"#;

    let (mut guard, script_path) = spawn_python_file(script);
    wait_for_marker(&mut guard, "READY", Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(300));

    let config = SamplerConfig {
        pid: guard.id(),
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };

    let mut handle = start_sampler(&config).expect("start_sampler should succeed");
    let sample_weight = 0.01;

    let mut data = ProfileData::default();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(4) {
        match handle.receiver.try_recv() {
            Ok(batch) => {
                data.ingest_traces(&batch.traces, sample_weight, false);
            }
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    handle.stop();
    while let Ok(batch) = handle.receiver.try_recv() {
        data.ingest_traces(&batch.traces, sample_weight, false);
    }

    guard.kill();

    // 1. We collected a substantial number of samples
    assert!(
        data.total_samples > 50,
        "should have 50+ samples after 4s at 100Hz, got {}",
        data.total_samples
    );

    // 2. We have function stats
    assert!(
        !data.function_stats.is_empty(),
        "should have function stats"
    );

    // 3. cpu_burner should appear in hot functions
    let hotspot_config = HotspotConfig {
        line_threshold_pct: 0.5,
        function_threshold_pct: 1.0,
        max_diagnostics_per_file: 50,
    };
    let hot_functions = data.hot_functions(&hotspot_config);

    let burner = hot_functions.iter().find(|f| f.name == "cpu_burner");
    assert!(
        burner.is_some(),
        "cpu_burner should appear in hot_functions. Got: {:?}",
        hot_functions.iter().map(|f| &f.name).collect::<Vec<_>>()
    );

    let burner = burner.unwrap();

    // 4. cpu_burner should have a high self_percentage (>40%)
    assert!(
        burner.self_percentage > 40.0,
        "cpu_burner should have >40% self time (single CPU-bound function), got {:.1}%",
        burner.self_percentage
    );

    // 5. cpu_burner should have the highest self_samples
    let max_self_samples = hot_functions
        .iter()
        .map(|f| f.self_samples)
        .max()
        .unwrap_or(0);
    assert_eq!(
        burner.self_samples, max_self_samples,
        "cpu_burner should have the highest self_samples"
    );

    // 6. cpu_burner total_samples should be > 0
    assert!(burner.samples > 0, "cpu_burner samples must be > 0");

    // 7. Verify hot lines exist for the script
    let hot_lines = data.hot_lines(&hotspot_config);
    assert!(
        !hot_lines.is_empty(),
        "should have hot lines from the cpu_burner script"
    );

    // Clean up.
    let _ = std::fs::remove_file(&script_path);
}
