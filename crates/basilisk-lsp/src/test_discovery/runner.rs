//! Test execution via pytest subprocess and result parsing.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::TestStatus;

/// Configuration for test execution.
#[derive(Debug, Clone)]
pub struct TestRunConfig<'a> {
    /// Workspace root directory.
    pub root: &'a Path,
    /// Test node IDs to run (empty = run all).
    pub test_ids: &'a [String],
    /// Path to the pytest executable.
    pub pytest_path: &'a str,
    /// Additional test runner arguments.
    pub extra_args: &'a [String],
    /// Whether to use `uv run` when a uv project is detected.
    pub use_uv_run: bool,
    /// When true, append `--cov` and `--cov-report=xml` args for coverage.
    pub enable_coverage: bool,
}

/// Result of running tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunResult {
    /// stdout from pytest.
    pub stdout: String,
    /// stderr from pytest.
    pub stderr: String,
    /// Process exit code.
    pub exit_code: i32,
    /// Whether all tests passed.
    pub passed: bool,
    /// Per-test results parsed from pytest output.
    pub per_test: Vec<PerTestResult>,
}

/// Result for a single test extracted from pytest output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerTestResult {
    /// Test node ID (e.g. `tests/test_api.py::test_login`).
    pub test_id: String,
    /// Pass/fail/skip/error status.
    pub status: TestStatus,
    /// Failure message or traceback (empty for passed tests).
    pub message: String,
}

/// Run tests via pytest subprocess and return results.
///
/// When `use_uv_run` is true and a uv project is detected (via
/// `basilisk_uv::detect_uv_project`), tests are run using `uv run pytest`
/// to ensure the correct environment. Otherwise, falls back to running
/// `pytest_path` directly with `VIRTUAL_ENV` and `PATH` set if a venv exists.
///
/// # Errors
///
/// Returns error string if pytest cannot be found or fails to execute.
pub fn run_tests(config: &TestRunConfig<'_>) -> Result<TestRunResult, String> {
    let is_uv =
        config.use_uv_run && basilisk_uv::detect_uv_project(&[config.root.to_path_buf()]).is_some();

    let mut cmd = if is_uv {
        let mut c = std::process::Command::new("uv");
        let _ = c.args(["run", config.pytest_path]);
        c
    } else {
        let mut c = std::process::Command::new(config.pytest_path);
        // Set VIRTUAL_ENV and prepend venv bin to PATH when not using uv run.
        set_venv_env(config.root, &mut c);
        c
    };

    let _ = cmd.current_dir(config.root);
    let _ = cmd.args(["--tb=short", "-q"]);

    // Append coverage args when coverage is enabled.
    if config.enable_coverage {
        let _ = cmd.arg("--cov");
        let cov_xml = config.root.join(".basilisk").join("coverage.xml");
        let _ = cmd.arg(format!("--cov-report=xml:{}", cov_xml.display()));
    }

    for arg in config.extra_args {
        let _ = cmd.arg(arg);
    }

    for test_id in config.test_ids {
        let _ = cmd.arg(test_id);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run pytest: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit_code = output.status.code().unwrap_or(-1);
    let per_test = parse_pytest_output(&stdout, &stderr);

    Ok(TestRunResult {
        stdout,
        stderr,
        exit_code,
        passed: exit_code == 0,
        per_test,
    })
}

/// Set `VIRTUAL_ENV` and prepend venv `bin/` to `PATH` on the command.
///
/// Looks for `.venv` or `venv` directories in the workspace root.
pub(super) fn set_venv_env(root: &Path, cmd: &mut std::process::Command) {
    for venv_dir in &[".venv", "venv"] {
        let venv_path = root.join(venv_dir);
        let bin_dir = if cfg!(windows) {
            venv_path.join("Scripts")
        } else {
            venv_path.join("bin")
        };
        if bin_dir.is_dir() {
            let _ = cmd.env("VIRTUAL_ENV", &venv_path);
            // Prepend venv bin to PATH.
            if let Ok(current_path) = std::env::var("PATH") {
                let new_path = format!(
                    "{}{}{}",
                    bin_dir.display(),
                    std::path::MAIN_SEPARATOR,
                    current_path
                );
                let _ = cmd.env("PATH", new_path);
            }
            break;
        }
    }
}

/// Parse pytest `-q --tb=short` output into per-test results.
///
/// Handles lines like:
/// - `tests/test_api.py::test_login PASSED`
/// - `tests/test_api.py::test_signup FAILED`
/// - `tests/test_api.py::test_slow SKIPPED`
/// - `tests/test_api.py::test_bad ERROR`
#[must_use]
pub fn parse_pytest_output(stdout: &str, stderr: &str) -> Vec<PerTestResult> {
    let mut results = Vec::new();
    let mut current_failure_id = String::new();
    let mut failure_lines: Vec<String> = Vec::new();
    let mut in_failure_block = false;

    for line in stdout.lines() {
        let trimmed = line.trim();

        // Match result lines: "test_id STATUS"
        if let Some((test_id, status)) = parse_result_line(trimmed) {
            results.push(PerTestResult {
                test_id,
                status,
                message: String::new(),
            });
            continue;
        }

        // Detect failure/error blocks: "FAILED test_id" or "= FAILURES ="
        if trimmed.starts_with("FAILED ") || trimmed.starts_with("ERROR ") {
            // Flush previous failure block.
            flush_failure(&mut results, &current_failure_id, &failure_lines);
            trimmed
                .strip_prefix("FAILED ")
                .or_else(|| trimmed.strip_prefix("ERROR "))
                .unwrap_or("")
                .trim_start_matches("- ")
                .clone_into(&mut current_failure_id);
            failure_lines.clear();
            in_failure_block = true;
            continue;
        }

        if trimmed.starts_with("_____") || trimmed.starts_with("=====") {
            if in_failure_block {
                flush_failure(&mut results, &current_failure_id, &failure_lines);
                current_failure_id.clear();
                failure_lines.clear();
                in_failure_block = false;
            }
            continue;
        }

        if in_failure_block {
            failure_lines.push(line.to_owned());
        }
    }

    // Flush any remaining failure block.
    flush_failure(&mut results, &current_failure_id, &failure_lines);

    // Also check stderr for failures not in stdout.
    if !stderr.is_empty() && results.is_empty() {
        // If no results parsed from stdout, check for import errors etc.
        for line in stderr.lines() {
            if line.contains("ERROR") || line.contains("ModuleNotFoundError") {
                results.push(PerTestResult {
                    test_id: String::new(),
                    status: TestStatus::Error,
                    message: line.to_owned(),
                });
            }
        }
    }

    results
}

/// Parse a single pytest result line like `tests/test_api.py::test_login PASSED`.
fn parse_result_line(line: &str) -> Option<(String, TestStatus)> {
    let (test_id_str, status_str) = line.rsplit_once(' ')?;
    let test_id = test_id_str.to_owned();

    if !test_id.contains("::") {
        return None;
    }

    let status = match status_str {
        "PASSED" => TestStatus::Passed,
        "FAILED" => TestStatus::Failed,
        "SKIPPED" | "XFAIL" => TestStatus::Skipped,
        "ERROR" => TestStatus::Error,
        _ => return None,
    };

    Some((test_id, status))
}

/// Flush a failure block: find the matching result and attach the message.
fn flush_failure(results: &mut [PerTestResult], test_id: &str, lines: &[String]) {
    if test_id.is_empty() {
        return;
    }
    let message = lines.join("\n").trim().to_owned();
    if let Some(result) = results.iter_mut().find(|r| r.test_id == test_id) {
        result.message = message;
    }
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
    fn test_run_result_serialization() {
        let result = TestRunResult {
            stdout: "1 passed".to_owned(),
            stderr: String::new(),
            exit_code: 0,
            passed: true,
            per_test: vec![PerTestResult {
                test_id: "test_foo.py::test_bar".to_owned(),
                status: TestStatus::Passed,
                message: String::new(),
            }],
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("\"exitCode\":0"));
        assert!(json.contains("\"passed\":true"));
        assert!(json.contains("\"perTest\""));
    }

    #[test]
    fn test_set_venv_env_no_venv() {
        let dir = std::env::temp_dir().join("basilisk_no_venv");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");

        let mut cmd = std::process::Command::new("echo");
        set_venv_env(&dir, &mut cmd);
        // No assertion needed — just verify it doesn't panic.

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_pytest_output_passed() {
        let stdout = "tests/test_api.py::test_login PASSED\ntests/test_api.py::test_signup PASSED\n1 passed\n";
        let results = parse_pytest_output(stdout, "");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].test_id, "tests/test_api.py::test_login");
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(results[1].test_id, "tests/test_api.py::test_signup");
        assert_eq!(results[1].status, TestStatus::Passed);
    }

    #[test]
    fn test_parse_pytest_output_failed() {
        let stdout =
            "tests/test_api.py::test_login PASSED\ntests/test_api.py::test_signup FAILED\n";
        let results = parse_pytest_output(stdout, "");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(results[1].status, TestStatus::Failed);
    }

    #[test]
    fn test_parse_pytest_output_skipped() {
        let stdout = "tests/test_slow.py::test_heavy SKIPPED\n";
        let results = parse_pytest_output(stdout, "");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::Skipped);
    }

    #[test]
    fn test_parse_pytest_output_error() {
        let stdout = "tests/test_bad.py::test_broken ERROR\n";
        let results = parse_pytest_output(stdout, "");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::Error);
    }

    #[test]
    fn test_parse_pytest_output_empty() {
        let results = parse_pytest_output("", "");
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_pytest_output_non_test_lines_ignored() {
        let stdout = "============================= test session starts ==============================\ncollected 2 items\n\ntests/test_api.py::test_login PASSED\n\n1 passed in 0.01s\n";
        let results = parse_pytest_output(stdout, "");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].test_id, "tests/test_api.py::test_login");
    }

    #[test]
    fn test_parse_pytest_output_xfail() {
        let stdout = "tests/test_api.py::test_known_issue XFAIL\n";
        let results = parse_pytest_output(stdout, "");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::Skipped);
    }

    #[test]
    fn test_per_test_result_serialization() {
        let result = PerTestResult {
            test_id: "tests/test_foo.py::test_bar".to_owned(),
            status: TestStatus::Failed,
            message: "assert 1 == 2".to_owned(),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("\"testId\""));
        assert!(json.contains("\"failed\""));
    }

    #[test]
    fn test_run_tests_with_nonexistent_pytest() {
        let dir = std::env::temp_dir().join("basilisk_nonexistent_pytest");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");

        let config = TestRunConfig {
            root: &dir,
            test_ids: &[],
            pytest_path: "/nonexistent/pytest",
            extra_args: &[],
            use_uv_run: false,
            enable_coverage: false,
        };
        let result = run_tests(&config);
        assert!(result.is_err(), "should fail with nonexistent pytest");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_tests_coverage_flag_nonexistent_pytest() {
        let dir = std::env::temp_dir().join("basilisk_coverage_flag_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");

        let config = TestRunConfig {
            root: &dir,
            test_ids: &[],
            pytest_path: "/nonexistent/pytest",
            extra_args: &[],
            use_uv_run: false,
            enable_coverage: true,
        };
        // Should still fail (nonexistent pytest), but coverage flag shouldn't panic.
        let result = run_tests(&config);
        assert!(
            result.is_err(),
            "should fail with nonexistent pytest even with coverage enabled"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_config_coverage_flag_defaults() {
        let dir = std::env::temp_dir();
        let config = TestRunConfig {
            root: &dir,
            test_ids: &[],
            pytest_path: "pytest",
            extra_args: &[],
            use_uv_run: false,
            enable_coverage: false,
        };
        assert!(!config.enable_coverage);
    }

    #[test]
    fn test_run_config_coverage_enabled() {
        let dir = std::env::temp_dir();
        let config = TestRunConfig {
            root: &dir,
            test_ids: &[],
            pytest_path: "pytest",
            extra_args: &[],
            use_uv_run: false,
            enable_coverage: true,
        };
        assert!(config.enable_coverage);
    }
}
