//! LSP command handlers for profiling operations.
//!
//! Each handler extracts parameters from the `executeCommand` arguments,
//! delegates to `ProfileSessionManager`, and returns JSON results matching
//! the protocol defined in `LSP-PROFILING-SPEC.md`.

use tower_lsp::jsonrpc::{Error as LspError, ErrorCode, Result as LspResult};
use tower_lsp::lsp_types::MessageType;
use tracing::{error, info};

use super::aggregator::HotspotConfig;
use super::diagnostics;
use super::export::{self, ExportFormat};
use super::ProfileSessionManager;

/// Default sample rate (Hz) when not specified by the client.
const DEFAULT_SAMPLE_RATE: u32 = 100;

/// Default export format when not specified.
const DEFAULT_FORMAT: &str = "speedscope";

/// Handle `basilisk.profiler.start`.
///
/// Params (first arg JSON object):
/// - `pid`: u32 (required)
/// - `sampleRate`: u32 (optional, default 100)
/// - `includeNative`: bool (optional, default false)
/// - `duration`: f64 seconds (optional)
pub async fn execute_profiler_start(
    profiler: &ProfileSessionManager,
    client: &tower_lsp::Client,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let arg = args.first().unwrap_or(&serde_json::Value::Null);

    let Some(pid) = arg.get("pid").and_then(|v| v.as_u64()).map(|v| v as u32) else {
        return Err(LspError {
            code: ErrorCode::InvalidParams,
            message: "Missing required parameter: pid".into(),
            data: None,
        });
    };

    let sample_rate = arg
        .get("sampleRate")
        .and_then(|v| v.as_u64())
        .map_or(DEFAULT_SAMPLE_RATE, |v| v as u32);

    let include_native = arg
        .get("includeNative")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    info!(pid, sample_rate, include_native, "profiler start requested");

    match profiler.start(pid, sample_rate, include_native).await {
        Ok(info) => {
            client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Basilisk: profiling PID {} (Python {}, {}Hz)",
                        pid, info.python_version, sample_rate
                    ),
                )
                .await;

            Ok(Some(serde_json::json!({
                "sessionId": info.session_id,
                "pid": info.pid,
                "pythonVersion": info.python_version,
                "startedAt": format!("{:?}", info.started_at),
            })))
        }
        Err(err) => {
            error!(pid, %err, "failed to start profiling");
            client
                .log_message(MessageType::ERROR, err.to_string())
                .await;

            let code = match &err {
                super::ProfileError::ProcessNotFound(_) => -32001,
                super::ProfileError::NotPython(_) => -32002,
                super::ProfileError::PermissionDenied(_) => -32003,
                super::ProfileError::AlreadyProfiling(_, _) => -32004,
                _ => -32000,
            };

            Err(LspError {
                code: ErrorCode::ServerError(code),
                message: err.to_string().into(),
                data: None,
            })
        }
    }
}

/// Handle `basilisk.profiler.stop`.
///
/// Params (first arg JSON object):
/// - `sessionId`: string (required)
/// - `format`: "speedscope" | "flamegraph" | "summary" (optional, default "speedscope")
pub async fn execute_profiler_stop(
    profiler: &ProfileSessionManager,
    client: &tower_lsp::Client,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let arg = args.first().unwrap_or(&serde_json::Value::Null);

    let Some(session_id) = arg.get("sessionId").and_then(|v| v.as_str()) else {
        return Err(LspError {
            code: ErrorCode::InvalidParams,
            message: "Missing required parameter: sessionId".into(),
            data: None,
        });
    };

    let format_str = arg
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_FORMAT);

    info!(session_id, format = format_str, "profiler stop requested");

    match profiler.stop(session_id).await {
        Ok((info, data)) => {
            let duration = info.started_at.elapsed().as_secs_f64();
            let config = HotspotConfig::default();
            let hot_funcs = data.hot_functions(&config);
            let hot_lines = data.hot_lines(&config);

            // Export to file.
            let output_dir = std::env::temp_dir();
            let export_format = match format_str {
                "flamegraph" => ExportFormat::Flamegraph,
                _ => ExportFormat::Speedscope,
            };

            let output_file = if format_str != "summary" {
                match export::export(
                    &data,
                    export_format,
                    &info.session_id,
                    info.pid,
                    duration,
                    &output_dir,
                ) {
                    Ok(result) => Some(result.path.display().to_string()),
                    Err(err) => {
                        error!(%err, "failed to export profile data");
                        None
                    }
                }
            } else {
                None
            };

            // Generate and publish diagnostics.
            let diag_map = diagnostics::generate_diagnostics(&data, &config);
            for (uri, diags) in &diag_map {
                client.publish_diagnostics(uri.clone(), diags.clone(), None).await;
            }

            client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Basilisk: profiling stopped \u{2014} {} samples in {:.1}s",
                        data.total_samples, duration
                    ),
                )
                .await;

            let hot_funcs_json: Vec<serde_json::Value> = hot_funcs
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
                .collect();

            let hot_lines_json: Vec<serde_json::Value> = hot_lines
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
                .collect();

            Ok(Some(serde_json::json!({
                "sessionId": info.session_id,
                "duration": duration,
                "totalSamples": data.total_samples,
                "outputFile": output_file,
                "hotFunctions": hot_funcs_json,
                "hotLines": hot_lines_json,
            })))
        }
        Err(err) => {
            error!(session_id, %err, "failed to stop profiling");
            Err(LspError {
                code: ErrorCode::ServerError(-32000),
                message: err.to_string().into(),
                data: None,
            })
        }
    }
}

/// Handle `basilisk.profiler.snapshot`.
///
/// Params (first arg JSON object):
/// - `sessionId`: string (required)
pub async fn execute_profiler_snapshot(
    profiler: &ProfileSessionManager,
    client: &tower_lsp::Client,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let arg = args.first().unwrap_or(&serde_json::Value::Null);

    let Some(session_id) = arg.get("sessionId").and_then(|v| v.as_str()) else {
        return Err(LspError {
            code: ErrorCode::InvalidParams,
            message: "Missing required parameter: sessionId".into(),
            data: None,
        });
    };

    info!(session_id, "profiler snapshot requested");

    match profiler.snapshot(session_id).await {
        Ok((info, data)) => {
            let duration = info.started_at.elapsed().as_secs_f64();
            let config = HotspotConfig::default();

            // Publish diagnostics from snapshot.
            let diag_map = diagnostics::generate_diagnostics(&data, &config);
            for (uri, diags) in &diag_map {
                client.publish_diagnostics(uri.clone(), diags.clone(), None).await;
            }

            let hot_funcs = data.hot_functions(&config);
            let hot_funcs_json: Vec<serde_json::Value> = hot_funcs
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
                .collect();

            Ok(Some(serde_json::json!({
                "sessionId": info.session_id,
                "duration": duration,
                "totalSamples": data.total_samples,
                "hotFunctions": hot_funcs_json,
            })))
        }
        Err(err) => {
            error!(session_id, %err, "failed to take snapshot");
            Err(LspError {
                code: ErrorCode::ServerError(-32000),
                message: err.to_string().into(),
                data: None,
            })
        }
    }
}

/// Handle `basilisk.profiler.list`.
///
/// No parameters required.
pub async fn execute_profiler_list(
    profiler: &ProfileSessionManager,
) -> LspResult<Option<serde_json::Value>> {
    let sessions = profiler.list().await;

    let sessions_json: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| {
            serde_json::json!({
                "sessionId": s.session_id,
                "pid": s.pid,
                "sampleRate": s.sample_rate,
                "duration": s.started_at.elapsed().as_secs_f64(),
            })
        })
        .collect();

    Ok(Some(serde_json::json!({
        "sessions": sessions_json,
    })))
}
