//! Tests for [PROFILE-COOPERATIVE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-COOPERATIVE
//!
//! REAL full-stack CPU profiling through the LSP wire — no mocks.
//!
//! Where `profiler_cooperative.rs` drives the sampler engine directly, this
//! suite goes through the ACTUAL JSON-RPC `workspace/executeCommand` surface of
//! a live server: mint the cooperative sampling script with
//! `basilisk.profiler.cooperativeScript`, inject it into a real CPU-bound Python
//! process, adopt the stream with `basilisk.profiler.cooperativeAttach`, watch
//! live `basilisk/profiler/progress` notifications, then `snapshot`, `list`, and
//! `stop` — asserting the genuine hotspot, on-disk speedscope / flamegraph /
//! `.cpuprofile` artifacts, and diagnostics. This is the out-of-the-box CPU path
//! for debug-launched sessions and runs on every platform (no ptrace / task
//! ports). SKIPs cleanly when `python3` is absent.

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

mod common;
#[path = "lsp/ws_test_common.rs"]
mod ws_test_common;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::ProcessGuard;
use ws_test_common::*;

/// A CPU-bound main program appended after the injected sampler. It self-stops
/// well after a test finishes; `ProcessGuard` kills it on drop regardless.
const HOT_WORKLOAD: &str = "\
import time


def hot_function():
    total = 0
    for value in range(300000):
        total += value * value
    return total


_deadline = time.time() + 15
while time.time() < _deadline:
    hot_function()
";

/// Spawn a long-running Python program from source, returning the guard and the
/// temp script path. `None` means SKIP (no interpreter).
fn spawn_python_program(src: &str) -> Option<(ProcessGuard, PathBuf)> {
    let path = common::unique_temp_file("basilisk_coop_prog", "py");
    std::fs::write(&path, src).ok()?;
    let child = Command::new(common::python_path())
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    Some((ProcessGuard::new(child), path))
}

/// Poll the wire for a live progress notification with a non-zero sample count.
async fn wait_for_progress(
    fixture: &mut WsTestFixture,
    max_messages: usize,
) -> Option<serde_json::Value> {
    for _ in 0..max_messages {
        let msg = fixture.recv().await?;
        if !msg.contains("basilisk/profiler/progress") {
            continue;
        }
        let parsed: serde_json::Value = serde_json::from_str(&msg).ok()?;
        let params = parsed.get("params")?;
        if params
            .get("sampleCount")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|count| count > 0)
        {
            return Some(params.clone());
        }
    }
    None
}

// ── The headline full-pipeline test ──────────────────────────────────────────

/// Script → inject into real Python → attach → snapshot → list → stop, all over
/// the JSON-RPC wire, asserting the genuine hotspot and every on-disk artifact.
#[tokio::test]
async fn cooperative_cpu_full_lsp_roundtrip() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Leg 1: mint the injection script, then inject it into a real process.
    let (script, sample_file) = mint_cooperative_script(&mut fixture, 100).await?;
    let program = format!("{script}\n{HOT_WORKLOAD}");
    let Some((mut guard, prog_path)) = spawn_python_program(&program) else {
        eprintln!("SKIP: python3 not available");
        return Ok(());
    };

    // Leg 2: adopt the cooperative stream and confirm live progress flows.
    let session_id = attach_cooperative(&mut fixture, 101, &sample_file, guard.id()).await?;
    assert!(
        wait_for_progress(&mut fixture, 20).await.is_some(),
        "the server must push live progress with a non-zero sample count"
    );

    // A mid-flight snapshot must already show real samples.
    let snapshot_resp = execute_command(
        &mut fixture,
        102,
        "basilisk.profiler.snapshot",
        serde_json::json!({ "sessionId": session_id }),
    )
    .await?;
    assert!(
        command_result(&snapshot_resp, "snapshot")?["totalSamples"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "a mid-flight snapshot must report real samples: {snapshot_resp}"
    );
    assert_session_listed(&mut fixture, 103, &session_id, true).await?;

    // Stop, then validate the genuine hotspot + every on-disk artifact.
    let stop_resp = execute_command(
        &mut fixture,
        104,
        "basilisk.profiler.stop",
        serde_json::json!({ "sessionId": session_id }),
    )
    .await?;
    let stop_result = command_result(&stop_resp, "stop")?;
    guard.kill();
    let _ = std::fs::remove_file(&prog_path);

    validate_stop_result(stop_result)?;
    assert_session_listed(&mut fixture, 105, &session_id, false).await?;
    Ok(())
}

/// Mint the cooperative injection script and its sample file over the wire,
/// asserting the file is a minted `basilisk-cpu-*` path.
async fn mint_cooperative_script(
    fixture: &mut WsTestFixture,
    id: u64,
) -> TestResult<(String, String)> {
    let resp = execute_command(
        fixture,
        id,
        "basilisk.profiler.cooperativeScript",
        serde_json::json!({ "sampleRate": 100 }),
    )
    .await?;
    let result = command_result(&resp, "cooperativeScript")?;
    let script = result["script"]
        .as_str()
        .ok_or("cooperativeScript must return a script")?
        .to_owned();
    let sample_file = result["sampleFile"]
        .as_str()
        .ok_or("cooperativeScript must return a sampleFile")?
        .to_owned();
    assert!(
        PathBuf::from(&sample_file)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("basilisk-cpu-")),
        "sample file must be a minted basilisk-cpu path: {sample_file}"
    );
    Ok((script, sample_file))
}

/// Adopt the cooperative stream, asserting the debuggee identity, and return the
/// session id.
async fn attach_cooperative(
    fixture: &mut WsTestFixture,
    id: u64,
    sample_file: &str,
    expected_pid: u32,
) -> TestResult<String> {
    let resp = execute_command(
        fixture,
        id,
        "basilisk.profiler.cooperativeAttach",
        serde_json::json!({ "sampleFile": sample_file }),
    )
    .await?;
    let result = command_result(&resp, "cooperativeAttach")?;
    let session_id = result["sessionId"]
        .as_str()
        .ok_or("attach must return a sessionId")?
        .to_owned();
    assert_eq!(
        result["pid"].as_u64(),
        Some(u64::from(expected_pid)),
        "attach must report the debuggee pid"
    );
    assert!(
        result["pythonVersion"]
            .as_str()
            .is_some_and(|version| version.starts_with("3.")),
        "attach must detect Python 3.x: {result}"
    );
    Ok(session_id)
}

/// Assert the active-session list does (`present`) or does not contain the id.
async fn assert_session_listed(
    fixture: &mut WsTestFixture,
    id: u64,
    session_id: &str,
    present: bool,
) -> TestResult<()> {
    let resp = execute_command(fixture, id, "basilisk.profiler.list", serde_json::json!({})).await?;
    let result = command_result(&resp, "list")?;
    let sessions = result["sessions"]
        .as_array()
        .ok_or("list must return a sessions array")?;
    let found = sessions
        .iter()
        .any(|session| session["sessionId"].as_str() == Some(session_id));
    assert_eq!(
        found, present,
        "session {session_id} listed-presence mismatch (want {present}): {result}"
    );
    Ok(())
}

/// Validate the stop result: real samples, the genuine hotspot, and that the
/// speedscope, flamegraph, and `.cpuprofile` artifacts all exist and parse.
fn validate_stop_result(stop_result: &serde_json::Value) -> TestResult<()> {
    assert!(
        stop_result["totalSamples"]
            .as_u64()
            .is_some_and(|count| count > 10),
        "stop must report the accumulated samples: {stop_result}"
    );
    let hot_functions = stop_result["hotFunctions"]
        .as_array()
        .ok_or("stop must return hotFunctions")?;
    assert!(
        hot_functions
            .iter()
            .any(|func| func["name"].as_str() == Some("hot_function")),
        "stop must attribute the genuine hotspot `hot_function`: {hot_functions:?}"
    );

    assert_artifact(stop_result, "outputFile", |path| {
        let json = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            json.contains("hot_function"),
            "speedscope export must contain the hot frame"
        );
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        assert!(
            parsed
                .get("profiles")
                .and_then(serde_json::Value::as_array)
                .is_some(),
            "speedscope must have a profiles array"
        );
    });
    assert_artifact(stop_result, "flamegraphPath", |path| {
        let svg = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            svg.starts_with("<svg") || svg.starts_with("<?xml"),
            "flamegraph must be valid SVG"
        );
    });
    assert_artifact(stop_result, "cpuProfilePath", |path| {
        let json = std::fs::read_to_string(path).unwrap_or_default();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        assert!(
            parsed
                .get("nodes")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|nodes| !nodes.is_empty()),
            ".cpuprofile must carry a non-empty nodes array for VS Code's viewer"
        );
    });
    Ok(())
}

/// Assert a stop artifact path is present, exists on disk, and passes `check`.
fn assert_artifact(stop_result: &serde_json::Value, key: &str, check: impl Fn(&str)) {
    let path = stop_result[key]
        .as_str()
        .unwrap_or_else(|| panic!("stop must return {key} (export must not fail): {stop_result}"));
    assert!(
        std::path::Path::new(path).exists(),
        "{key} must exist on disk: {path}"
    );
    check(path);
}

// ── Wire-level guardrails (no Python needed) ─────────────────────────────────

/// `cooperativeAttach` must reject any sample path it did not mint — a hardening
/// guard so a caller cannot point the tailer at an arbitrary file.
#[tokio::test]
async fn cooperative_attach_rejects_foreign_sample_path() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let resp = execute_command(
        &mut fixture,
        200,
        "basilisk.profiler.cooperativeAttach",
        serde_json::json!({ "sampleFile": "/tmp/not-a-basilisk-file.jsonl" }),
    )
    .await?;
    let error = resp
        .get("error")
        .filter(|err| !err.is_null())
        .ok_or("a foreign sample path must be rejected with an error")?;
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|msg| msg.contains("basilisk-cpu")),
        "the error must explain the path constraint: {error}"
    );
    Ok(())
}

/// `cooperativeAttach` with no `sampleFile` must be a clean parameter error.
#[tokio::test]
async fn cooperative_attach_missing_sample_file_is_error() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let resp = execute_command(
        &mut fixture,
        201,
        "basilisk.profiler.cooperativeAttach",
        serde_json::json!({}),
    )
    .await?;
    let error = resp
        .get("error")
        .filter(|err| !err.is_null())
        .ok_or("missing sampleFile must error")?;
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|msg| msg.contains("sampleFile")),
        "the error must name the missing parameter: {error}"
    );
    Ok(())
}

/// Each `cooperativeScript` call mints a distinct `basilisk-cpu-*` sample file —
/// two concurrent debug sessions must never share a tail file.
#[tokio::test]
async fn cooperative_script_mints_unique_basilisk_cpu_files() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let first = execute_command(
        &mut fixture,
        210,
        "basilisk.profiler.cooperativeScript",
        serde_json::json!({}),
    )
    .await?;
    let second = execute_command(
        &mut fixture,
        211,
        "basilisk.profiler.cooperativeScript",
        serde_json::json!({}),
    )
    .await?;

    let file_a = command_result(&first, "cooperativeScript a")?["sampleFile"]
        .as_str()
        .ok_or("missing sampleFile a")?
        .to_owned();
    let file_b = command_result(&second, "cooperativeScript b")?["sampleFile"]
        .as_str()
        .ok_or("missing sampleFile b")?
        .to_owned();

    assert_ne!(file_a, file_b, "minted sample files must be unique");
    for file in [&file_a, &file_b] {
        assert!(
            PathBuf::from(file)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("basilisk-cpu-")),
            "minted file must be a basilisk-cpu path: {file}"
        );
    }
    Ok(())
}

/// Stopping an unknown session is a clean error, not a panic.
#[tokio::test]
async fn profiler_stop_unknown_session_is_error() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let resp = execute_command(
        &mut fixture,
        220,
        "basilisk.profiler.stop",
        serde_json::json!({ "sessionId": "prof-does-not-exist" }),
    )
    .await?;
    assert!(
        resp.get("error").is_some_and(|err| !err.is_null()),
        "stopping a phantom session must error: {resp}"
    );
    Ok(())
}

/// With no sessions started, `list` returns an empty set over the wire.
#[tokio::test]
async fn profiler_list_is_empty_before_any_session() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let resp = execute_command(
        &mut fixture,
        230,
        "basilisk.profiler.list",
        serde_json::json!({}),
    )
    .await?;
    let result = command_result(&resp, "list")?;
    assert_eq!(
        result["sessions"].as_array().map(Vec::len),
        Some(0),
        "a fresh server lists no sessions: {result}"
    );
    Ok(())
}
