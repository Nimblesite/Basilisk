//! Tests for [PROFILE-HELPER-SOCKET]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-HELPER-SOCKET
//!
//! Real end-to-end coverage of the elevated-helper profiling path — the surface
//! that issue #61 proved was 0% covered (no `UnixListener`, helper connects to a
//! socket nobody listens on → `os error 2`).
//!
//! These tests drive the **actual** `basilisk-profiler-helper` binary over a
//! real Unix socket — no mocks — and assert the full protocol:
//! `attach → attached → samples → stop → stopped`, with real py-spy samples and
//! correct hotspot attribution.
//!
//! Non-elevated by design (so they run in CI with no `sudo`): the helper attaches
//! to a Python process that is its own **child** (parent-may-trace-child holds on
//! macOS and on Linux regardless of `ptrace_scope`), set up via an `exec` wrapper.

#![cfg(unix)]
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs,
    dead_code,
    unused_imports
)]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use basilisk_profiler_protocol::{read_message, write_message, Command as Cmd, Message};
use common::ProcessGuard;
use tokio::io::BufReader;
use tokio::net::UnixListener;
use tokio::time::timeout;

/// Path to the Python 3 interpreter.
fn python_path() -> String {
    std::env::var("PYTHON")
        .or_else(|_| std::env::var("BASILISK_PYTHON"))
        .unwrap_or_else(|_| "python3".to_owned())
}

/// Monotonic-ish unique suffix for temp paths (avoids cross-test collisions).
fn unique_suffix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{nanos}-{seq}", std::process::id())
}

/// A short `/tmp` socket path — macOS caps Unix socket paths near 104 bytes.
fn unique_socket_path() -> PathBuf {
    PathBuf::from(format!("/tmp/bsk-helper-test-{}.sock", unique_suffix()))
}

/// Locate the built `basilisk-profiler-helper` binary, building it if absent.
fn helper_binary_path() -> PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut cursor = exe.parent();
    while let Some(dir) = cursor {
        candidates.push(dir.join("basilisk-profiler-helper"));
        cursor = dir.parent();
        if candidates.len() > 6 {
            break;
        }
    }
    if let Some(found) = candidates.iter().find(|c| c.exists()) {
        return found.clone();
    }

    // Fallback for `cargo test -p basilisk-lsp` without a prior workspace build.
    let status = Command::new(env!("CARGO"))
        .args(["build", "-p", "basilisk-profiler-helper"])
        .status()
        .expect("run cargo build for helper");
    assert!(status.success(), "failed to build basilisk-profiler-helper");

    if let Some(found) = candidates.iter().find(|c| c.exists()) {
        return found.clone();
    }
    let debug = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/basilisk-profiler-helper");
    assert!(
        debug.exists(),
        "basilisk-profiler-helper not found after build; looked at {candidates:?} and {}",
        debug.display()
    );
    debug
}

/// Write a CPU-bound Python burner that spends ~all its time in `hot_function`.
///
/// On Linux it opts into being traced by any process (`PR_SET_PTRACER_ANY`) so a
/// sibling helper can attach even under `ptrace_scope=1`.
fn write_burner_script() -> PathBuf {
    let dir = std::env::temp_dir().join("basilisk_helper_socket_e2e");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(format!("burner-{}.py", unique_suffix()));
    let script = r#"
import sys
import time

try:
    import ctypes
    # PR_SET_PTRACER = 0x59616d61, PR_SET_PTRACER_ANY = -1 (allow any tracer; Linux Yama).
    _libc = ctypes.CDLL("libc.so.6", use_errno=True)
    _libc.prctl(0x59616d61, ctypes.c_ulong(0xFFFFFFFFFFFFFFFF), 0, 0, 0)
except Exception:
    pass


def hot_function():
    total = 0
    for i in range(1_000_000):
        total += i * i
    return total


def main():
    print("READY", flush=True)
    deadline = time.time() + 30
    while time.time() < deadline:
        hot_function()


if __name__ == "__main__":
    main()
"#;
    std::fs::write(&path, script).expect("write burner script");
    path
}

/// Block until the wrapper records the Python child's PID, then return it.
async fn wait_for_pid(pidfile: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(contents) = std::fs::read_to_string(pidfile) {
            if let Ok(pid) = contents.trim().parse::<u32>() {
                if pid != 0 {
                    return pid;
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the wrapper to record the Python PID"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Wait for the Python burner to print READY on its stdout.
fn wait_for_ready(guard: &mut ProcessGuard) {
    use std::io::BufRead;
    let stdout = guard.child_mut().stdout.as_mut().expect("stdout");
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).expect("read stdout");
        assert!(bytes > 0, "Python exited before printing READY");
        if line.trim() == "READY" {
            return;
        }
        assert!(Instant::now() <= deadline, "timed out waiting for READY");
    }
}

/// Force-kill a PID (the Python child is not a direct `Child` handle here).
fn kill_pid(pid: u32) {
    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
}

/// Cross-platform proof of the #61 fix (Defect 1): the LSP binds a
/// `UnixListener` **before** spawning the helper, so the helper always finds
/// something to connect to.
///
/// We point the real helper at a non-Python process. On the fixed code the
/// helper connects (listener present) and only then fails to attach — surfacing
/// as "closed the connection before confirming attach". On `main` (no listener)
/// the helper would die with `os error 2` and never connect, so the driver would
/// instead report a connection timeout — which this test asserts it does NOT.
///
/// This runs on macOS too (where py-spy cannot attach non-elevated), because it
/// asserts the *connection*, not a successful attach.
#[tokio::test]
async fn helper_listener_is_bound_before_helper_connects() {
    use basilisk_lsp::profiler::helper_client::{start_helper_sampler, HelperSpawn};
    use basilisk_lsp::profiler::sampler::SamplerConfig;

    let helper = helper_binary_path();

    // A live, non-Python target: the helper will connect, then fail to attach.
    let mut sleeper = ProcessGuard::new(
        Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep"),
    );
    let pid = sleeper.id();

    let config = SamplerConfig {
        pid,
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };

    let result = timeout(
        Duration::from_secs(30),
        start_helper_sampler(&config, HelperSpawn::Direct(helper)),
    )
    .await
    .expect("start_helper_sampler must not hang");
    sleeper.kill();

    let err = result.expect_err("attaching to a non-Python process must fail");
    let msg = err.to_string();
    // The helper must CONNECT (listener bound) and then fail the ATTACH —
    // since #81 it reports the structured py-spy cause over the socket.
    assert!(
        msg.contains("py-spy attach failed"),
        "the helper must connect (listener bound) then report its attach failure; got: {msg}"
    );
    assert!(
        !msg.contains("timed out waiting for the profiler helper to connect"),
        "a connection timeout means no listener was bound — the #61 bug: {msg}"
    );
    assert!(
        !msg.contains("closed the connection before confirming attach"),
        "a bare EOF is the undiagnosable #81 failure mode: {msg}"
    );
}

/// THE failing-without-fix test: drive the real helper binary over a real socket
/// through the entire protocol, asserting real samples and hotspot attribution.
///
/// On `main` (no `UnixListener` anywhere in the LSP) the helper connects to a
/// socket nobody listens on and dies with `os error 2`; here the socket is bound
/// before the helper spawns, so the full handshake succeeds.
///
/// Linux-only: it needs a successful py-spy attach, which on macOS requires root
/// even for a child (the very subject of #61 — see the elevated `osascript` path
/// in `helper_client`). CI runs on Linux, where this exercises the real samples.
/// Read `samples` batches until the deadline (or ~60), reporting how many
/// arrived and whether `hot_function` appeared.
#[cfg(target_os = "linux")]
async fn collect_sample_batches(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
) -> (u64, bool) {
    let mut batches = 0u64;
    let mut saw_hot = false;
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline && batches <= 60 {
        match timeout(Duration::from_secs(5), read_message::<_, Message>(reader)).await {
            Ok(Ok(Some(Message::Samples { traces }))) => {
                batches += 1;
                if traces
                    .iter()
                    .any(|t| t.frames.iter().any(|f| f.name == "hot_function"))
                {
                    saw_hot = true;
                }
            }
            Ok(Ok(Some(Message::Attached { .. }))) => {}
            Ok(Ok(Some(Message::Error { kind, message }))) => {
                panic!("helper reported an error mid-sampling: {kind:?}: {message}")
            }
            Ok(Ok(Some(Message::Stopped) | None)) => break,
            Ok(Err(err)) => panic!("sample read failed: {err}"),
            Err(_elapsed) => break,
        }
    }
    (batches, saw_hot)
}

/// Drain messages after sending `stop`, returning whether the helper confirmed
/// `stopped` before closing.
#[cfg(target_os = "linux")]
async fn drain_until_stopped(reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match timeout(Duration::from_secs(5), read_message::<_, Message>(reader)).await {
            Ok(Ok(Some(Message::Stopped))) => return true,
            Ok(Ok(Some(_))) => {}
            Ok(Ok(None) | Err(_)) | Err(_) => break,
        }
    }
    false
}

// Exercises [PROFILE-HELPER-PROTOCOL]
#[cfg(target_os = "linux")]
#[tokio::test]
async fn helper_socket_full_protocol_with_real_binary() {
    let helper = helper_binary_path();
    let burner = write_burner_script();
    let socket = unique_socket_path();
    let pidfile = std::env::temp_dir().join(format!("bsk-helper-pid-{}.txt", unique_suffix()));
    let _ = std::fs::remove_file(&socket);
    let _ = std::fs::remove_file(&pidfile);

    // Bind BEFORE spawning the helper — the crux of the #61 fix.
    let listener = UnixListener::bind(&socket).expect("bind profiler socket");

    // sh: launch Python in the background, record its PID, then `exec` the helper
    // so the helper REPLACES the shell (same PID) and becomes Python's parent.
    let mut wrapper = ProcessGuard::new(
        Command::new("sh")
            .arg("-c")
            .arg(r#""$1" "$2" & echo $! > "$3"; exec "$4" "$5""#)
            .arg("sh")
            .arg(python_path())
            .arg(&burner)
            .arg(&pidfile)
            .arg(&helper)
            .arg(&socket)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn helper wrapper"),
    );

    // Accept proves the listener exists (Defect 1) — the helper connected.
    let (stream, _addr) = timeout(Duration::from_secs(20), listener.accept())
        .await
        .expect("helper should connect to the bound socket")
        .expect("accept");
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    let py_pid = wait_for_pid(&pidfile).await;
    // Let the interpreter finish initializing so py-spy can read it.
    tokio::time::sleep(Duration::from_millis(800)).await;

    // attach
    write_message(
        &mut write_half,
        &Cmd::Attach {
            pid: py_pid,
            rate: Some(200),
            native: Some(false),
        },
    )
    .await
    .expect("send attach");

    // attached{pid, python}
    let attached: Option<Message> = timeout(Duration::from_secs(20), read_message(&mut reader))
        .await
        .expect("attached confirmation in time")
        .expect("read attached");
    let python = match attached {
        Some(Message::Attached { pid, python }) => {
            assert_eq!(pid, py_pid, "attached PID must match");
            python
        }
        other => panic!("expected 'attached', got {other:?}"),
    };
    assert!(
        python.starts_with("3."),
        "expected Python 3.x, got {python}"
    );

    // samples (repeating) — collect a healthy number with the hot frame present.
    let (batches, saw_hot) = collect_sample_batches(&mut reader).await;
    assert!(batches > 50, "expected >50 sample batches, got {batches}");
    assert!(saw_hot, "hot_function must appear in the streamed samples");

    // stop -> stopped
    write_message(&mut write_half, &Cmd::Stop)
        .await
        .expect("send stop");
    assert!(
        drain_until_stopped(&mut reader).await,
        "helper must confirm 'stopped' before exiting"
    );

    kill_pid(py_pid);
    wrapper.kill();
    let _ = std::fs::remove_file(&socket);
    let _ = std::fs::remove_file(&pidfile);
    let _ = std::fs::remove_file(&burner);
}

/// Drive the LSP's own listener + driver (`start_helper_sampler`) against the
/// real helper binary and assert it produces real samples with correct hotspot
/// attribution — the in-LSP equivalent of the protocol test above.
///
/// Linux-only: it relies on a sibling helper tracing the Python child, which the
/// burner enables via `PR_SET_PTRACER_ANY`. On macOS a non-child, non-root
/// process cannot be traced at all (that is the very bug), so the elevated
/// `osascript` path — exercised manually — is the only non-test route there.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn start_helper_sampler_profiles_real_python() {
    use basilisk_lsp::profiler::aggregator::{HotspotConfig, ProfileData};
    use basilisk_lsp::profiler::helper_client::{start_helper_sampler, HelperSpawn};
    use basilisk_lsp::profiler::sampler::SamplerConfig;

    let helper = helper_binary_path();
    let burner = write_burner_script();

    let mut guard = ProcessGuard::new(
        Command::new(python_path())
            .arg(&burner)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn python"),
    );
    let pid = guard.id();
    wait_for_ready(&mut guard);
    tokio::time::sleep(Duration::from_millis(300)).await;

    let config = SamplerConfig {
        pid,
        sample_rate: 200,
        include_native: false,
        include_idle: false,
        duration: None,
    };

    let mut handle = start_helper_sampler(&config, HelperSpawn::Direct(helper))
        .await
        .expect("start_helper_sampler must attach over the socket");
    assert_eq!(handle.pid, pid);
    assert!(
        handle.python_version.starts_with("3."),
        "expected Python 3.x, got {}",
        handle.python_version
    );

    let mut data = ProfileData::default();
    let weight = 1.0 / 200.0;
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline && data.total_samples <= 60 {
        while let Ok(batch) = handle.receiver.try_recv() {
            data.ingest_traces(&batch.traces, weight, false);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    while let Ok(batch) = handle.receiver.try_recv() {
        data.ingest_traces(&batch.traces, weight, false);
    }

    handle.stop();
    guard.kill();
    let _ = std::fs::remove_file(&burner);

    assert!(
        data.total_samples > 50,
        "expected >50 samples over the helper socket, got {}",
        data.total_samples
    );
    let hot = data.hot_functions(&HotspotConfig::default());
    assert!(
        hot.iter().any(|f| f.name == "hot_function"),
        "hot_function must be attributed, got {:?}",
        hot.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
}

/// A missing helper binary must surface a clear spawn error (and the driver must
/// bind, then tear down the socket cleanly) rather than hang. Cross-platform.
#[tokio::test]
async fn missing_helper_binary_surfaces_spawn_error() {
    use basilisk_lsp::profiler::helper_client::{start_helper_sampler, HelperSpawn};
    use basilisk_lsp::profiler::sampler::SamplerConfig;

    let config = SamplerConfig {
        pid: std::process::id(),
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };
    let bogus = PathBuf::from("/nonexistent/basilisk-profiler-helper-xyzzy");
    let err = timeout(
        Duration::from_secs(10),
        start_helper_sampler(&config, HelperSpawn::Direct(bogus)),
    )
    .await
    .expect("must not hang")
    .expect_err("a missing helper binary must fail fast");
    assert!(
        err.to_string().contains("failed to spawn profiler helper"),
        "expected a spawn error, got: {err}"
    );
}

// Exercises [PROFILE-PERMISSIONS-MACOS] (elevated helper spawn is macOS-only)
/// Off macOS, `HelperSpawn::Elevated` has no supported path and must surface a
/// clear error (the socket is still bound first, so this also exercises the
/// driver's spawn-failure teardown).
#[cfg(not(target_os = "macos"))]
#[tokio::test]
async fn elevated_spawn_is_macos_only() {
    use basilisk_lsp::profiler::helper_client::{start_helper_sampler, HelperSpawn};
    use basilisk_lsp::profiler::sampler::SamplerConfig;

    let config = SamplerConfig {
        pid: std::process::id(),
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };
    let err = start_helper_sampler(&config, HelperSpawn::Elevated)
        .await
        .expect_err("elevation is unsupported off macOS");
    assert!(
        err.to_string().contains("only supported on macOS"),
        "expected a macOS-only error, got: {err}"
    );
}

// ── Issue #81: attach failures must be diagnosable ───────────────────────────
//
// Tests for [PROFILE-HELPER-PROTOCOL-ERRORS]. The bug: when py-spy
// attach fails inside the helper, the helper exits without reporting anything
// over the socket, the LSP reads a clean EOF, and the user sees the bare
// "helper closed the connection before confirming attach" with no exit code,
// no stderr, and no underlying cause.

/// The bare, non-actionable EOF message from issue #81. No surfaced error may
/// consist of only this text.
const BARE_EOF_MSG: &str = "helper closed the connection before confirming attach";

/// Issue #81 (cause surfacing): attaching the REAL helper to a live non-Python
/// process must surface the underlying py-spy failure to the caller — not a
/// bare "helper closed the connection" EOF.
#[tokio::test]
async fn attach_failure_to_non_python_target_surfaces_pyspy_cause() {
    use basilisk_lsp::profiler::helper_client::{start_helper_sampler, HelperSpawn};
    use basilisk_lsp::profiler::sampler::SamplerConfig;

    let helper = helper_binary_path();
    let mut sleeper = ProcessGuard::new(
        Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep"),
    );
    let config = SamplerConfig {
        pid: sleeper.id(),
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };

    let result = timeout(
        Duration::from_secs(30),
        start_helper_sampler(&config, HelperSpawn::Direct(helper)),
    )
    .await
    .expect("start_helper_sampler must not hang");
    sleeper.kill();

    let err = result.expect_err("attaching to a non-Python process must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("py-spy attach failed"),
        "the helper's real attach error must reach the caller (#81), got: {msg}"
    );
}

/// Issue #81 (distinct failure modes): a target PID that no longer exists must
/// be reported as a distinct, actionable cause — not a bare EOF.
#[tokio::test]
async fn attach_to_exited_target_reports_distinct_cause() {
    use basilisk_lsp::profiler::helper_client::{start_helper_sampler, HelperSpawn};
    use basilisk_lsp::profiler::sampler::{SamplerConfig, SamplerError};

    let helper = helper_binary_path();

    // A PID that is guaranteed dead: spawn a no-op and reap it.
    let mut done = Command::new("true").spawn().expect("spawn true");
    let dead_pid = done.id();
    let _ = done.wait();

    let config = SamplerConfig {
        pid: dead_pid,
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };
    let err = timeout(
        Duration::from_secs(30),
        start_helper_sampler(&config, HelperSpawn::Direct(helper)),
    )
    .await
    .expect("must not hang")
    .expect_err("attaching to an exited process must fail");

    let msg = err.to_string();
    let is_distinct = matches!(err, SamplerError::ProcessNotFound(_))
        || msg.contains("No such process")
        || msg.to_lowercase().contains("not found");
    assert!(
        is_distinct,
        "an exited target must be reported as process-not-found, not a bare EOF (#81): {msg}"
    );
    assert!(
        !msg.trim_end().ends_with(BARE_EOF_MSG),
        "error must not be the bare EOF message (#81): {msg}"
    );
}

/// Write an executable fake helper that connects, reads the attach command,
/// writes a recognizable failure to stderr, and exits with a nonzero status —
/// the silent-death shape that issue #81 says must be diagnosable.
fn write_silent_death_fake_helper() -> PathBuf {
    let dir = std::env::temp_dir().join("basilisk_helper_socket_e2e");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(format!("fake-helper-{}.py", unique_suffix()));
    let script = format!(
        "#!/usr/bin/env {python}\nimport socket, sys\ns = socket.socket(socket.AF_UNIX)\ns.connect(sys.argv[1])\ns.recv(4096)\nsys.stderr.write(\"task_for_pid mismatch: helper lacks entitlement\\n\")\nsys.stderr.flush()\nsys.exit(7)\n",
        python = python_path()
    );
    std::fs::write(&path, script).expect("write fake helper");
    let mut perms = std::fs::metadata(&path)
        .expect("stat fake helper")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(&path, perms).expect("chmod fake helper");
    path
}

/// Issue #81 (exit code + stderr surfacing): when the helper dies before
/// confirming attach WITHOUT reporting a structured error, the LSP must capture
/// and surface the helper's exit status and stderr so the failure is
/// diagnosable.
#[tokio::test]
async fn helper_dying_before_attach_surfaces_exit_status_and_stderr() {
    use basilisk_lsp::profiler::helper_client::{start_helper_sampler, HelperSpawn};
    use basilisk_lsp::profiler::sampler::SamplerConfig;

    let fake_helper = write_silent_death_fake_helper();
    let config = SamplerConfig {
        pid: std::process::id(),
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };

    let err = timeout(
        Duration::from_secs(30),
        start_helper_sampler(&config, HelperSpawn::Direct(fake_helper.clone())),
    )
    .await
    .expect("must not hang")
    .expect_err("a helper that dies before confirming attach must fail");
    let _ = std::fs::remove_file(&fake_helper);

    let msg = err.to_string();
    assert!(
        msg.contains("task_for_pid mismatch"),
        "the helper's stderr must be surfaced so the user sees WHY (#81): {msg}"
    );
    assert!(
        msg.contains('7'),
        "the helper's exit status (7) must be surfaced (#81): {msg}"
    );
}

/// Defect 2: the elevated command must not be able to fail on an inaccessible
/// working directory — it `cd /`'s before invoking the helper.
#[test]
fn elevation_command_guards_against_inaccessible_cwd() {
    let script = basilisk_lsp::profiler::helper_client::build_elevation_script(
        "/opt/basilisk/basilisk-profiler-helper",
        "/tmp/sock.sock",
    );
    assert!(
        script.contains("cd / &&"),
        "the elevated command must cd to / first so getcwd cannot fail: {script}"
    );
    let cd = script.find("cd /").expect("cd present");
    let helper = script
        .find("basilisk-profiler-helper")
        .expect("helper present");
    assert!(
        cd < helper,
        "cd / must precede the helper invocation: {script}"
    );
}

/// Issue #81: when the helper cannot attach (here: the target PID is already
/// dead), the LSP must surface the helper's REAL cause — not the undiagnosable
/// "helper closed the connection before confirming attach" bare-EOF message.
#[tokio::test]
async fn attach_to_exited_pid_reports_real_cause_not_bare_eof() {
    use basilisk_lsp::profiler::helper_client::{start_helper_sampler, HelperSpawn};
    use basilisk_lsp::profiler::sampler::SamplerConfig;

    let helper = helper_binary_path();

    // Spawn a process that exits immediately, and reap it so the PID is dead
    // by the time the helper tries to attach.
    let mut child = Command::new("true").spawn().expect("spawn `true`");
    let pid = child.id();
    let _ = child.wait();

    let config = SamplerConfig {
        pid,
        sample_rate: 100,
        include_native: false,
        include_idle: false,
        duration: None,
    };
    let err = timeout(
        Duration::from_secs(30),
        start_helper_sampler(&config, HelperSpawn::Direct(helper)),
    )
    .await
    .expect("must not hang")
    .expect_err("attaching to a dead PID must fail");

    let msg = err.to_string();
    assert!(
        !msg.contains("closed the connection before confirming attach"),
        "a bare EOF is not a diagnosis — the helper's failure cause must be surfaced: {msg}"
    );
    // The py-spy attach error (process gone / not a Python process) must reach
    // the caller so the failure is actionable.
    let lowered = msg.to_lowercase();
    assert!(
        lowered.contains("py-spy") || lowered.contains("process"),
        "expected the helper's real attach-failure cause in the error, got: {msg}"
    );
}
