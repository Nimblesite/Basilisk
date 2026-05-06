//! Basilisk profiler helper -- elevated binary for py-spy `vm_read` on macOS.
//!
//! On macOS, reading another process's memory requires elevated privileges
//! (the `task_for_pid` mach call needs root or `SecTaskAccess`). This small
//! binary is spawned by the LSP via `osascript` to get a one-time privilege
//! elevation. It communicates with the LSP over a Unix domain socket,
//! streaming stack trace samples back.
//!
//! On Linux, this binary is not needed (`ptrace_scope=0` or same-user tracing).
//! On Windows, `ReadProcessMemory` works without elevation for owned processes.
//!
//! # Protocol (over Unix socket, newline-delimited JSON)
//!
//! LSP sends: `{"cmd":"attach","pid":12345,"rate":100,"native":false}`
//! Helper sends: `{"type":"attached","pid":12345,"python":"3.12.0"}`
//! Helper sends: `{"type":"samples","traces":[...]}`  (repeating)
//! LSP sends: `{"cmd":"stop"}`
//! Helper sends: `{"type":"stopped"}`
//! Helper exits.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use basilisk_common::shipwright_version::{self, VersionOutput};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{error, info};

/// Command from LSP to helper.
#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
enum Command {
    /// Attach to a Python process and begin sampling.
    Attach {
        /// Target process ID.
        pid: u32,
        /// Samples per second.
        rate: Option<u64>,
        /// Include native C frames.
        native: Option<bool>,
    },
    /// Stop sampling and exit.
    Stop,
}

/// Message from helper to LSP.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Message {
    /// Confirms successful attachment.
    Attached {
        /// Target PID.
        pid: u32,
        /// Detected Python version.
        python: String,
    },
    /// A batch of stack trace samples.
    Samples {
        /// The sampled traces.
        traces: Vec<TraceData>,
    },
    /// Sampling has stopped.
    Stopped,
}

/// Simplified stack trace for serialization.
#[derive(Debug, Serialize)]
struct TraceData {
    /// OS thread ID.
    thread_id: u64,
    /// Thread name if available.
    thread_name: Option<String>,
    /// Whether the thread is actively running.
    active: bool,
    /// Whether the thread holds the GIL.
    owns_gil: bool,
    /// Stack frames from innermost to outermost.
    frames: Vec<FrameData>,
}

/// Single frame in a stack trace.
#[derive(Debug, Serialize)]
struct FrameData {
    /// Function or method name.
    name: String,
    /// Source file path.
    filename: String,
    /// Line number in the source file.
    line: i32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if shipwright_version::print_if_requested(
        &args,
        VersionOutput {
            name: "basilisk-profiler-helper",
            kind: "tool",
            product: "basilisk",
            capabilities: &["profiler-helper"],
        },
    ) {
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
    let mut line = String::new();
    let bytes_read = reader
        .read_line(&mut line)
        .await
        .map_err(|err| format!("read failed: {err}"))?;

    if bytes_read == 0 {
        return Err("EOF before attach command".to_owned());
    }

    let cmd: Command =
        serde_json::from_str(line.trim()).map_err(|err| format!("parse failed: {err}"))?;

    let Command::Attach { pid, rate, native } = cmd else {
        return Err("expected 'attach' command first".to_owned());
    };

    Ok((pid, rate.unwrap_or(100), native.unwrap_or(false)))
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

/// Main protocol loop: read attach, sample, respond.
async fn run_protocol(
    mut reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    mut writer: tokio::net::unix::OwnedWriteHalf,
) -> Result<(), String> {
    let (pid, sample_rate, include_native) = read_attach_command(&mut reader).await?;
    info!(pid, sample_rate, include_native, "attaching to process");

    let (spy, python_version) = attach_pyspy(pid, sample_rate, include_native)?;

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
        let mut buf = String::new();
        loop {
            buf.clear();
            match reader.read_line(&mut buf).await {
                Ok(0) | Err(_) => {
                    stop_flag.store(true, Ordering::SeqCst);
                    break;
                }
                Ok(_) => {
                    if let Ok(Command::Stop) = serde_json::from_str(buf.trim()) {
                        stop_flag.store(true, Ordering::SeqCst);
                        break;
                    }
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

/// Send a JSON message followed by a newline over the async writer.
async fn send_message(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    msg: &Message,
) -> Result<(), String> {
    let json = serde_json::to_string(msg).map_err(|err| format!("serialize failed: {err}"))?;
    writer
        .write_all(json.as_bytes())
        .await
        .map_err(|err| format!("write failed: {err}"))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|err| format!("write newline failed: {err}"))?;
    writer
        .flush()
        .await
        .map_err(|err| format!("flush failed: {err}"))?;
    Ok(())
}
