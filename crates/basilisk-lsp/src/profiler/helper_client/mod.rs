//! Implements [PROFILE-HELPER-SOCKET]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-HELPER-SOCKET
//!
//! LSP side of the elevated-helper profiling path.
//!
//! On macOS, py-spy cannot `vm_read` a process Basilisk did not launch as a
//! child unless it runs as root. The LSP therefore:
//!
//! 1. binds a [`UnixListener`] **before** spawning the helper (issue #61,
//!    Defect 1 — previously nothing ever listened, so the helper's
//!    `connect()` always failed with `No such file or directory`);
//! 2. spawns `basilisk-profiler-helper` — elevated via `osascript` in
//!    production, or directly for tests — without blocking on its exit
//!    (Defect 3 — the helper is a long-lived streamer, not a one-shot);
//! 3. accepts the connection, drives the `attach`/`samples`/`stop` protocol
//!    from [`basilisk_profiler_protocol`], and forwards samples into the same
//!    [`SamplerHandle`] channel the in-process sampler uses, so the rest of the
//!    pipeline is identical for both sources.
//!
//! The elevated `osascript` command is cwd-guarded (Defect 2) so it never
//! inherits an inaccessible working directory.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use basilisk_profiler_protocol::{read_message, write_message, Command, Message};
use tokio::io::BufReader;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixListener;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tracing::{info, warn};

use super::sampler::{SampleBatch, SamplerConfig, SamplerError, SamplerHandle};

mod wire;
pub use wire::build_elevation_script;
use wire::{create_socket_path, to_pyspy_traces};

/// How long to wait for the helper to connect back to the bound socket.
const ACCEPT_TIMEOUT: Duration = Duration::from_secs(20);
/// How long to wait for the helper's `attached` confirmation after `attach`.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);
/// How often the driver checks whether a stop was requested.
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// How long to wait for the helper's final `stopped` after we send `stop`.
const STOP_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
/// Sample-batch channel depth (mirrors the in-process sampler).
const SAMPLE_CHANNEL_DEPTH: usize = 256;

/// How the helper binary should be launched.
#[derive(Debug)]
pub enum HelperSpawn {
    /// Launch with administrator privileges via `osascript` (macOS only).
    Elevated,
    /// Launch the given binary directly, without elevation (used by tests and
    /// any environment where the caller already has sufficient privileges).
    Direct(PathBuf),
}

/// Start a profiling sampler backed by the elevated helper over a Unix socket.
///
/// Binds the listener, spawns the helper, performs the attach handshake, and
/// returns a [`SamplerHandle`] whose channel streams samples for as long as the
/// handle lives. Dropping or [`SamplerHandle::stop`]-ing the handle tells the
/// helper to stop and tears down the socket.
///
/// # Errors
///
/// Returns a [`SamplerError`] if the socket cannot be bound, the helper cannot
/// be spawned or elevated, or the attach handshake fails or times out.
pub async fn start_helper_sampler(
    config: &SamplerConfig,
    spawn: HelperSpawn,
) -> Result<SamplerHandle, SamplerError> {
    let socket_path = create_socket_path(config.pid);
    let stop_flag = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = oneshot::channel::<Result<String, SamplerError>>();
    let (sample_tx, sample_rx) = mpsc::channel::<SampleBatch>(SAMPLE_CHANNEL_DEPTH);

    let driver = DriverArgs {
        socket_path,
        spawn,
        pid: config.pid,
        rate: config.sample_rate,
        native: config.include_native,
        ready_tx,
        sample_tx,
        stop_flag: Arc::clone(&stop_flag),
    };

    let join_handle = std::thread::Builder::new()
        .name(format!("profiler-helper-{}", config.pid))
        .spawn(move || run_driver_thread(driver))
        .map_err(|err| {
            SamplerError::AttachFailed(format!("failed to spawn helper driver thread: {err}"))
        })?;

    match ready_rx.await {
        Ok(Ok(python_version)) => Ok(SamplerHandle::from_parts(
            stop_flag,
            join_handle,
            sample_rx,
            python_version,
            config.pid,
        )),
        Ok(Err(err)) => {
            let _ = join_handle.join();
            Err(err)
        }
        Err(_) => {
            let _ = join_handle.join();
            Err(SamplerError::AttachFailed(
                "helper driver exited before reporting readiness".to_owned(),
            ))
        }
    }
}

/// Everything the driver thread needs to run the socket protocol.
struct DriverArgs {
    socket_path: PathBuf,
    spawn: HelperSpawn,
    pid: u32,
    rate: u64,
    native: bool,
    ready_tx: oneshot::Sender<Result<String, SamplerError>>,
    sample_tx: mpsc::Sender<SampleBatch>,
    stop_flag: Arc<AtomicBool>,
}

/// Entry point for the dedicated driver thread: builds a single-threaded
/// runtime and drives the helper to completion.
fn run_driver_thread(args: DriverArgs) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            let _ = args.ready_tx.send(Err(SamplerError::AttachFailed(format!(
                "failed to build helper runtime: {err}"
            ))));
            return;
        }
    };
    runtime.block_on(drive_helper(args));
}

/// A live, attached helper connection.
struct HelperConn {
    child: tokio::process::Child,
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
    python_version: String,
}

/// Bind, spawn, accept, handshake, then stream until stop — cleaning up after.
async fn drive_helper(args: DriverArgs) {
    let DriverArgs {
        socket_path,
        spawn,
        pid,
        rate,
        native,
        ready_tx,
        sample_tx,
        stop_flag,
    } = args;

    // A stale socket file from a crashed run would make bind fail with EADDRINUSE.
    let _ = std::fs::remove_file(&socket_path);

    match handshake(&socket_path, &spawn, pid, rate, native).await {
        Err(err) => {
            let _ = ready_tx.send(Err(err));
            let _ = std::fs::remove_file(&socket_path);
        }
        Ok(mut conn) => {
            if ready_tx.send(Ok(conn.python_version.clone())).is_err() {
                // The caller gave up already; don't keep the helper running.
                stop_flag.store(true, Ordering::SeqCst);
            }
            stream_samples(
                &mut conn.reader,
                &mut conn.writer,
                pid,
                &sample_tx,
                &stop_flag,
            )
            .await;
            let _ = conn.child.start_kill();
            let _ = conn.child.wait().await;
            let _ = std::fs::remove_file(&socket_path);
        }
    }
}

/// Bind the socket, launch the helper, and complete the attach handshake.
async fn handshake(
    socket_path: &Path,
    spawn: &HelperSpawn,
    pid: u32,
    rate: u64,
    native: bool,
) -> Result<HelperConn, SamplerError> {
    // Bind BEFORE spawning so the helper's connect() always finds a listener.
    let listener = UnixListener::bind(socket_path).map_err(|err| {
        SamplerError::AttachFailed(format!(
            "failed to bind profiler socket {}: {err}",
            socket_path.display()
        ))
    })?;

    let child = spawn_helper(spawn, socket_path)?;

    let (stream, _addr) = match timeout(ACCEPT_TIMEOUT, listener.accept()).await {
        Ok(Ok(accepted)) => accepted,
        Ok(Err(err)) => {
            return Err(SamplerError::AttachFailed(format!(
                "accept on profiler socket failed: {err}"
            )))
        }
        Err(_elapsed) => {
            return Err(SamplerError::AttachFailed(
                "timed out waiting for the profiler helper to connect".to_owned(),
            ))
        }
    };

    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut writer = write_half;

    let attach = Command::Attach {
        pid,
        rate: Some(rate),
        native: Some(native),
    };
    write_message(&mut writer, &attach).await.map_err(|err| {
        SamplerError::AttachFailed(format!("failed to send attach command: {err}"))
    })?;

    let python_version =
        match timeout(HANDSHAKE_TIMEOUT, read_message::<_, Message>(&mut reader)).await {
            Ok(Ok(Some(Message::Attached { python, .. }))) => python,
            Ok(Ok(Some(Message::Samples { .. } | Message::Stopped))) => {
                return Err(SamplerError::AttachFailed(
                    "helper sent an unexpected message before confirming attach".to_owned(),
                ))
            }
            Ok(Ok(None)) => {
                return Err(SamplerError::AttachFailed(
                    "helper closed the connection before confirming attach".to_owned(),
                ))
            }
            Ok(Err(err)) => {
                return Err(SamplerError::AttachFailed(format!(
                    "attach handshake failed: {err}"
                )))
            }
            Err(_elapsed) => {
                return Err(SamplerError::AttachFailed(
                    "timed out waiting for the helper to confirm attach".to_owned(),
                ))
            }
        };

    info!(pid, %python_version, "elevated helper attached");
    Ok(HelperConn {
        child,
        reader,
        writer,
        python_version,
    })
}

/// Spawn the helper process per the requested mode (detached — never awaited).
///
/// `tokio::process::Command::spawn` returns immediately (it does not await the
/// child), so this is synchronous; it must still run inside a Tokio runtime,
/// which the driver thread provides.
fn spawn_helper(
    spawn: &HelperSpawn,
    socket_path: &Path,
) -> Result<tokio::process::Child, SamplerError> {
    match spawn {
        HelperSpawn::Direct(path) => tokio::process::Command::new(path)
            .arg(socket_path)
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| {
                SamplerError::AttachFailed(format!(
                    "failed to spawn profiler helper {}: {err}",
                    path.display()
                ))
            }),
        HelperSpawn::Elevated => spawn_elevated(socket_path),
    }
}

/// Launch the helper with administrator privileges via `osascript` (macOS).
#[cfg(target_os = "macos")]
fn spawn_elevated(socket_path: &Path) -> Result<tokio::process::Child, SamplerError> {
    let helper = find_helper_binary()?;
    let script = build_elevation_script(
        &helper.display().to_string(),
        &socket_path.display().to_string(),
    );
    info!(helper = %helper.display(), "spawning elevated profiler helper via osascript");
    tokio::process::Command::new("osascript")
        .args(["-e", &script])
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| {
            SamplerError::PermissionDenied(format!(
                "failed to spawn osascript for privilege elevation: {err}"
            ))
        })
}

/// Non-macOS fallback: elevation via the helper is macOS-only.
#[cfg(not(target_os = "macos"))]
fn spawn_elevated(_socket_path: &Path) -> Result<tokio::process::Child, SamplerError> {
    Err(SamplerError::PermissionDenied(
        "Privilege elevation via the profiler helper is only supported on macOS".to_owned(),
    ))
}

/// Locate the `basilisk-profiler-helper` binary next to the LSP or on `PATH`.
#[cfg(target_os = "macos")]
fn find_helper_binary() -> Result<PathBuf, SamplerError> {
    let helper_name = "basilisk-profiler-helper";

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(adjacent) = exe_path.parent().map(|dir| dir.join(helper_name)) {
            if adjacent.exists() {
                return Ok(adjacent);
            }
        }
    }

    let which = std::process::Command::new("which")
        .arg(helper_name)
        .output()
        .map_err(|err| {
            SamplerError::PermissionDenied(format!("failed to locate {helper_name}: {err}"))
        })?;

    if which.status.success() {
        return Ok(PathBuf::from(String::from_utf8_lossy(&which.stdout).trim()));
    }

    Err(SamplerError::PermissionDenied(format!(
        "could not find {helper_name}; ensure it is installed alongside the Basilisk LSP"
    )))
}

/// Stream samples from the helper, forwarding them to the sampler channel until
/// the helper stops on its own or a stop is requested.
///
/// The read side is never cancelled mid-message (newline framing is not
/// cancellation-safe): the stop signal is handled on the write side, and the
/// reader is then awaited to completion to drain the final `stopped`.
async fn stream_samples(
    reader: &mut BufReader<OwnedReadHalf>,
    writer: &mut OwnedWriteHalf,
    pid: u32,
    sample_tx: &mpsc::Sender<SampleBatch>,
    stop_flag: &Arc<AtomicBool>,
) {
    let read_loop = read_until_stopped(reader, pid, sample_tx);
    tokio::pin!(read_loop);

    let send_stop = async {
        let mut poll = tokio::time::interval(STOP_POLL_INTERVAL);
        loop {
            let _ = poll.tick().await;
            if stop_flag.load(Ordering::SeqCst) {
                if let Err(err) = write_message(writer, &Command::Stop).await {
                    warn!(%err, "failed to send stop command to helper");
                }
                break;
            }
        }
    };

    tokio::select! {
        () = &mut read_loop => {}
        () = send_stop => {
            // We asked the helper to stop; drain until it confirms (bounded).
            let _ = timeout(STOP_DRAIN_TIMEOUT, read_loop).await;
        }
    }
}

/// Read messages until the helper reports `stopped`, closes, or the receiver
/// is gone. Forwards each `samples` batch to the sampler channel.
async fn read_until_stopped(
    reader: &mut BufReader<OwnedReadHalf>,
    pid: u32,
    sample_tx: &mpsc::Sender<SampleBatch>,
) {
    loop {
        match read_message::<_, Message>(reader).await {
            Ok(Some(Message::Samples { traces })) => {
                let batch = SampleBatch {
                    traces: to_pyspy_traces(pid, traces),
                };
                if sample_tx.send(batch).await.is_err() {
                    break;
                }
            }
            Ok(Some(Message::Stopped) | None) => break,
            Ok(Some(Message::Attached { .. })) => {}
            Err(err) => {
                warn!(%err, "error reading from profiler helper");
                break;
            }
        }
    }
}
