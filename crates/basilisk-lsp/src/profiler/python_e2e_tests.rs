//! Implements [PROFILE-COOPERATIVE] / [PROFILE-MEMORY-FINAL] e2e tests that
//! execute the REAL injected Python scripts in a real `python3` — the scripts
//! the editor evaluates inside a debuggee. Synthetic `py_spy::StackTrace`
//! tests (`scenario_tests.rs`) cannot catch bugs in the Python source itself:
//! misclassification, handler clobbering, or wire anomalies only show up when
//! the genuine script runs against a genuine interpreter, the way profiling a
//! real GitHub project does.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::cooperative::{sampling_script, start_cooperative_sampler};
use super::memory::scripts::{diff_snapshot, start_tracemalloc, store_baseline};

/// A unique scratch directory for one test.
fn mint_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "bsk_pye2e_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Run `python3` over a driver program, returning (exit-code, stdout).
/// A traceback goes to stderr — fold it into the returned text so a failing
/// assertion shows WHY the driver died instead of an empty string.
fn run_python(driver: &Path) -> (Option<i32>, String) {
    // -u: a signal-killed or os._exit-ing driver must not lose buffered stdout.
    let output = Command::new("python3")
        .arg("-u")
        .arg(driver)
        .output()
        .expect("python3 must be runnable (make setup installs it)");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        text.push_str("\n--- stderr ---\n");
        text.push_str(&stderr);
    }
    (output.status.code(), text)
}

/// Parse every tick line of a cooperative sample file into
/// `(filename, active)` pairs per frame.
fn frames_in_sample_file(sample_file: &Path) -> Vec<(String, bool)> {
    let text = std::fs::read_to_string(sample_file).expect("sample file must exist");
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(ticks) = value.get("ticks").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for entry in ticks {
            let active = entry
                .get(1)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let Some(frames) = entry.get(2).and_then(serde_json::Value::as_array) else {
                continue;
            };
            for frame in frames {
                if let Some(file) = frame.get(0).and_then(serde_json::Value::as_str) {
                    out.push((file.to_owned(), active));
                }
            }
        }
    }
    out
}

/// Drive the REAL sampling script over spin threads whose `co_filename`s are
/// hostile (set via `compile()`, no files needed), then return the captured
/// frames. The driver runs the sampler exactly as the courier would: `exec`.
fn sample_hostile_workload(tag: &str, filenames: &[&str]) -> Vec<(String, bool)> {
    let dir = mint_dir(tag);
    let sample_file = dir.join("samples.jsonl");
    let script = sampling_script(&sample_file, 100).expect("render script");
    let inject = dir.join("inject.py");
    std::fs::write(&inject, script).expect("write inject");

    let spin_files = filenames
        .iter()
        .map(|f| format!("{f:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let driver_src = format!(
        r#"
import threading, time
exec(open({inject:?}).read())
SRC = "def spin():\n    x = 0\n    while True:\n        x += 1\n"
for fname in ({spin_files},):
    g = {{}}
    exec(compile(SRC, fname, "exec"), g)
    threading.Thread(target=g["spin"], daemon=True).start()
time.sleep(1.2)
open({sample_file:?} + ".stop", "w").write("stop")
time.sleep(0.4)
"#,
        inject = inject.display().to_string(),
        sample_file = sample_file.display().to_string(),
    );
    let driver = dir.join("driver.py");
    std::fs::write(&driver, driver_src).expect("write driver");

    let (code, stdout) = run_python(&driver);
    assert_eq!(code, Some(0), "driver must exit cleanly; stdout: {stdout}");
    assert!(
        stdout.contains("__BASILISK_CPU_ACK__"),
        "the injected script must ack; stdout: {stdout}"
    );

    let frames = frames_in_sample_file(&sample_file);
    let _ = std::fs::remove_dir_all(&dir);
    frames
}

// [PROFILE-COOPERATIVE] A user's hot code in a file whose NAME merely ends
// like a stdlib wait module (websocket.py, task_queue.py, openssl.py) must be
// sampled as ACTIVE — a spinning loop is not a waiting thread. The real-world
// repro: a 4-second pygments+spin run classified 755/755 samples of a
// /realapp/websocket.py busy loop as idle, making the thread invisible.
#[test]
fn sampler_script_keeps_user_files_named_like_wait_modules_active() {
    let frames = sample_hostile_workload(
        "waitnames",
        &["/realapp/websocket.py", "/realapp/task_queue.py"],
    );

    for hostile in ["/realapp/websocket.py", "/realapp/task_queue.py"] {
        let seen: Vec<&(String, bool)> = frames.iter().filter(|(f, _)| f == hostile).collect();
        assert!(
            !seen.is_empty(),
            "{hostile} must be sampled at all (captured files: {:?})",
            frames
                .iter()
                .map(|(f, _)| f)
                .collect::<std::collections::HashSet<_>>()
        );
        assert!(
            seen.iter().any(|(_, active)| *active),
            "{hostile} is a busy spin loop — it must be sampled ACTIVE, not misread as a stdlib waiter"
        );
    }
}

// [PROFILE-COOPERATIVE] Only genuine debugger machinery may be stripped from
// the leaf: matching must be ANCHORED (basename prefix / exact path segment),
// mirroring the Rust-side `is_runtime_scaffolding`. A user path that merely
// CONTAINS "pydevd"/"debugpy" is the user's own hot code.
#[test]
fn sampler_script_keeps_user_paths_that_merely_contain_debugger_names() {
    let hostile = "/home/dev/my_pydevd_tools/hot.py";
    let frames = sample_hostile_workload("dbgnames", &[hostile]);

    assert!(
        frames.iter().any(|(f, _)| f == hostile),
        "a user path containing 'pydevd' as a substring must NOT be stripped \
         (captured files: {:?})",
        frames
            .iter()
            .map(|(f, _)| f)
            .collect::<std::collections::HashSet<_>>()
    );
}

/// Append raw bytes to the sample file, as the debuggee's buffered writer
/// would during a flush.
fn append(sample_file: &Path, bytes: &[u8]) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(sample_file)
        .expect("open sample file");
    file.write_all(bytes).expect("append");
    file.flush().expect("flush");
}

// [PROFILE-COOPERATIVE] A tick record split across two writes (the debuggee's
// 8 KB text buffer flushing mid-line under many threads / deep stacks) must
// still be captured: the tailer has to wait for the line's newline, never
// parse-and-drop the two halves.
#[tokio::test(flavor = "multi_thread")]
async fn cooperative_tailer_recovers_a_tick_split_across_writes() {
    let dir = mint_dir("splitline");
    let sample_file = dir.join("samples.jsonl");
    append(
        &sample_file,
        b"{\"header\":{\"python\":\"3.12.0\",\"pid\":4242}}\n",
    );

    let mut handle = start_cooperative_sampler(sample_file.clone())
        .await
        .expect("attach to the sample file");

    // First tick arrives in TWO writes, cut mid-record.
    append(
        &sample_file,
        b"{\"ticks\":[[1,true,[[\"/app/hot.py\",7,\"spin\"]",
    );
    tokio::time::sleep(Duration::from_millis(350)).await;
    append(&sample_file, b"]]]}\n");
    // Second tick arrives whole.
    append(
        &sample_file,
        b"{\"ticks\":[[1,true,[[\"/app/other.py\",9,\"work\"]]]]}\n",
    );

    // Drain whatever arrives (bounded), so a dropped tick shows up as a
    // missing FILE below — not as an ambiguous recv timeout.
    let mut seen_files = std::collections::HashSet::new();
    while seen_files.len() < 2 {
        match tokio::time::timeout(Duration::from_secs(5), handle.receiver.recv()).await {
            Ok(Some(batch)) => {
                for trace in &batch.traces {
                    for frame in &trace.frames {
                        let _ = seen_files.insert(frame.filename.clone());
                    }
                }
            }
            Ok(None) | Err(_) => break, // channel closed or nothing more coming
        }
    }

    assert!(
        seen_files.contains("/app/hot.py"),
        "the split tick must be recovered once its newline lands (got: {seen_files:?})"
    );
    assert!(
        seen_files.contains("/app/other.py"),
        "the following whole tick must arrive too (got: {seen_files:?})"
    );

    handle.join();
    let _ = std::fs::remove_dir_all(&dir);
}

// [PROFILE-MEMORY-FINAL] Installing the final-snapshot SIGTERM/SIGINT hook
// must not CLOBBER the application's own handler: after the snapshot is
// written, the app's graceful-shutdown handler still runs. A server that
// flushes state on SIGTERM keeps working under memory tracking.
#[test]
fn final_snapshot_signal_hook_chains_the_apps_own_handler() {
    let dir = mint_dir("sigchain");
    let snapshot_file = dir.join("final.json");
    let flag_file = dir.join("app_handler_ran.flag");
    let inject = dir.join("inject.py");
    std::fs::write(
        &inject,
        start_tracemalloc(10, &snapshot_file.display().to_string(), 50),
    )
    .expect("write inject");

    let driver_src = format!(
        r#"
import os, signal, sys, time

def app_handler(signum, frame):
    # The application's own graceful shutdown: persist state, then exit.
    open({flag:?}, "w").write("ran")
    os._exit(7)

signal.signal(signal.SIGTERM, app_handler)
exec(open({inject:?}).read())
data = [bytes(2048) for _ in range(200)]
os.kill(os.getpid(), signal.SIGTERM)
time.sleep(5)
sys.exit(1)  # unreachable if the handler chain works
"#,
        flag = flag_file.display().to_string(),
        inject = inject.display().to_string(),
    );
    let driver = dir.join("driver.py");
    std::fs::write(&driver, driver_src).expect("write driver");

    let (code, stdout) = run_python(&driver);
    assert!(
        stdout.contains("__BASILISK_MEM_OK__"),
        "tracking must start; stdout: {stdout}"
    );
    assert!(
        snapshot_file.exists(),
        "the final snapshot must still be captured on SIGTERM"
    );
    assert!(
        flag_file.exists(),
        "the app's own SIGTERM handler must still run after the snapshot — \
         the injected hook may not clobber it ([PROFILE-MEMORY-FINAL])"
    );
    assert_eq!(
        code,
        Some(7),
        "the process must die the way the APP's handler decided (exit 7)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// Memory diffs must report SHRINKING sites, not only growth: total_freed /
// net_growth are lies otherwise, and a cache that provably releases memory
// reads like a one-way leak. ([PROFILE-MEMORY-CONFIDENCE] input contract.)
#[test]
fn diff_script_reports_shrinking_allocations() {
    let dir = mint_dir("shrink");
    let baseline = dir.join("baseline.py");
    std::fs::write(&baseline, store_baseline()).expect("write baseline script");
    let diff = dir.join("diff.py");
    std::fs::write(&diff, diff_snapshot(50)).expect("write diff script");

    let driver_src = format!(
        r"
import gc, tracemalloc
tracemalloc.start(5)
big = [bytes(4096) for _ in range(2000)]  # ~8 MB retained
exec(open({baseline:?}).read())
del big
gc.collect()
exec(open({diff:?}).read())
",
        baseline = baseline.display().to_string(),
        diff = diff.display().to_string(),
    );
    let driver = dir.join("driver.py");
    std::fs::write(&driver, driver_src).expect("write driver");

    let (code, stdout) = run_python(&driver);
    assert_eq!(code, Some(0), "driver must exit cleanly; stdout: {stdout}");

    // The diff script ferries its payload through a temp file
    // ([PROFILE-MEMORY-COURIER]): __BASILISK_MEM_FILE__<path> on stdout.
    let payload_path = stdout
        .lines()
        .find_map(|l| l.strip_prefix("__BASILISK_MEM_FILE__"))
        .expect("diff must emit a payload file marker");
    let payload = std::fs::read_to_string(payload_path.trim()).expect("read payload");
    let json = payload
        .strip_prefix("__BASILISK_MEM_DIFF__")
        .expect("payload must carry the diff marker");

    let parsed = super::memory::diff::parse_diff_output(json).expect("parse diff");
    assert!(
        !parsed.freed_allocations.is_empty(),
        "an ~8 MB allocation was freed between snapshots — the diff must report \
         the shrinking site, not silently drop it"
    );
    assert!(
        parsed.total_freed > 1024 * 1024,
        "total_freed must reflect the released megabytes, got {} bytes",
        parsed.total_freed
    );

    let _ = std::fs::remove_file(payload_path.trim());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The render worker must still deliver when `os.replace` refuses.
///
/// On Windows a rename onto a path another process has open fails with
/// `PermissionError`, and the editor polls the reserved courier path every
/// 100 ms while the worker renders — so the collision is designed in. The
/// `os.replace` used to sit OUTSIDE the worker's `try`, so one collision killed
/// the thread, the reservation stayed empty for the editor's whole 60 s wait,
/// and ingest rejected the bare marker line as "no recognized marker": a
/// win32-only flake that read as a broken injection script
/// ([VSIX-CI-PLATFORM-COVERAGE], [PROFILE-MEMORY-COURIER]).
///
/// POSIX `os.replace` never fails that way, so the collision is injected rather
/// than waited for: the driver wraps `os.replace` to raise `PermissionError` a
/// fixed number of times before delegating. `refusals=3` proves the retry wins,
/// and `refusals` beyond the 5 s budget (a permanent refusal) proves the
/// direct-write fallback still lands the payload. Both must produce the SAME
/// marker-carrying payload the un-refused path does.
#[test]
fn diff_payload_lands_even_when_os_replace_is_refused() {
    for (tag, refusals) in [("retry", 3usize), ("always", usize::MAX)] {
        let dir = mint_dir(tag);
        let baseline = dir.join("baseline.py");
        std::fs::write(&baseline, store_baseline()).expect("write baseline script");
        let diff = dir.join("diff.py");
        std::fs::write(&diff, diff_snapshot(50)).expect("write diff script");

        // A permanent refusal must not burn the worker's full 5 s retry budget
        // in every run, so `always` is spelled as a huge count, not a flag: the
        // wrapper still counts down and the fallback is reached the same way.
        let driver_src = format!(
            r"
import gc, os, tracemalloc
_real_replace = os.replace
_left = {refusals}
def _refusing_replace(src, dst):
    global _left
    if _left > 0:
        _left -= 1
        raise PermissionError(13, 'simulated win32 sharing violation')
    return _real_replace(src, dst)
os.replace = _refusing_replace

tracemalloc.start(5)
big = [bytes(4096) for _ in range(2000)]
exec(open({baseline:?}).read())
del big
gc.collect()
exec(open({diff:?}).read())
",
            refusals = if refusals == usize::MAX {
                "10**9".to_owned()
            } else {
                refusals.to_string()
            },
            baseline = baseline.display().to_string(),
            diff = diff.display().to_string(),
        );
        let driver = dir.join("driver.py");
        std::fs::write(&driver, driver_src).expect("write driver");

        let (code, stdout) = run_python(&driver);
        assert_eq!(
            code,
            Some(0),
            "[{tag}] driver must exit cleanly; stdout: {stdout}"
        );

        let payload_path = stdout
            .lines()
            .find_map(|line| line.strip_prefix("__BASILISK_MEM_FILE__"))
            .unwrap_or_else(|| panic!("[{tag}] diff must emit a payload file marker: {stdout}"))
            .trim()
            .to_owned();
        let payload = std::fs::read_to_string(&payload_path)
            .unwrap_or_else(|error| panic!("[{tag}] read payload: {error}"));
        assert!(
            !payload.is_empty(),
            "[{tag}] a refused os.replace must never leave the reservation empty — \
             that is the win32 hang the editor cannot recover from"
        );
        let json = payload
            .strip_prefix("__BASILISK_MEM_DIFF__")
            .unwrap_or_else(|| {
                panic!("[{tag}] payload must carry the diff marker, got: {payload}")
            });
        let parsed = super::memory::diff::parse_diff_output(json)
            .unwrap_or_else(|error| panic!("[{tag}] parse diff: {error}"));
        assert!(
            parsed.total_freed > 1024 * 1024,
            "[{tag}] the delivered payload must be the real diff, got {} bytes freed",
            parsed.total_freed
        );

        let _ = std::fs::remove_file(&payload_path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
