//! Test Explorer: discover pytest and unittest tests from Python source files.
//!
//! Scans workspace for `test_*.py` and `*_test.py` files, parses them with
//! `basilisk-parser`, and extracts test items (functions, classes, methods)
//! without importing or executing the code.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A discovered test item in the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestItem {
    /// Display name (e.g. `test_login`, `TestUserEndpoints::test_get_user`).
    pub name: String,
    /// Full qualified ID for running (e.g. `tests/test_api.py::test_login`).
    pub id: String,
    /// File path where this test is defined.
    pub file: PathBuf,
    /// 0-based line number of the test definition.
    pub line: usize,
    /// Kind of test item.
    pub kind: TestItemKind,
    /// Children (methods inside a test class).
    pub children: Vec<TestItem>,
}

/// The kind of test item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestItemKind {
    /// A test file (e.g. `test_api.py`).
    File,
    /// A test function (def test_*).
    Function,
    /// A test class (class Test*).
    Class,
    /// A test method inside a class.
    Method,
}

/// Discover all test files in a directory tree.
#[must_use]
pub fn discover_test_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_test_files(root, &mut files);
    files
}

/// Discover test items from a single Python source file.
#[must_use]
pub fn discover_tests_in_file(path: &Path, source: &str) -> Vec<TestItem> {
    let path_str = path.to_string_lossy().into_owned();
    let Ok(parsed) = basilisk_parser::parse_source(source.to_owned(), path_str) else {
        return Vec::new();
    };

    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return Vec::new();
    };

    let relative = path.to_string_lossy();
    let mut items = Vec::new();

    // Find test functions (def test_*) — skip methods (they belong to classes).
    for func in &resolved.functions {
        if func.name.starts_with("test_") && func.class_name.is_none() {
            let line = func.def_span.start_usize();
            let line_num = byte_offset_to_line(source, line);
            items.push(TestItem {
                name: func.name.clone(),
                id: format!("{relative}::{}", func.name),
                file: path.to_path_buf(),
                line: line_num,
                kind: TestItemKind::Function,
                children: Vec::new(),
            });
        }
    }

    // Find test classes (class Test*) and their test methods.
    for class in &resolved.classes {
        if class.name.starts_with("Test") || is_unittest_class(class) {
            let class_line = class.def_span.start_usize();
            let class_line_num = byte_offset_to_line(source, class_line);

            // Find test methods by matching functions whose class_name == this class.
            let mut methods = Vec::new();
            for func in &resolved.functions {
                let is_method = func.class_name.as_ref().is_some_and(|cn| cn == &class.name);
                if is_method && func.name.starts_with("test") {
                    let method_line = func.def_span.start_usize();
                    let method_line_num = byte_offset_to_line(source, method_line);
                    methods.push(TestItem {
                        name: func.name.clone(),
                        id: format!("{relative}::{}::{}", class.name, func.name),
                        file: path.to_path_buf(),
                        line: method_line_num,
                        kind: TestItemKind::Method,
                        children: Vec::new(),
                    });
                }
            }

            items.push(TestItem {
                name: class.name.clone(),
                id: format!("{relative}::{}", class.name),
                file: path.to_path_buf(),
                line: class_line_num,
                kind: TestItemKind::Class,
                children: methods,
            });
        }
    }

    items
}

/// Discover all tests across a workspace.
#[must_use]
pub fn discover_workspace_tests(root: &Path) -> Vec<TestItem> {
    let test_files = discover_test_files(root);
    let mut all_items = Vec::new();

    for path in &test_files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let file_tests = discover_tests_in_file(path, &source);
        if !file_tests.is_empty() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            all_items.push(TestItem {
                name: relative.clone(),
                id: relative,
                file: path.clone(),
                line: 0,
                kind: TestItemKind::File,
                children: file_tests,
            });
        }
    }

    all_items
}

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
fn set_venv_env(root: &Path, cmd: &mut std::process::Command) {
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

/// Status of an individual test after execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestStatus {
    /// Test passed.
    Passed,
    /// Test failed.
    Failed,
    /// Test was skipped.
    Skipped,
    /// Test errored (e.g. setup failure).
    Error,
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
    // Pytest -q outputs: "test_id PASSED/FAILED/SKIPPED/ERROR"
    // Split from the right: "test_id STATUS" → ["STATUS", "test_id"]
    let (test_id_str, status_str) = line.rsplit_once(' ')?;
    let test_id = test_id_str.to_owned();

    // Validate it looks like a test ID (contains ::).
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

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Check if a class inherits from `unittest.TestCase`.
fn is_unittest_class(class: &basilisk_resolver::scope::ClassInfo) -> bool {
    class
        .bases
        .iter()
        .any(|base| base == "TestCase" || base == "unittest.TestCase")
}

/// Convert a byte offset to a 0-based line number.
fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    source
        .get(..clamped)
        .map_or(0, |s| s.chars().filter(|&c| c == '\n').count())
}

/// Recursively collect test files (test_*.py, *_test.py).
fn collect_test_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.')
                || name_str == "__pycache__"
                || name_str == "node_modules"
                || name_str == "venv"
            {
                continue;
            }
            collect_test_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "py") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if stem.starts_with("test_") || stem.ends_with("_test") {
                    out.push(path);
                }
            }
        }
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
    fn test_discover_tests_in_source() {
        let source = r"
import pytest

def test_login() -> None:
    assert True

def test_signup() -> None:
    assert True

def helper() -> None:
    pass

class TestUserEndpoints:
    def test_get_user(self) -> None:
        assert True

    def test_delete_user(self) -> None:
        assert True

    def not_a_test(self) -> None:
        pass
";
        let path = Path::new("tests/test_api.py");
        let items = discover_tests_in_file(path, source);

        // Should find 2 test functions + 1 test class
        assert_eq!(items.len(), 3, "items: {items:?}");

        let func_names: Vec<&str> = items
            .iter()
            .filter(|i| i.kind == TestItemKind::Function)
            .map(|i| i.name.as_str())
            .collect();
        assert!(func_names.contains(&"test_login"));
        assert!(func_names.contains(&"test_signup"));

        let class = items
            .iter()
            .find(|i| i.kind == TestItemKind::Class)
            .expect("should find TestUserEndpoints class");
        assert_eq!(class.name, "TestUserEndpoints");
        assert_eq!(class.children.len(), 2);
    }

    #[test]
    fn test_unittest_class_detection() {
        let source = r"
import unittest

class TestMyCase(unittest.TestCase):
    def test_something(self) -> None:
        self.assertTrue(True)
";
        let path = Path::new("tests/test_unit.py");
        let items = discover_tests_in_file(path, source);

        assert_eq!(items.len(), 1);
        let item = items.first().expect("expected at least one test item");
        assert_eq!(item.name, "TestMyCase");
        assert_eq!(item.kind, TestItemKind::Class);
        assert_eq!(item.children.len(), 1);
        assert_eq!(
            item.children
                .first()
                .expect("expected at least one child")
                .name,
            "test_something"
        );
    }

    #[test]
    fn test_empty_source_produces_no_items() {
        let items = discover_tests_in_file(Path::new("test_empty.py"), "");
        assert!(items.is_empty());
    }

    #[test]
    fn test_no_tests_in_source() {
        let source = r"
def helper() -> None:
    pass

class NotATestClass:
    def do_something(self) -> None:
        pass
";
        let items = discover_tests_in_file(Path::new("test_nothing.py"), source);
        assert!(items.is_empty(), "should find no tests: {items:?}");
    }

    #[test]
    fn test_mixed_test_and_non_test_functions() {
        let source = r"
def test_alpha() -> None:
    pass

def not_a_test() -> None:
    pass

def test_beta() -> None:
    pass

def setup() -> None:
    pass
";
        let items = discover_tests_in_file(Path::new("test_mixed.py"), source);
        assert_eq!(items.len(), 2, "should find exactly 2 test functions");
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"test_alpha"));
        assert!(names.contains(&"test_beta"));
    }

    #[test]
    fn test_item_ids_use_file_relative_path() {
        let source = r"
def test_example() -> None:
    pass
";
        let path = Path::new("tests/test_example.py");
        let items = discover_tests_in_file(path, source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "tests/test_example.py::test_example");
    }

    #[test]
    fn test_class_method_ids_include_class_name() {
        let source = r"
class TestFoo:
    def test_bar(self) -> None:
        pass
";
        let path = Path::new("test_cls.py");
        let items = discover_tests_in_file(path, source);
        assert_eq!(items.len(), 1);
        let class_item = &items[0];
        assert_eq!(class_item.children.len(), 1);
        assert_eq!(class_item.children[0].id, "test_cls.py::TestFoo::test_bar");
    }

    #[test]
    fn test_testcase_subclass_detected() {
        let source = r"
from unittest import TestCase

class MyTests(TestCase):
    def test_one(self) -> None:
        pass
";
        let items = discover_tests_in_file(Path::new("test_tc.py"), source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, TestItemKind::Class);
        assert_eq!(items[0].name, "MyTests");
    }

    #[test]
    fn test_discover_test_files_skips_hidden_dirs() {
        let dir = std::env::temp_dir().join("basilisk_test_files_hidden");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".hidden")).expect("create .hidden");
        std::fs::write(dir.join(".hidden/test_secret.py"), "").expect("write");
        std::fs::write(dir.join("test_visible.py"), "").expect("write");

        let files = discover_test_files(&dir);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("test_visible.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_test_files_skips_pycache() {
        let dir = std::env::temp_dir().join("basilisk_test_files_pycache");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("__pycache__")).expect("create __pycache__");
        std::fs::write(dir.join("__pycache__/test_cached.py"), "").expect("write");
        std::fs::write(dir.join("test_real.py"), "").expect("write");

        let files = discover_test_files(&dir);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("test_real.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_test_files_finds_both_patterns() {
        let unique = format!(
            "basilisk_test_patterns_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("create dir");
        std::fs::write(dir.join("test_alpha.py"), "").expect("write");
        std::fs::write(dir.join("beta_test.py"), "").expect("write");
        std::fs::write(dir.join("helper.py"), "").expect("write");

        let files = discover_test_files(&dir);
        assert_eq!(
            files.len(),
            2,
            "should find test_alpha.py and beta_test.py, got: {files:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_workspace_discovery_creates_file_items() {
        let dir = std::env::temp_dir().join("basilisk_workspace_disc");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        std::fs::write(
            dir.join("test_ws.py"),
            "def test_workspace() -> None:\n    pass\n",
        )
        .expect("write");

        let items = discover_workspace_tests(&dir);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, TestItemKind::File);
        assert_eq!(items[0].children.len(), 1);
        assert_eq!(items[0].children[0].name, "test_workspace");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_serialization_round_trip() {
        let item = TestItem {
            name: "test_foo".to_owned(),
            id: "tests/test_foo.py::test_foo".to_owned(),
            file: PathBuf::from("tests/test_foo.py"),
            line: 5,
            kind: TestItemKind::Function,
            children: Vec::new(),
        };
        let json = serde_json::to_string(&item).expect("serialize");
        let back: TestItem = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.name, "test_foo");
        assert_eq!(back.kind, TestItemKind::Function);
    }

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
    fn test_kind_serializes_as_camel_case() {
        let json = serde_json::to_string(&TestItemKind::Function).expect("serialize");
        assert_eq!(json, "\"function\"");
        let json = serde_json::to_string(&TestItemKind::Class).expect("serialize");
        assert_eq!(json, "\"class\"");
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
