//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Tests for LSP: `ws_test_processes` — the `basilisk.profiler.processes`
// enumeration command. Implements [PROFILE-PROCESSES-LSP].
//
// This is the headline fix for #62: the LSP — not the editor — enumerates
// attachable Python processes, so the panel can offer one-click profiling
// instead of a raw PID text box. Enumeration must NOT require elevation
// (it only reads the process table), so these tests run in normal CI.

use std::process::{Command, Stdio};

use super::common::ProcessGuard;
use super::ws_test_common::*;

/// Path to the Python 3 interpreter, honoring the same env overrides the
/// profiler e2e suite uses.
fn python_path() -> String {
    std::env::var("PYTHON")
        .or_else(|_| std::env::var("BASILISK_PYTHON"))
        .unwrap_or_else(|_| "python3".to_owned())
}

/// Spawn a long-lived Python interpreter that prints `READY` then sleeps, so
/// the process is guaranteed to be in the OS process table when we enumerate.
/// Returns `None` if python is unavailable (the test then skips).
fn spawn_idle_python() -> Option<ProcessGuard> {
    let child = Command::new(python_path())
        .arg("-c")
        .arg("import time, sys; print('READY', flush=True); time.sleep(60)")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let mut guard = ProcessGuard::new(child);
    if wait_for_ready(&mut guard) {
        Some(guard)
    } else {
        None
    }
}

/// Block until the child prints `READY` (or fail fast). Returns false if the
/// process never signalled readiness.
fn wait_for_ready(guard: &mut ProcessGuard) -> bool {
    use std::io::BufRead;
    let Some(stdout) = guard.child_mut().stdout.as_mut() else {
        return false;
    };
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(n) if n > 0 => line.trim() == "READY",
        _ => false,
    }
}

/// Call `basilisk.profiler.processes` and return the parsed `processes` array.
async fn fetch_processes(
    fixture: &mut WsTestFixture,
    id: u64,
) -> TestResult<Vec<serde_json::Value>> {
    let resp = fixture
        .request(
            id,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.profiler.processes",
                "arguments": [{}]
            }),
        )
        .await?
        .ok_or("no response to basilisk.profiler.processes")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("error").is_none(),
        "processes command must not return an error: {resp}"
    );
    let processes = parsed
        .get("result")
        .and_then(|r| r.get("processes"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("response should contain result.processes array: {resp}"))?;
    Ok(processes.clone())
}

#[tokio::test]
async fn test_ws_profiler_processes_lists_running_python() -> TestResult<()> {
    let Some(python) = spawn_idle_python() else {
        eprintln!("SKIP: python3 not available for process-enumeration e2e");
        return Ok(());
    };
    let target_pid = python.id();

    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let processes = fetch_processes(&mut fixture, 700).await?;
    assert!(
        !processes.is_empty(),
        "enumeration should find at least the spawned Python process"
    );

    let entry = processes
        .iter()
        .find(|p| p.get("pid").and_then(serde_json::Value::as_u64) == Some(u64::from(target_pid)))
        .unwrap_or_else(|| {
            panic!("spawned Python PID {target_pid} should appear in processes: {processes:?}")
        });

    // Rich detail must be populated for the panel to render a useful row.
    let name = entry.get("name").and_then(serde_json::Value::as_str);
    assert!(
        name.is_some_and(|n| n.to_lowercase().contains("python")),
        "process name should identify python: {entry:?}"
    );
    assert!(
        entry
            .get("interpreterPath")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|p| !p.is_empty()),
        "interpreterPath should be resolved: {entry:?}"
    );
    // Our own child is owned by the current user — no elevation prompt.
    assert_eq!(
        entry
            .get("requiresElevation")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "a process owned by the current user must not require elevation: {entry:?}"
    );
    // Every documented field must be present (value may be null for lazy ones).
    for field in [
        "pid",
        "ppid",
        "name",
        "pythonVersion",
        "cpuPercent",
        "memoryBytes",
        "runtimeSecs",
        "kind",
    ] {
        assert!(
            entry.get(field).is_some(),
            "ProcessInfo must include `{field}`: {entry:?}"
        );
    }

    drop(python);
    Ok(())
}

#[tokio::test]
async fn test_ws_profiler_processes_excludes_non_python_noise() -> TestResult<()> {
    let Some(python) = spawn_idle_python() else {
        eprintln!("SKIP: python3 not available for process-enumeration e2e");
        return Ok(());
    };

    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let processes = fetch_processes(&mut fixture, 710).await?;

    // The test/LSP process itself is a Rust binary, not Python, so it must be
    // filtered out — proving enumeration discriminates Python from noise.
    let own_pid = u64::from(std::process::id());
    assert!(
        !processes
            .iter()
            .any(|p| p.get("pid").and_then(serde_json::Value::as_u64) == Some(own_pid)),
        "non-Python current process (pid {own_pid}) must be excluded from the Python list"
    );

    drop(python);
    Ok(())
}
