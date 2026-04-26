//! Test discovery and execution command handlers for the Basilisk LSP server.
//!
//! Covers `basilisk.discoverTests`, `basilisk.runTests`, `basilisk.runTestFile`,
//! and `basilisk.debugTest`.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, MessageType, NumberOrString, Position, Range,
};
use tracing::{error, info, warn};

use super::LspServer;

/// Handle `basilisk.discoverTests`.
///
/// Discovers tests in the workspace or a specific file. Accepts an optional
/// `{ uri: string }` argument to scope discovery to one file.
pub(super) async fn execute_discover_tests(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("execute_discover_tests called");

    let roots = server.workspace_roots.read().await;
    let Some(root) = roots.first().cloned() else {
        warn!("discoverTests: no workspace root available");
        return Ok(Some(serde_json::json!({ "items": [] })));
    };
    drop(roots);

    // If a URI is provided, discover tests in that file only.
    if let Some(uri_str) = args
        .first()
        .and_then(|v| v.get("uri"))
        .and_then(|v| v.as_str())
    {
        let Ok(uri) = tower_lsp::lsp_types::Url::parse(uri_str) else {
            return Ok(Some(serde_json::json!({ "items": [] })));
        };
        let Ok(path) = uri.to_file_path() else {
            return Ok(Some(serde_json::json!({ "items": [] })));
        };

        let source = server
            .with_index(|idx| idx.get_text(&uri))
            .await
            .unwrap_or_default();

        if source.is_empty() {
            return Ok(Some(serde_json::json!({ "items": [] })));
        }

        let items = crate::test_discovery::discover_tests_in_file(&path, &source);
        info!(uri = %uri, count = items.len(), "discovered tests in file");
        return Ok(Some(serde_json::json!({ "items": items })));
    }

    // Full workspace discovery.
    let items = crate::test_discovery::discover_workspace_tests(&root);
    let count: usize = items
        .iter()
        .map(|file_item| file_item.children.len() + 1)
        .sum();
    info!(count, "discovered tests in workspace");

    Ok(Some(serde_json::json!({ "items": items })))
}

/// Handle `basilisk.runTests`.
///
/// Runs one or more tests by node ID. Args: `{ testIds: string[] }`.
pub(super) async fn execute_run_tests(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let test_ids = extract_test_ids(args);
    info!(count = test_ids.len(), "execute_run_tests called");

    let (root, pytest_path, extra_args, use_uv_run) = read_test_config(server).await;

    let run_config = crate::test_discovery::TestRunConfig {
        root: &root,
        test_ids: &test_ids,
        pytest_path: &pytest_path,
        extra_args: &extra_args,
        use_uv_run,
        enable_coverage: false,
    };

    run_and_report(server, &run_config).await
}

/// Handle `basilisk.runTestFile`.
///
/// Runs all tests in the current file. Args: `uri` string.
pub(super) async fn execute_run_test_file(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(uri_str) = args.first().and_then(|v| v.as_str()) else {
        warn!("runTestFile: missing URI argument");
        return Ok(None);
    };
    info!(uri = uri_str, "execute_run_test_file called");

    let Ok(uri) = tower_lsp::lsp_types::Url::parse(uri_str) else {
        return Ok(None);
    };
    let Ok(path) = uri.to_file_path() else {
        return Ok(None);
    };

    let (root, pytest_path, extra_args, use_uv_run) = read_test_config(server).await;
    let relative = path
        .strip_prefix(&root)
        .unwrap_or(&path)
        .to_string_lossy()
        .into_owned();

    let test_ids = vec![relative];
    let run_config = crate::test_discovery::TestRunConfig {
        root: &root,
        test_ids: &test_ids,
        pytest_path: &pytest_path,
        extra_args: &extra_args,
        use_uv_run,
        enable_coverage: false,
    };

    run_and_report(server, &run_config).await
}

/// Handle `basilisk.debugTest`.
///
/// Starts a debug session targeting a specific test. Args: `{ testId: string }`.
/// Returns the debug session connection details (host, port, sessionId) like
/// `basilisk.startDebugSession`.
pub(super) async fn execute_debug_test(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let test_id = args
        .first()
        .and_then(|v| v.get("testId"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    info!(test_id, "execute_debug_test called");

    if test_id.is_empty() {
        warn!("debugTest: missing testId argument");
        return Ok(None);
    }

    // Resolve python interpreter the same way as startDebugSession.
    let workspace = server.workspace_roots.read().await;
    let root = workspace
        .first()
        .map_or_else(|| std::path::Path::new("."), std::path::PathBuf::as_path);
    let python = crate::debug::resolve_python(root);
    drop(workspace);

    // Verify debugpy is installed.
    if let Err(err) = crate::debug::check_debugpy(&python).await {
        error!(python = %python, %err, "debugpy check failed for test debug");
        server
            .client
            .log_message(MessageType::ERROR, err.to_string())
            .await;
        return Err(tower_lsp::jsonrpc::Error {
            code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32012),
            message: err.to_string().into(),
            data: None,
        });
    }

    match server.debug_manager.start_session(&python).await {
        Ok((host, port, session_id)) => {
            info!(
                host = %host, port, session_id = %session_id,
                test_id, "debug session started for test"
            );
            server
                .client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Basilisk: debug session {session_id} for test '{test_id}' on {host}:{port}"
                    ),
                )
                .await;
            Ok(Some(serde_json::json!({
                "host": host,
                "port": port,
                "sessionId": session_id,
                "testId": test_id,
            })))
        }
        Err(err) => {
            error!(%err, "failed to start debug session for test");
            server
                .client
                .log_message(MessageType::ERROR, err.to_string())
                .await;
            Err(tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32013),
                message: err.to_string().into(),
                data: None,
            })
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract test IDs from the first argument's `testIds` array.
fn extract_test_ids(args: &[serde_json::Value]) -> Vec<String> {
    args.first()
        .and_then(|v| v.get("testIds"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Read test configuration from workspace roots and the live test explorer config.
async fn read_test_config(server: &LspServer) -> (std::path::PathBuf, String, Vec<String>, bool) {
    let roots = server.workspace_roots.read().await;
    let root = roots
        .first()
        .cloned()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    drop(roots);

    let config = server.test_config.read().await;
    let pytest_path = config.pytest_path.clone();
    let extra_args = config.args.clone();
    let use_uv_run = config.use_uv_run;

    (root, pytest_path, extra_args, use_uv_run)
}

/// Run tests and report results. Shared by `execute_run_tests` and `execute_run_test_file`.
async fn run_and_report(
    server: &LspServer,
    run_config: &crate::test_discovery::TestRunConfig<'_>,
) -> LspResult<Option<serde_json::Value>> {
    match crate::test_discovery::run_tests(run_config) {
        Ok(result) => {
            let status = if result.passed { "passed" } else { "failed" };
            info!(status, exit_code = result.exit_code, "test run complete");
            server
                .client
                .log_message(
                    MessageType::INFO,
                    format!("Basilisk: tests {status} (exit code {})", result.exit_code),
                )
                .await;

            // If coverage was enabled, parse and send coverage notification.
            if run_config.enable_coverage {
                let coverage_path = run_config.root.join(".basilisk").join("coverage.xml");
                send_coverage_notification(server, &coverage_path).await;
            }

            Ok(Some(serde_json::to_value(&result).unwrap_or_default()))
        }
        Err(err) => {
            error!(%err, "failed to run tests");
            server
                .client
                .log_message(MessageType::ERROR, format!("Basilisk: {err}"))
                .await;
            Err(tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32010),
                message: err.into(),
                data: None,
            })
        }
    }
}

/// Handle `basilisk.runTestsCoverage`.
///
/// Runs tests with `--cov` and `--cov-report=xml` to generate coverage data.
/// After the run, parses `coverage.xml` and sends a `basilisk/coverageResult`
/// notification to the client for gutter decoration rendering.
pub(super) async fn execute_run_tests_coverage(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let test_ids = extract_test_ids(args);
    info!(count = test_ids.len(), "execute_run_tests_coverage called");

    // Check pytest-cov availability before running.
    check_pytest_cov_availability(server).await;

    let (root, pytest_path, extra_args, use_uv_run) = read_test_config(server).await;

    // Ensure .basilisk directory exists for coverage output.
    let basilisk_dir = root.join(".basilisk");
    if !basilisk_dir.is_dir() {
        let _ = std::fs::create_dir_all(&basilisk_dir);
    }

    let run_config = crate::test_discovery::TestRunConfig {
        root: &root,
        test_ids: &test_ids,
        pytest_path: &pytest_path,
        extra_args: &extra_args,
        use_uv_run,
        enable_coverage: true,
    };

    run_and_report(server, &run_config).await
}

/// Parse coverage.xml and send a `basilisk/coverageResult` notification.
async fn send_coverage_notification(server: &LspServer, coverage_path: &std::path::Path) {
    match crate::coverage::parse_coverage_xml(coverage_path) {
        Ok(result) => {
            info!(
                files = result.files.len(),
                total_pct = format!("{:.1}%", result.total_pct),
                "coverage parsed"
            );
            let value = serde_json::to_value(&result).unwrap_or_default();
            server
                .client
                .send_notification::<CoverageResultNotification>(value)
                .await;
        }
        Err(err) => {
            warn!(%err, "failed to parse coverage XML");
        }
    }
}

/// Custom notification type for coverage results.
struct CoverageResultNotification;

impl tower_lsp::lsp_types::notification::Notification for CoverageResultNotification {
    type Params = serde_json::Value;
    const METHOD: &'static str = basilisk_common::coverage_notifications::COVERAGE_RESULT;
}

/// Diagnostic code for pytest not found in uv project.
pub(crate) const PYTEST_NOT_FOUND_CODE: &str = "BSK-W0014";

/// Check if pytest is available in the uv package registry.
///
/// If a uv project is detected but pytest is not in the lock file, sends a
/// warning to the client suggesting `uv add --dev pytest` and publishes a
/// `BSK-W0014` diagnostic on each discovered test file so the code action
/// system can offer a quick-fix.
pub(super) async fn check_pytest_availability(server: &LspServer) {
    let roots = server.workspace_roots.read().await;
    let root = roots.first().cloned();
    let is_uv = basilisk_uv::detect_uv_project(&roots).is_some();
    drop(roots);

    if !is_uv {
        return;
    }

    let has_pytest = server
        .with_index(|idx| {
            Some(
                idx.registry
                    .as_ref()
                    .is_none_or(|reg| reg.has_package("pytest")),
            )
        })
        .await
        .unwrap_or(true);

    if !has_pytest {
        server
            .client
            .log_message(
                MessageType::WARNING,
                "Basilisk: pytest not found in uv.lock. Run `uv add --dev pytest` to install it."
                    .to_owned(),
            )
            .await;

        // Publish BSK-W0014 diagnostic on each test file so the code action
        // system can offer a "uv add --dev pytest" quick-fix.
        if let Some(root) = root {
            let test_files = crate::test_discovery::discover_test_files(&root);
            for path in test_files {
                let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&path) else {
                    continue;
                };
                let diag = make_pytest_not_found_diagnostic();
                server
                    .client
                    .publish_diagnostics(uri, vec![diag], None)
                    .await;
            }
        }
    }
}

/// Build a `BSK-W0014` diagnostic for a test file missing pytest.
fn make_pytest_not_found_diagnostic() -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(PYTEST_NOT_FOUND_CODE.to_owned())),
        code_description: None,
        source: Some("basilisk".to_owned()),
        message: "pytest not found in uv.lock — run `uv add --dev pytest` to install it".to_owned(),
        tags: None,
        related_information: None,
        data: None,
    }
}

/// Check if pytest-cov is available in the uv package registry.
///
/// Sends an info message if coverage is requested but pytest-cov is missing.
pub(super) async fn check_pytest_cov_availability(server: &LspServer) {
    let roots = server.workspace_roots.read().await;
    let is_uv = basilisk_uv::detect_uv_project(&roots).is_some();
    if !is_uv {
        return;
    }

    let has_pytest_cov = server
        .with_index(|idx| {
            Some(
                idx.registry
                    .as_ref()
                    .is_none_or(|reg| reg.has_package("pytest_cov")),
            )
        })
        .await
        .unwrap_or(true);

    if !has_pytest_cov {
        server
            .client
            .log_message(
                MessageType::INFO,
                "Basilisk: pytest-cov not found in uv.lock. Run `uv add --dev pytest-cov` for coverage support."
                    .to_owned(),
            )
            .await;
    }
}

/// Send test discovery results as a notification to the client.
///
/// Called after workspace initialization and on file save for test files.
pub(super) async fn send_test_discovery_notification(
    server: &LspServer,
    items: Vec<crate::test_discovery::TestItem>,
) {
    let value = serde_json::json!({ "items": items });
    server
        .client
        .send_notification::<TestDiscoveryNotification>(value)
        .await;
}

/// Custom notification type for test discovery results.
struct TestDiscoveryNotification;

impl tower_lsp::lsp_types::notification::Notification for TestDiscoveryNotification {
    type Params = serde_json::Value;
    const METHOD: &'static str = "basilisk/testDiscoveryResult";
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test-only code: expect and indexing acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn extract_test_ids_valid_array() {
        let args = vec![serde_json::json!({"testIds": ["test_a", "test_b"]})];
        let ids = extract_test_ids(&args);
        assert_eq!(ids, vec!["test_a", "test_b"]);
    }

    #[test]
    fn extract_test_ids_empty_array() {
        let args = vec![serde_json::json!({"testIds": []})];
        let ids = extract_test_ids(&args);
        assert!(ids.is_empty());
    }

    #[test]
    fn extract_test_ids_missing_field() {
        let args = vec![serde_json::json!({"other": "value"})];
        let ids = extract_test_ids(&args);
        assert!(ids.is_empty());
    }

    #[test]
    fn extract_test_ids_no_args() {
        let ids = extract_test_ids(&[]);
        assert!(ids.is_empty());
    }

    #[test]
    fn extract_test_ids_filters_non_strings() {
        let args = vec![serde_json::json!({"testIds": ["test_a", 42, "test_b"]})];
        let ids = extract_test_ids(&args);
        assert_eq!(ids, vec!["test_a", "test_b"]);
    }

    #[test]
    fn make_pytest_not_found_diagnostic_has_correct_code() {
        let diag = make_pytest_not_found_diagnostic();
        assert_eq!(
            diag.code,
            Some(NumberOrString::String(PYTEST_NOT_FOUND_CODE.to_owned()))
        );
        assert_eq!(diag.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(diag.source.as_deref(), Some("basilisk"));
        assert!(diag.message.contains("pytest"));
    }

    #[test]
    fn pytest_not_found_code_constant() {
        assert_eq!(PYTEST_NOT_FOUND_CODE, "BSK-W0014");
    }

    #[test]
    fn coverage_result_notification_method() {
        assert_eq!(
            <CoverageResultNotification as tower_lsp::lsp_types::notification::Notification>::METHOD,
            "basilisk/coverageResult"
        );
    }

    #[test]
    fn test_discovery_notification_method() {
        assert_eq!(
            <TestDiscoveryNotification as tower_lsp::lsp_types::notification::Notification>::METHOD,
            "basilisk/testDiscoveryResult"
        );
    }

    #[test]
    fn coverage_result_serializes() {
        let result = crate::coverage::CoverageResult {
            files: vec![crate::coverage::FileCoverage {
                file: "test.py".to_owned(),
                lines: vec![
                    crate::coverage::LineCoverage { line: 1, hits: 5 },
                    crate::coverage::LineCoverage { line: 2, hits: 0 },
                ],
                coverage_pct: 50.0,
            }],
            total_pct: 50.0,
        };
        let value = serde_json::to_value(&result).expect("should serialize");
        assert_eq!(value["totalPct"], 50.0);
        assert_eq!(value["files"][0]["file"], "test.py");
        assert_eq!(value["files"][0]["lines"][0]["line"], 1);
        assert_eq!(value["files"][0]["lines"][0]["hits"], 5);
    }

    #[test]
    fn coverage_command_constant() {
        assert_eq!(
            basilisk_common::commands::RUN_TESTS_COVERAGE,
            "basilisk.runTestsCoverage"
        );
    }

    #[test]
    fn coverage_notification_constant() {
        assert_eq!(
            basilisk_common::coverage_notifications::COVERAGE_RESULT,
            "basilisk/coverageResult"
        );
    }

    #[test]
    fn coverage_config_key_constant() {
        assert_eq!(
            basilisk_common::config_keys::TEST_EXPLORER_COVERAGE_ENABLED,
            "coverageEnabled"
        );
    }
}
