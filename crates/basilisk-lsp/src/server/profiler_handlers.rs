//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! LSP command handlers for `basilisk.profiler.*` profiling commands.
//!
//! Each handler extracts arguments, delegates to [`ProfileSessionManager`],
//! and returns structured JSON responses matching the profiling protocol spec.

use std::time::Duration;

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::MessageType;
use tracing::{debug, error, info};

use super::LspServer;

/// How often live progress is pushed to the editor while profiling.
const PROGRESS_NOTIFY_INTERVAL: Duration = Duration::from_secs(1);

/// Custom notification carrying live profiling progress.
struct ProfilerProgressNotification;

impl tower_lsp::lsp_types::notification::Notification for ProfilerProgressNotification {
    type Params = serde_json::Value;
    const METHOD: &'static str = basilisk_common::notifications::PROFILER_PROGRESS;
}

/// Implements [PROFILE-NOTIFICATIONS-PROGRESS]: push periodic progress
/// (sample count, duration, top function) until the session ends, so editors
/// can show a live status indicator while profiling.
fn spawn_progress_notifier(server: &LspServer, session_id: String) {
    let manager = server.profiler_manager.clone();
    let client = server.client.clone();
    let _notifier = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(PROGRESS_NOTIFY_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let _tick = ticker.tick().await;
            let Some(progress) = manager.progress(&session_id).await else {
                debug!(%session_id, "profiling session ended; stopping progress notifier");
                break;
            };
            let params = serde_json::json!({
                "sessionId": progress.session_id,
                "sampleCount": progress.sample_count,
                "duration": progress.duration,
                "topFunction": progress.top_function,
            });
            client
                .send_notification::<ProfilerProgressNotification>(params)
                .await;
        }
    });
}

/// Construct a profiler-domain LSP error.
fn profiler_error(code: i64, message: impl Into<String>) -> tower_lsp::jsonrpc::Error {
    tower_lsp::jsonrpc::Error {
        code: tower_lsp::jsonrpc::ErrorCode::ServerError(code),
        message: message.into().into(),
        data: None,
    }
}

/// Handle `basilisk.profiler.start` — begin profiling a Python process.
///
/// Accepts a JSON object with:
/// - `pid` (u32, required): target process ID
/// - `sampleRate` (u64, optional): samples per second (default 100)
/// - `includeNative` (bool, optional): include C extension frames (default false)
/// - `duration` (f64, optional): auto-stop after N seconds
pub(super) async fn execute_profiler_start(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_profiler_start called");

    let arg = args
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let pid = arg
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .map(|p| u32::try_from(p).unwrap_or(0));

    let Some(target_pid) = pid else {
        return Err(profiler_error(-32001, "Missing required parameter: pid"));
    };

    // Check for preset-based configuration first.
    let preset_name = arg.get("preset").and_then(serde_json::Value::as_str);

    let (sample_rate, include_native, duration) = if let Some(preset) =
        preset_name.and_then(crate::profiler::presets::ProfilingPreset::parse_name)
    {
        let config = crate::profiler::presets::preset_config(preset, target_pid);
        info!(preset = ?preset, "using profiling preset");
        (
            Some(config.sample_rate),
            Some(config.include_native),
            config.duration,
        )
    } else {
        let rate = arg.get("sampleRate").and_then(serde_json::Value::as_u64);
        let native = arg
            .get("includeNative")
            .and_then(serde_json::Value::as_bool);
        let dur = arg
            .get("duration")
            .and_then(serde_json::Value::as_f64)
            .map(std::time::Duration::from_secs_f64);
        (rate, native, dur)
    };

    match server
        .profiler_manager
        .start(target_pid, sample_rate, include_native, duration)
        .await
    {
        Ok(result) => {
            server
                .client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Basilisk: profiling PID {} (Python {})",
                        result.pid, result.python_version
                    ),
                )
                .await;

            spawn_progress_notifier(server, result.session_id.clone());

            Ok(Some(serde_json::json!({
                "sessionId": result.session_id,
                "pid": result.pid,
                "pythonVersion": result.python_version,
                "startedAt": result.started_at,
            })))
        }
        Err(err) => {
            let code = err.error_code();
            error!(pid = target_pid, %err, "profiler start failed");
            server
                .client
                .log_message(MessageType::ERROR, format!("Basilisk: {err}"))
                .await;
            Err(profiler_error(code, err.to_string()))
        }
    }
}

/// Handle `basilisk.profiler.cooperativeScript` — mint the in-process
/// sampling script for the active debug session (leg 1 of the courier
/// round-trip). Implements [PROFILE-COOPERATIVE].
///
/// Accepts `sampleRate` (u64, optional, default 100). Returns
/// `{ script, sampleFile, sampleRate }`; the editor evaluates `script` in the
/// paused debuggee, resumes it, then calls `cooperativeAttach`.
pub(super) fn execute_profiler_cooperative_script(
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let arg = args.first().cloned().unwrap_or_default();
    let sample_rate = arg
        .get("sampleRate")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(100);

    let sample_file = crate::profiler::cooperative::mint_sample_file();
    let script = crate::profiler::cooperative::sampling_script(&sample_file, sample_rate)
        .map_err(|err| profiler_error(-32000, err.to_string()))?;

    info!(sample_rate, file = %sample_file.display(), "cooperative sampling script minted");
    Ok(Some(serde_json::json!({
        "script": script,
        "sampleFile": sample_file.display().to_string(),
        "sampleRate": sample_rate,
    })))
}

/// Handle `basilisk.profiler.cooperativeAttach` — adopt the cooperative
/// sample stream after the editor injected the script (leg 2). Implements
/// [PROFILE-COOPERATIVE]; returns the same shape as `profiler.start`.
pub(super) async fn execute_profiler_cooperative_attach(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let arg = args.first().cloned().unwrap_or_default();
    let Some(sample_file) = arg.get("sampleFile").and_then(serde_json::Value::as_str) else {
        return Err(profiler_error(
            -32602,
            "Missing required parameter: sampleFile",
        ));
    };
    if !std::path::Path::new(sample_file)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("basilisk-cpu-"))
    {
        return Err(profiler_error(
            -32602,
            "sampleFile must be a basilisk-cpu sample path minted by cooperativeScript",
        ));
    }
    let sample_rate = arg.get("sampleRate").and_then(serde_json::Value::as_u64);

    match server
        .profiler_manager
        .start_cooperative(std::path::PathBuf::from(sample_file), sample_rate)
        .await
    {
        Ok(result) => {
            spawn_progress_notifier(server, result.session_id.clone());
            Ok(Some(serde_json::json!({
                "sessionId": result.session_id,
                "pid": result.pid,
                "pythonVersion": result.python_version,
                "startedAt": result.started_at,
            })))
        }
        Err(err) => {
            error!(%err, "cooperative attach failed");
            Err(profiler_error(err.error_code(), err.to_string()))
        }
    }
}

/// Handle `basilisk.profiler.stop` — stop profiling and return results.
///
/// Accepts a JSON object with:
/// - `sessionId` (string, required): the session to stop
/// - `format` (string, optional): `"speedscope"` (default), `"flamegraph"`, `"summary"`
pub(super) async fn execute_profiler_stop(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_profiler_stop called");

    let arg = args
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let Some(session_id) = arg.get("sessionId").and_then(serde_json::Value::as_str) else {
        return Err(profiler_error(
            -32001,
            "Missing required parameter: sessionId",
        ));
    };

    let format_str = arg
        .get("format")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("speedscope");

    match server.profiler_manager.stop(session_id).await {
        Ok(result) => {
            let artifacts = export_stop_artifacts(&result, format_str);

            publish_profiler_diagnostics(server, &result.data, &result.hotspot_config).await;

            server
                .client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Basilisk: profiling stopped — {} samples in {:.1}s",
                        result.total_samples, result.duration
                    ),
                )
                .await;

            let hot_funcs_json = build_hot_functions_json(&result.hot_functions);
            let hot_lines_json = build_hot_lines_json(&result.hot_lines);

            Ok(Some(serde_json::json!({
                "sessionId": result.session_id,
                "duration": result.duration,
                "totalSamples": result.total_samples,
                "outputFile": artifacts.output_file,
                "flamegraphPath": artifacts.flamegraph_path,
                "cpuProfilePath": artifacts.cpu_profile_path,
                "exportError": artifacts.export_error,
                "hotFunctions": hot_funcs_json,
                "hotLines": hot_lines_json,
            })))
        }
        Err(err) => {
            let code = err.error_code();
            error!(%err, "profiler stop failed");
            Err(profiler_error(code, err.to_string()))
        }
    }
}

/// Export all stop artifacts to the temp dir ([PROFILE-REQUESTS-STOP]).
fn export_stop_artifacts(
    result: &crate::profiler::StopResult,
    format_str: &str,
) -> crate::profiler::export::StopArtifacts {
    crate::profiler::export::export_stop_artifacts(
        &result.data,
        &result.session_id,
        result.duration,
        result.sample_rate,
        format_str,
        &std::env::temp_dir(),
    )
}

/// Handle `basilisk.profiler.snapshot` — take a point-in-time snapshot.
///
/// Accepts a JSON object with:
/// - `sessionId` (string, required): the session to snapshot
pub(super) async fn execute_profiler_snapshot(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_profiler_snapshot called");

    let arg = args
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let Some(session_id) = arg.get("sessionId").and_then(serde_json::Value::as_str) else {
        return Err(profiler_error(
            -32001,
            "Missing required parameter: sessionId",
        ));
    };

    match server.profiler_manager.snapshot(session_id).await {
        Ok(result) => {
            // Publish diagnostics from snapshot.
            publish_profiler_diagnostics(server, &result.data, &result.hotspot_config).await;

            server
                .client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Basilisk: snapshot — {} samples so far",
                        result.total_samples
                    ),
                )
                .await;

            let hot_funcs_json = build_hot_functions_json(&result.hot_functions);

            Ok(Some(serde_json::json!({
                "sessionId": result.session_id,
                "duration": result.duration,
                "totalSamples": result.total_samples,
                "hotFunctions": hot_funcs_json,
            })))
        }
        Err(err) => {
            let code = err.error_code();
            error!(%err, "profiler snapshot failed");
            Err(profiler_error(code, err.to_string()))
        }
    }
}

/// Publish profiling diagnostics for all files with hotspots.
async fn publish_profiler_diagnostics(
    server: &LspServer,
    data: &crate::profiler::aggregator::ProfileData,
    hotspot_config: &crate::profiler::aggregator::HotspotConfig,
) {
    let diag_map = crate::profiler::diagnostics::generate_diagnostics(data, hotspot_config);
    for (uri, diags) in &diag_map {
        server
            .client
            .publish_diagnostics(uri.clone(), diags.clone(), None)
            .await;
    }
}

/// Build JSON array for hot functions (top 10).
fn build_hot_functions_json(
    hot_functions: &[crate::profiler::aggregator::HotFunction],
) -> Vec<serde_json::Value> {
    hot_functions
        .iter()
        .take(10)
        .map(|f| {
            serde_json::json!({
                "name": f.name,
                "file": f.file,
                "line": f.line,
                "samples": f.samples,
                "percentage": f.percentage,
                "selfPercentage": f.self_percentage,
            })
        })
        .collect()
}

/// Build JSON array for hot lines (top 10).
fn build_hot_lines_json(
    hot_lines: &[crate::profiler::aggregator::HotLine],
) -> Vec<serde_json::Value> {
    hot_lines
        .iter()
        .take(10)
        .map(|l| {
            serde_json::json!({
                "file": l.file,
                "line": l.line,
                "samples": l.samples,
                "percentage": l.percentage,
            })
        })
        .collect()
}

/// Handle `basilisk.profiler.processes` — enumerate attachable Python processes.
///
/// Implements [PROFILE-PROCESSES-LSP]. This is the headline fix for #62: the LSP
/// enumerates running Python processes so the editor's panel can offer one-click
/// profiling instead of a raw PID input box. Enumeration only reads the process
/// table and never requires elevation. Returns `{ "processes": ProcessInfo[] }`.
pub(super) async fn execute_profiler_processes(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_profiler_processes called");

    // Enumeration is system-wide and zero-filter ([PROFILE-PROCESSES-SCOPE]); the
    // roots are passed only to compute each row's `in_workspace` flag (the green
    // row hint), never to exclude a process.
    let roots = server.workspace_roots.read().await.clone();

    // Enumeration blocks (~200ms for CPU sampling + `--version` probes), so run
    // it off the async runtime to avoid stalling other LSP requests.
    let processes = match tokio::task::spawn_blocking(move || {
        crate::profiler::processes::enumerate_python_processes(&roots)
    })
    .await
    {
        Ok(list) => list,
        Err(err) => {
            error!(%err, "process enumeration task failed");
            Vec::new()
        }
    };

    info!(count = processes.len(), "returning python processes");
    Ok(Some(serde_json::json!({ "processes": processes })))
}

/// Handle `basilisk.profiler.list` — list active profiling sessions.
pub(super) async fn execute_profiler_list(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_profiler_list called");

    let sessions = server.profiler_manager.list().await;

    let sessions_json: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| {
            serde_json::json!({
                "sessionId": s.session_id,
                "pid": s.pid,
                "startedAt": s.started_at,
                "sampleCount": s.sample_count,
                "duration": s.duration,
            })
        })
        .collect();

    Ok(Some(serde_json::json!({
        "sessions": sessions_json,
    })))
}

/// Check if a debug session start request includes `profileOnLaunch: true`.
///
/// If so, begin profiling the debuggee PID automatically. Called from the debug
/// session start handler after debugpy spawns the target process.
///
/// # Returns
///
/// `Some(session_id)` if profiling was started, `None` if `profileOnLaunch` was
/// not set or profiling failed (the debug session continues regardless).
pub(super) async fn maybe_profile_on_launch(
    server: &LspServer,
    args: &serde_json::Value,
    debuggee_pid: u32,
) -> Option<String> {
    let profile_on_launch = args
        .get("profileOnLaunch")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    if !profile_on_launch {
        return None;
    }

    info!(debuggee_pid, "profileOnLaunch: auto-starting profiler");

    let preset_name = args
        .get("profilePreset")
        .and_then(serde_json::Value::as_str);
    let (sample_rate, duration) = resolve_launch_profile_config(args, preset_name, debuggee_pid);

    match server
        .profiler_manager
        .start(debuggee_pid, Some(sample_rate), Some(false), duration)
        .await
    {
        Ok(result) => {
            server
                .client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Basilisk: auto-profiling PID {} (Python {})",
                        result.pid, result.python_version
                    ),
                )
                .await;
            Some(result.session_id)
        }
        Err(err) => {
            error!(debuggee_pid, %err, "profileOnLaunch: failed to start profiler");
            server
                .client
                .log_message(
                    MessageType::WARNING,
                    format!("Basilisk: auto-profiling failed: {err}"),
                )
                .await;
            None
        }
    }
}

/// Resolve sample rate and duration from launch args or preset.
fn resolve_launch_profile_config(
    args: &serde_json::Value,
    preset_name: Option<&str>,
    pid: u32,
) -> (u64, Option<std::time::Duration>) {
    if let Some(preset) =
        preset_name.and_then(crate::profiler::presets::ProfilingPreset::parse_name)
    {
        let config = crate::profiler::presets::preset_config(preset, pid);
        (config.sample_rate, config.duration)
    } else {
        let rate = args
            .get("profileSampleRate")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(100);
        let dur = args
            .get("profileDuration")
            .and_then(serde_json::Value::as_f64)
            .map(std::time::Duration::from_secs_f64);
        (rate, dur)
    }
}
