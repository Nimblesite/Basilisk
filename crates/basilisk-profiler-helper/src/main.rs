//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//! Basilisk profiler helper -- elevated binary for py-spy `vm_read` on macOS.
//!
//! On macOS, reading another process's memory requires elevated privileges
//! (the `task_for_pid` mach call needs root or `SecTaskAccess`). This small
//! binary is spawned by the LSP via `osascript` to get a one-time privilege
//! elevation. It connects back to the LSP over a Unix domain socket and
//! streams stack-trace samples.
//!
//! On Linux, this binary is not needed (`ptrace_scope=0` or same-user tracing).
//! On Windows, `ReadProcessMemory` works without elevation for owned processes.
//!
//! The wire protocol (message shapes + newline-JSON framing) lives in the
//! shared [`basilisk_profiler_protocol`] crate so the LSP and helper never
//! drift. See [PROFILE-HELPER-PROTOCOL].

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use basilisk_profiler_protocol::{
    classify_attach_error, read_message, write_message, AttachErrorKind, Command, FrameData,
    Message, TraceData,
};
use shipwright::{dispatch, BuildInfo, VersionSpec};
use shipwright_manifest::{ExecutableKind, Language};
use tokio::io::BufReader;
use tokio::net::UnixStream;
use tracing::{error, info};

/// Default profiler sampling frequency in hertz.
const DEFAULT_SAMPLE_RATE: u64 = 100;
/// Highest accepted frequency; keeps the timer interval nonzero and CPU bounded.
const MAX_SAMPLE_RATE: u64 = 10_000;

/// Handle `--version` / `--version --json` via the Shipwright contract emitter.
///
/// Returns `true` when a version flag was handled and `main` should exit 0.
/// Build-time metadata is supplied by `build.rs`.
fn handle_version(args: &[String]) -> bool {
    let spec = VersionSpec {
        name: "basilisk-profiler-helper",
        version: env!("CARGO_PKG_VERSION"),
        kind: ExecutableKind::Tool,
        language: Language::Rust,
        product: Some("basilisk"),
        capabilities: &["profiler-helper"],
        build: BuildInfo {
            git_sha: option_env!("SHIPWRIGHT_GIT_SHA"),
            git_dirty: option_env!("SHIPWRIGHT_GIT_DIRTY").map(|s| s == "true"),
            build_time: option_env!("SHIPWRIGHT_BUILD_TIME"),
            target: option_env!("SHIPWRIGHT_TARGET"),
            toolchain: option_env!("SHIPWRIGHT_TOOLCHAIN"),
        },
    };
    match dispatch(args, &mut std::io::stdout(), &spec) {
        Ok(handled) => handled,
        Err(err) => {
            let _ = std::io::Write::write_all(
                &mut std::io::stderr(),
                format!("basilisk-profiler-helper: --version emission failed: {err}\n").as_bytes(),
            );
            true
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if handle_version(&args) {
        return ExitCode::SUCCESS;
    }

    init_tracing();

    let Some(path) = args.first() else {
        error!("usage: basilisk-profiler-helper <socket-path>");
        return ExitCode::FAILURE;
    };

    match run_socket(path).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!(%err, "helper failed");
            ExitCode::FAILURE
        }
    }
}

/// Initialize tracing with stderr output and env-filter support.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
}

/// Connect to the LSP over a Unix domain socket and run the protocol.
async fn run_socket(socket_path: &str) -> Result<(), String> {
    let stream = UnixStream::connect(socket_path)
        .await
        .map_err(|err| format!("connect to {socket_path} failed: {err}"))?;

    let (reader, writer) = stream.into_split();
    run_protocol(BufReader::new(reader), writer).await
}

/// Read the initial attach command from the socket.
async fn read_attach_command(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
) -> Result<(u32, u64, bool), String> {
    let cmd: Option<Command> = read_message(reader)
        .await
        .map_err(|err| format!("read failed: {err}"))?;

    match cmd {
        Some(Command::Attach { pid, rate, native }) => {
            let sample_rate = rate.unwrap_or(DEFAULT_SAMPLE_RATE);
            if !(1..=MAX_SAMPLE_RATE).contains(&sample_rate) {
                return Err(format!(
                    "sample rate must be between 1 and {MAX_SAMPLE_RATE} Hz"
                ));
            }
            Ok((pid, sample_rate, native.unwrap_or(false)))
        }
        Some(Command::Stop) => Err("expected 'attach' command first".to_owned()),
        None => Err("EOF before attach command".to_owned()),
    }
}

/// Attach py-spy to a Python process, returning the spy instance and version.
fn attach_pyspy(
    pid: u32,
    sample_rate: u64,
    include_native: bool,
) -> Result<(py_spy::PythonSpy, String), String> {
    let config = py_spy::Config {
        sampling_rate: sample_rate,
        native: include_native,
        ..Default::default()
    };

    let target_pid = i32::try_from(pid).map_err(|err| format!("invalid PID: {err}"))?;

    let spy = py_spy::PythonSpy::new(target_pid, &config)
        .map_err(|err| format!("py-spy attach failed: {err}"))?;

    let version = format!(
        "{}.{}.{}",
        spy.version.major, spy.version.minor, spy.version.patch
    );

    Ok((spy, version))
}

/// Whether the target PID is alive — running, not just existing.
///
/// Implements [PROFILE-HELPER-PROTOCOL-ERRORS].
/// Refines attach-failure classification (issue #81): py-spy reports the same
/// "Failed to open process" for a dead target and for a live one the helper
/// lacks privileges to read — the user needs to know which. A `kill -0` probe
/// is not enough: it succeeds for a **zombie** (exited, unreaped) process,
/// which dressed a stale-panel-row attach up as a permissions failure (#267).
/// `ps -o stat=` distinguishes the two — no output means gone, a `Z…` state
/// means exited.
fn target_alive(pid: u32) -> bool {
    std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .is_some_and(|stat| !stat.is_empty() && !stat.starts_with('Z'))
}

/// Classify an attach failure, refining ambiguous "cannot open" errors with a
/// liveness probe so target-gone and permission-denied are reported distinctly.
fn classify_helper_attach_error(pid: u32, message: &str) -> AttachErrorKind {
    match classify_attach_error(message) {
        AttachErrorKind::ProcessNotFound | AttachErrorKind::AttachFailed if target_alive(pid) => {
            AttachErrorKind::PermissionDenied
        }
        kind => kind,
    }
}

/// Main protocol loop: read attach, sample, respond.
///
/// Attach failures are reported back over the socket as a structured
/// [`Message::Error`] before the helper exits, so the LSP never sees an
/// undiagnosable bare EOF (issue #81).
async fn run_protocol(
    mut reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    mut writer: tokio::net::unix::OwnedWriteHalf,
) -> Result<(), String> {
    let (pid, sample_rate, include_native) = read_attach_command(&mut reader).await?;
    info!(pid, sample_rate, include_native, "attaching to process");

    let (spy, python_version) = match attach_pyspy(pid, sample_rate, include_native) {
        Ok(attached) => attached,
        Err(message) => {
            error!(pid, %message, "attach failed");
            let kind = classify_helper_attach_error(pid, &message);
            let _ = send_message(
                &mut writer,
                &Message::Error {
                    kind,
                    message: message.clone(),
                },
            )
            .await;
            return Err(message);
        }
    };

    send_message(
        &mut writer,
        &Message::Attached {
            pid,
            python: python_version,
        },
    )
    .await?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    spawn_stop_listener(reader, Arc::clone(&stop_flag));
    run_sampling_loop(spy, &mut writer, &stop_flag, sample_rate).await?;

    let _ = send_message(&mut writer, &Message::Stopped).await;
    info!("helper stopped");
    Ok(())
}

/// Spawn a tokio task that watches for the stop command or EOF.
fn spawn_stop_listener(
    mut reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    stop_flag: Arc<AtomicBool>,
) {
    let _listener = tokio::spawn(async move {
        loop {
            match read_message::<_, Command>(&mut reader).await {
                Ok(Some(Command::Stop) | None) | Err(_) => {
                    stop_flag.store(true, Ordering::SeqCst);
                    break;
                }
                Ok(Some(Command::Attach { .. })) => {
                    // Ignore a duplicate attach; keep waiting for stop/EOF.
                }
            }
        }
    });
}

/// Convert py-spy stack traces to the wire-format structs.
fn convert_traces(traces: &[py_spy::StackTrace]) -> Vec<TraceData> {
    traces
        .iter()
        .map(|trace| TraceData {
            thread_id: trace.thread_id,
            thread_name: trace.thread_name.clone(),
            active: trace.active,
            owns_gil: trace.owns_gil,
            frames: trace
                .frames
                .iter()
                .map(|frame| FrameData {
                    name: frame.name.clone(),
                    filename: frame.filename.clone(),
                    line: frame.line,
                })
                .collect(),
        })
        .collect()
}

/// Sample the target process in a loop until the stop flag is set.
async fn run_sampling_loop(
    mut spy: py_spy::PythonSpy,
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    stop_flag: &Arc<AtomicBool>,
    sample_rate: u64,
) -> Result<(), String> {
    let interval = Duration::from_micros(1_000_000 / sample_rate.max(1));

    while !stop_flag.load(Ordering::SeqCst) {
        match spy.get_stack_traces() {
            Ok(traces) => {
                let data = convert_traces(&traces);
                if let Err(err) = send_message(writer, &Message::Samples { traces: data }).await {
                    info!("parent disconnected: {err}");
                    break;
                }
            }
            Err(err) => {
                if err.to_string().contains("No such process") {
                    info!("target process exited");
                    break;
                }
            }
        }

        tokio::time::sleep(interval).await;
    }

    Ok(())
}

/// Send a framed JSON message over the async writer.
async fn send_message(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    msg: &Message,
) -> Result<(), String> {
    write_message(writer, msg)
        .await
        .map_err(|err| format!("write failed: {err}"))
}

// Tests for [PROFILE-HELPER-PROTOCOL-ERRORS] — attach-failure classification.
#[cfg(test)]
mod tests {
    use super::*;

    /// py-spy's ambiguous "cannot open" attach failure.
    const CANNOT_OPEN: &str =
        "py-spy attach failed: Failed to open process - check if it is running.";

    #[tokio::test]
    async fn attach_rejects_sample_rates_that_can_spin_or_divide_by_zero() -> Result<(), String> {
        for rate in [0, u64::MAX] {
            let (client, server) = UnixStream::pair().map_err(|err| err.to_string())?;
            let (_client_reader, mut client_writer) = client.into_split();
            let (server_reader, _server_writer) = server.into_split();
            write_message(
                &mut client_writer,
                &Command::Attach {
                    pid: 1,
                    rate: Some(rate),
                    native: Some(false),
                },
            )
            .await
            .map_err(|err| err.to_string())?;
            let result = read_attach_command(&mut BufReader::new(server_reader)).await;
            assert!(
                result.is_err(),
                "unsafe sample rate {rate} must be rejected before attach"
            );
        }
        Ok(())
    }

    /// Poll `ps` until `pid` reports the zombie state (`Z…`), or time out.
    fn wait_until_zombie(pid: u32) -> Result<(), String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let output = std::process::Command::new("ps")
                .args(["-o", "stat=", "-p", &pid.to_string()])
                .output()
                .map_err(|err| format!("ps failed: {err}"))?;
            let stat = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if stat.starts_with('Z') {
                return Ok(());
            }
            if std::time::Instant::now() > deadline {
                return Err(format!("PID {pid} never became a zombie (stat: {stat:?})"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// A dead-but-unreaped (zombie) target must classify as `ProcessNotFound`,
    /// not `PermissionDenied` (#267): `kill -0` succeeds for a zombie, so the
    /// liveness refinement of issue #81 dressed a stale-panel-row attach
    /// failure up as a permissions problem the user cannot act on.
    /// [PROFILE-HELPER-PROTOCOL-ERRORS]
    #[test]
    fn zombie_target_classifies_as_process_not_found() -> Result<(), String> {
        // Spawn a process that exits immediately and deliberately do NOT reap
        // it yet — as its parent, we keep it a zombie until `wait` below.
        let mut child = std::process::Command::new("true")
            .spawn()
            .map_err(|err| format!("spawn: {err}"))?;
        let pid = child.id();
        let became_zombie = wait_until_zombie(pid);
        let kind = classify_helper_attach_error(pid, CANNOT_OPEN);
        // Reap before asserting so a failure never leaks the zombie.
        let _ = child.wait();
        became_zombie?;
        assert_eq!(
            kind,
            AttachErrorKind::ProcessNotFound,
            "a zombie target is gone, not a permissions failure (#267)"
        );
        Ok(())
    }

    /// The refinement itself must survive the zombie fix: a genuinely LIVE
    /// target that py-spy cannot open is still a permissions problem (#81).
    #[test]
    fn live_target_still_refines_to_permission_denied() -> Result<(), String> {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .map_err(|err| format!("spawn: {err}"))?;
        let kind = classify_helper_attach_error(child.id(), CANNOT_OPEN);
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(
            kind,
            AttachErrorKind::PermissionDenied,
            "a live-but-unopenable target stays a permissions failure (#81)"
        );
        Ok(())
    }
}
