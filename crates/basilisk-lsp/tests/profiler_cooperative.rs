//! Tests for [PROFILE-COOPERATIVE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-COOPERATIVE
//!
//! Real end-to-end coverage of the cooperative in-process sampler — the
//! out-of-the-box CPU path for debug-launched sessions. These tests run the
//! ACTUAL generated injection script inside a real Python process and tail
//! the real sample file. No ptrace, no task ports — so unlike the py-spy
//! attach suites, this runs fully on macOS, Linux, and Windows alike.

#![cfg(unix)]
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs,
    dead_code,
    unused_imports
)]

mod common;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use basilisk_lsp::profiler::aggregator::{HotspotConfig, ProfileData};
use basilisk_lsp::profiler::cooperative::{
    mint_sample_file, sampling_script, start_cooperative_sampler,
};
use basilisk_lsp::profiler::ProfileSessionManager;
use common::ProcessGuard;
use tokio::time::timeout;

/// Path to the Python 3 interpreter.
fn python_path() -> String {
    std::env::var("PYTHON")
        .or_else(|_| std::env::var("BASILISK_PYTHON"))
        .unwrap_or_else(|_| "python3".to_owned())
}

/// Compose the REAL injection script with a CPU-bound main program, exactly
/// as a debuggee would look after the editor's paused-`evaluate` injection.
fn write_instrumented_burner(sample_file: &std::path::Path, rate: u64) -> PathBuf {
    let script = sampling_script(sample_file, rate).expect("script must render");
    let burner = format!(
        "{script}\n\nimport time\n\n\ndef hot_function():\n    total = 0\n    for i in range(1_000_000):\n        total += i * i\n    return total\n\n\ndeadline = time.time() + 30\nwhile time.time() < deadline:\n    hot_function()\n"
    );
    let dir = std::env::temp_dir().join("basilisk_cooperative_e2e");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    // Key the burner name off the sample file's stem: it already carries a
    // process-wide counter, so concurrent tests can never collide (wall-clock
    // nanos alone tie within a microsecond).
    let stem = sample_file
        .file_stem()
        .and_then(|name| name.to_str())
        .expect("sample file must have a stem");
    let path = dir.join(format!("burner-{stem}.py"));
    std::fs::write(&path, burner).expect("write instrumented burner");
    path
}

/// Drain the handle's channel into `ProfileData` until enough ticks arrive.
async fn drain_until(
    handle: &mut basilisk_lsp::profiler::sampler::SamplerHandle,
    minimum_samples: u64,
) -> ProfileData {
    let mut data = ProfileData::default();
    let weight = 1.0 / 100.0;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline && data.total_samples <= minimum_samples {
        while let Ok(batch) = handle.receiver.try_recv() {
            data.ingest_traces(&batch.traces, weight, false);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    data
}

/// THE OOTB test: generated script + real Python + real tail = the full
/// cooperative pipeline attributes the genuine hotspot, on every platform.
#[tokio::test]
async fn cooperative_sampler_attributes_real_hotspots() {
    let sample_file = mint_sample_file();
    let burner = write_instrumented_burner(&sample_file, 100);

    let mut guard = ProcessGuard::new(
        Command::new(python_path())
            .arg(&burner)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn instrumented python"),
    );

    let mut handle = timeout(
        Duration::from_secs(30),
        start_cooperative_sampler(sample_file.clone()),
    )
    .await
    .expect("attach must not hang")
    .expect("the injected sampler must report its header");

    assert!(
        handle.python_version.starts_with("3."),
        "expected Python 3.x, got {}",
        handle.python_version
    );
    assert_eq!(handle.pid, guard.id(), "header pid must be the debuggee's");

    let data = drain_until(&mut handle, 50).await;
    handle.stop();
    guard.kill();
    let _ = std::fs::remove_file(&burner);

    assert!(
        data.total_samples > 50,
        "expected >50 cooperative ticks, got {}",
        data.total_samples
    );
    let hot = data.hot_functions(&HotspotConfig::default());
    assert!(
        hot.iter().any(|f| f.name == "hot_function"),
        "hot_function must be attributed, got {:?}",
        hot.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    let lines = data.hot_lines(&HotspotConfig::default());
    assert!(
        lines.iter().any(|l| {
            std::path::Path::new(&l.file)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
        }),
        "hot lines must carry the real source path"
    );
}

/// Manager-level: a cooperative session flows through the SAME session
/// machinery as py-spy sessions — live progress, then a stop result with
/// hotspot attribution.
#[tokio::test]
async fn manager_runs_cooperative_session_end_to_end() {
    let sample_file = mint_sample_file();
    let burner = write_instrumented_burner(&sample_file, 100);
    let mut guard = ProcessGuard::new(
        Command::new(python_path())
            .arg(&burner)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn instrumented python"),
    );

    let manager = ProfileSessionManager::new();
    let started = timeout(
        Duration::from_secs(30),
        manager.start_cooperative(sample_file, Some(100)),
    )
    .await
    .expect("start must not hang")
    .expect("cooperative session must start");
    assert_eq!(started.pid, guard.id());

    // [PROFILE-NOTIFICATIONS-PROGRESS] feeds off the same probe.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut saw_progress = false;
    while Instant::now() < deadline {
        if let Some(progress) = manager.progress(&started.session_id).await {
            if progress.sample_count > 0 {
                saw_progress = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(saw_progress, "live progress must report a non-zero count");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let stopped = manager
        .stop(&started.session_id)
        .await
        .expect("stop must succeed");
    guard.kill();
    let _ = std::fs::remove_file(&burner);

    assert!(stopped.total_samples > 0, "stop must report real samples");
    assert!(
        stopped
            .hot_functions
            .iter()
            .any(|f| f.name == "hot_function"),
        "the stop result must attribute hot_function, got {:?}",
        stopped
            .hot_functions
            .iter()
            .map(|f| &f.name)
            .collect::<Vec<_>>()
    );
}

/// Without an injected sampler the attach must fail with an actionable
/// message — not hang or report a phantom session.
#[tokio::test]
async fn cooperative_attach_without_injection_fails_actionably() {
    let sample_file = mint_sample_file();
    let err = timeout(
        Duration::from_secs(30),
        start_cooperative_sampler(sample_file),
    )
    .await
    .expect("must not hang")
    .expect_err("no header can ever arrive");
    assert!(
        err.to_string().contains("never reported its header"),
        "the failure must say what went wrong: {err}"
    );
}
