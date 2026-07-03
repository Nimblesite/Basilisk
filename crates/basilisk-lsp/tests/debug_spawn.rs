//! Tests for [LSPDEBUG-START]. See docs/specs/LSP-DEBUG-INTEGRATION-SPEC.md#LSPDEBUG-START
//! Code under test: crates/basilisk-lsp/src/debug.rs (`DebugSessionManager`).
//!
//! The free-port allocation is a classic TOCTOU: `find_free_port` binds port 0,
//! drops the listener, and debugpy races everything else on the machine to
//! rebind it. A collision makes debugpy exit 1 before accepting connections and
//! the whole launch fails — a flaky first-run experience. These e2es pin the
//! retry contract: a candidate-port collision is retried on the next candidate,
//! and a genuinely broken interpreter still fails with a diagnosable cause.

#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    missing_docs,
    dead_code,
    unused_imports
)]

use std::net::TcpListener;
use std::process::Command;

use basilisk_lsp::debug::{DebugError, DebugSessionManager};

/// The python interpreter used to spawn debugpy, or None to skip.
fn find_python_with_debugpy() -> Option<String> {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let importable = Command::new(python)
        .args(["-c", "import debugpy"])
        .output()
        .ok()?
        .status
        .success();
    importable.then(|| python.to_owned())
}

/// A port something is listening on for the whole test (deterministic collision).
fn occupied_port(listener: &TcpListener) -> u16 {
    listener
        .local_addr()
        .expect("listener must have a local addr")
        .port()
}

/// A port that was free at allocation time (the retry's fresh candidate).
fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind port 0");
    listener
        .local_addr()
        .expect("listener must have a local addr")
        .port()
}

/// The port-collision TOCTOU: the first candidate port is already taken, so
/// debugpy exits before binding. The manager must retry on the next candidate
/// and still mint a working session — never surface the collision to the user.
#[tokio::test]
async fn a_port_collision_is_retried_on_the_next_candidate_port() {
    let Some(python) = find_python_with_debugpy() else {
        eprintln!("skipping: python3 with debugpy not available");
        return;
    };
    let blocker = TcpListener::bind("127.0.0.1:0").expect("bind blocker port");
    let taken = occupied_port(&blocker);
    // Several fresh candidates: the test's own allocation has the same TOCTOU,
    // so any of them may be stolen too — landing on any non-taken one is the
    // contract under test.
    let fresh = [free_port(), free_port(), free_port()];

    let manager = DebugSessionManager::new();
    let (host, port, session_id) = manager
        .start_session_with_ports(&python, std::iter::once(taken).chain(fresh))
        .await
        .expect("a collision on the first candidate port must be retried, not surfaced");

    assert_eq!(host, "localhost", "session must come up on localhost");
    assert_ne!(port, taken, "the session must never land on the taken port");
    assert!(
        fresh.contains(&port),
        "the session must land on a retried fresh candidate, got {port}"
    );
    assert!(
        manager.stop_session(&session_id).await,
        "the retried session must be tracked and stoppable"
    );
}

/// Exhausting every candidate port must fail with the port cause — the
/// pre-flight catches an occupied candidate without spawning a doomed adapter,
/// and crucially never reports a stranger's listener as a ready session.
#[tokio::test]
async fn exhausting_all_candidate_ports_reports_the_port_cause() {
    let Some(python) = find_python_with_debugpy() else {
        eprintln!("skipping: python3 with debugpy not available");
        return;
    };
    let blocker = TcpListener::bind("127.0.0.1:0").expect("bind blocker port");
    let taken = occupied_port(&blocker);

    let manager = DebugSessionManager::new();
    let err = manager
        .start_session_with_ports(&python, vec![taken, taken])
        .await
        .expect_err("every candidate port is taken — the start must fail, never adopt a stranger's listener");

    let DebugError::PortTaken(reported) = err else {
        panic!("expected PortTaken, got: {err:?}");
    };
    assert_eq!(reported, taken, "the failure must name the colliding port");
}

/// An adapter that dies before binding must carry its trailing stderr in the
/// error — a bare exit status is undiagnosable (#81 discipline). A shell posing
/// as the interpreter fails immediately with a stderr line naming the module.
#[cfg(unix)]
#[tokio::test]
async fn an_adapter_exit_carries_its_stderr_in_the_cause() {
    let manager = DebugSessionManager::new();
    let err = manager
        .start_session_with_ports("/bin/sh", vec![free_port()])
        .await
        .expect_err("a shell posing as python must fail to start the adapter");

    let DebugError::AdapterExited(cause) = err else {
        panic!("expected AdapterExited, got: {err:?}");
    };
    assert!(
        cause.contains("exit status") || cause.contains("exit code") || cause.contains("signal"),
        "the cause must carry the exit status, got: {cause}"
    );
    assert!(
        cause.contains("stderr:") && cause.contains("debugpy.adapter"),
        "the cause must include the adapter's trailing stderr, got: {cause}"
    );
}

/// A missing interpreter is NOT a port problem: it must fail immediately as a
/// spawn failure — the port retry must never mask or repeat it.
#[tokio::test]
async fn a_missing_interpreter_fails_fast_without_port_retries() {
    let Some(python) = find_python_with_debugpy() else {
        eprintln!("skipping: python3 with debugpy not available");
        return;
    };
    let manager = DebugSessionManager::new();
    let err = manager
        .start_session_with_ports(&format!("{python}-definitely-missing"), vec![free_port()])
        .await
        .expect_err("a missing interpreter must fail");
    assert!(
        matches!(err, DebugError::SpawnFailed(_)),
        "a missing interpreter is a spawn failure, not a retryable adapter exit: {err:?}"
    );
}
