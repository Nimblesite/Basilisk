//! Implements [PROFILE-COOPERATIVE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-COOPERATIVE
//!
//! Cooperative in-process CPU sampler — the out-of-the-box path for
//! debug-launched sessions.
//!
//! Reading another process's memory needs a task port on macOS, which Apple
//! gates behind signed, debugger-entitled callers — even root + py-spy gets
//! `EPERM` on modern macOS. So for sessions the editor launches under
//! debugpy, Basilisk does not read foreign memory at all: the editor injects
//! a small daemon thread (via the same paused-`evaluate` courier the memory
//! profiler uses, [PROFILE-MEMORY-HOWTO]) that samples
//! `sys._current_frames()` at the configured rate and streams one JSONL tick
//! record per sample to a temp file. This module tails that file and exposes
//! it as a [`SamplerHandle`] — the same abstraction the in-process py-spy
//! sampler and the elevated helper produce — so the entire downstream
//! pipeline (aggregation, heat map, speedscope, `.cpuprofile`, diagnostics,
//! live progress) is reused unchanged.
//!
//! Wire format (one JSON value per line):
//!
//! ```text
//! {"header":{"python":"3.13.7","pid":12345}}
//! {"ticks":[[140234,true,[["/app/x.py",12,"hot_function"],...]],...]}   (repeating)
//! ```
//!
//! Frames are leaf-first, matching py-spy. Stopping writes a `<file>.stop`
//! sentinel; the injected thread sees it and exits, and the tailer drains
//! whatever was flushed before removing both files.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use basilisk_profiler_protocol::{FrameData, TraceData};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tracing::{debug, info, warn};

use super::helper_client::to_pyspy_traces;
use super::sampler::{SampleBatch, SamplerError, SamplerHandle};

/// How long to wait for the injected thread to write its header line.
const HEADER_TIMEOUT: Duration = Duration::from_secs(15);
/// Poll interval while waiting for new sample lines.
const TAIL_POLL_INTERVAL: Duration = Duration::from_millis(100);
/// How long to keep draining after a stop is requested.
const STOP_DRAIN_GRACE: Duration = Duration::from_secs(2);
/// Sample-batch channel depth (mirrors the other samplers).
const SAMPLE_CHANNEL_DEPTH: usize = 256;

/// The Python injected into the debuggee. `@SAMPLE_FILE@` / `@RATE@` are
/// replaced before injection; the printed marker is the courier's ack.
const SCRIPT_TEMPLATE: &str = r#"def __basilisk_cpu_start():
    import json, os, sys, threading, time
    path = @SAMPLE_FILE@
    rate = @RATE@
    interval = 1.0 / max(rate, 1)
    stop_path = path + ".stop"
    wait_files = ("threading.py", "selectors.py", "queue.py", "socket.py", "ssl.py", "subprocess.py")

    def snap(me):
        ticks = []
        for tid, top in sys._current_frames().items():
            if tid == me:
                continue
            frames = []
            frame = top
            while frame is not None and len(frames) < 128:
                code = frame.f_code
                frames.append([code.co_filename, frame.f_lineno, code.co_name])
                frame = frame.f_back
            # Attribute tracer overhead to the user code being traced: the
            # sys.settrace callbacks (pydevd/debugpy) sit on TOP of the user
            # frame, so drop leading debugger frames instead of discarding
            # the sample — otherwise traced hot code is starved.
            while frames and (
                "pydevd" in frames[0][0]
                or "debugpy" in frames[0][0]
                or "_pydev" in frames[0][0]
            ):
                frames.pop(0)
            if not frames:
                continue
            active = not frames[0][0].endswith(wait_files)
            ticks.append([tid, active, frames])
        return ticks

    def run():
        me = threading.get_ident()
        with open(path, "a") as out:
            header = {"python": "%d.%d.%d" % sys.version_info[:3], "pid": os.getpid()}
            out.write(json.dumps({"header": header}) + "\n")
            out.flush()
            last_flush = time.monotonic()
            while not os.path.exists(stop_path):
                out.write(json.dumps({"ticks": snap(me)}) + "\n")
                now = time.monotonic()
                if now - last_flush >= 0.5:
                    out.flush()
                    last_flush = now
                time.sleep(interval)
            out.flush()

    threading.Thread(target=run, name="basilisk-cpu-sampler", daemon=True).start()
    print("__BASILISK_CPU_ACK__")


__basilisk_cpu_start()
"#;

/// Mint a unique sample-file path for one cooperative session (under `/tmp`
/// style short paths, like the helper socket). A process-wide counter breaks
/// ties — wall-clock nanos alone collide for concurrent sessions.
#[must_use]
pub fn mint_sample_file() -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "basilisk-cpu-{}-{nanos}-{sequence}.jsonl",
        std::process::id()
    ))
}

/// Render the injection script for one session.
///
/// # Errors
///
/// Returns an error if the sample-file path cannot be JSON-encoded.
pub fn sampling_script(sample_file: &Path, sample_rate: u64) -> Result<String, SamplerError> {
    let path_json = serde_json::to_string(&sample_file.display().to_string())
        .map_err(|err| SamplerError::AttachFailed(format!("encode sample path: {err}")))?;
    Ok(SCRIPT_TEMPLATE
        .replace("@SAMPLE_FILE@", &path_json)
        .replace("@RATE@", &sample_rate.to_string()))
}

/// One thread's tick entry on the wire: `[thread_id, active, frames]`.
type TickThread = (u64, bool, Vec<(String, i32, String)>);

#[derive(serde::Deserialize)]
struct HeaderLine {
    header: HeaderInfo,
}

#[derive(serde::Deserialize)]
struct HeaderInfo {
    python: String,
    pid: u32,
}

#[derive(serde::Deserialize)]
struct TickLine {
    ticks: Vec<TickThread>,
}

/// Start tailing a cooperative sample file as a [`SamplerHandle`].
///
/// Waits for the injected thread's header (Python version + PID), then
/// streams tick batches into the standard sampler channel.
///
/// # Errors
///
/// Returns [`SamplerError::AttachFailed`] if the header never arrives —
/// e.g. the editor failed to inject the script.
pub async fn start_cooperative_sampler(
    sample_file: PathBuf,
) -> Result<SamplerHandle, SamplerError> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = oneshot::channel::<(String, u32)>();
    let (sample_tx, sample_rx) = mpsc::channel::<SampleBatch>(SAMPLE_CHANNEL_DEPTH);

    let tail_flag = Arc::clone(&stop_flag);
    let tail_file = sample_file.clone();
    let join_handle = std::thread::Builder::new()
        .name("profiler-cooperative-tail".to_owned())
        .spawn(move || tail_loop(&tail_file, ready_tx, &sample_tx, &tail_flag))
        .map_err(|err| {
            SamplerError::AttachFailed(format!("failed to spawn cooperative tail thread: {err}"))
        })?;

    if let Ok(Ok((python_version, pid))) = timeout(HEADER_TIMEOUT, ready_rx).await {
        info!(pid, %python_version, file = %sample_file.display(), "cooperative sampler attached");
        Ok(SamplerHandle::from_parts(
            stop_flag,
            join_handle,
            sample_rx,
            python_version,
            pid,
        ))
    } else {
        stop_flag.store(true, Ordering::SeqCst);
        let _ = join_handle.join();
        Err(SamplerError::AttachFailed(format!(
            "the in-process sampler never reported its header in {}; \
             was the injection script evaluated in the paused debuggee?",
            sample_file.display()
        )))
    }
}

/// Tail the sample file: handshake on the header, then stream tick batches
/// until a stop is requested or the channel closes.
fn tail_loop(
    sample_file: &Path,
    ready_tx: oneshot::Sender<(String, u32)>,
    sample_tx: &mpsc::Sender<SampleBatch>,
    stop_flag: &Arc<AtomicBool>,
) {
    let Some((mut reader, header)) = await_header(sample_file, stop_flag) else {
        drop(ready_tx);
        return;
    };
    let pid = header.pid;
    if ready_tx.send((header.python, pid)).is_err() {
        cleanup(sample_file);
        return;
    }

    let mut line = String::new();
    let mut drain_deadline: Option<Instant> = None;
    loop {
        if stop_flag.load(Ordering::SeqCst) && drain_deadline.is_none() {
            // Tell the injected thread to exit, then drain what it flushed.
            let _ = std::fs::write(stop_sentinel(sample_file), b"stop");
            drain_deadline = Some(Instant::now() + STOP_DRAIN_GRACE);
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                if drain_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    break;
                }
                std::thread::sleep(TAIL_POLL_INTERVAL);
            }
            Ok(_) => {
                if let Ok(tick) = serde_json::from_str::<TickLine>(&line) {
                    let batch = SampleBatch {
                        traces: to_pyspy_traces(pid, tick_to_wire(tick.ticks)),
                    };
                    if sample_tx.blocking_send(batch).is_err() {
                        break;
                    }
                }
            }
            Err(err) => {
                warn!(%err, "cooperative sample file read failed");
                break;
            }
        }
    }
    cleanup(sample_file);
    debug!(file = %sample_file.display(), "cooperative tail finished");
}

/// Wait for the sample file to exist and its header line to arrive.
fn await_header(
    sample_file: &Path,
    stop_flag: &Arc<AtomicBool>,
) -> Option<(std::io::BufReader<std::fs::File>, HeaderInfo)> {
    loop {
        if stop_flag.load(Ordering::SeqCst) {
            return None;
        }
        if let Ok(file) = std::fs::File::open(sample_file) {
            let mut reader = std::io::BufReader::new(file);
            let mut line = String::new();
            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    return None;
                }
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => std::thread::sleep(TAIL_POLL_INTERVAL),
                    Ok(_) => {
                        return serde_json::from_str::<HeaderLine>(&line)
                            .ok()
                            .map(|parsed| (reader, parsed.header));
                    }
                    Err(_) => return None,
                }
            }
        }
        std::thread::sleep(TAIL_POLL_INTERVAL);
    }
}

/// Convert wire ticks into the shared protocol trace shape.
fn tick_to_wire(ticks: Vec<TickThread>) -> Vec<TraceData> {
    ticks
        .into_iter()
        .map(|(thread_id, active, frames)| TraceData {
            thread_id,
            thread_name: None,
            active,
            owns_gil: false,
            frames: frames
                .into_iter()
                .map(|(filename, line, name)| FrameData {
                    name,
                    filename,
                    line,
                })
                .collect(),
        })
        .collect()
}

/// The stop-sentinel path for a sample file.
fn stop_sentinel(sample_file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.stop", sample_file.display()))
}

/// Remove the sample file and its sentinel.
fn cleanup(sample_file: &Path) {
    let _ = std::fs::remove_file(sample_file);
    let _ = std::fs::remove_file(stop_sentinel(sample_file));
}
