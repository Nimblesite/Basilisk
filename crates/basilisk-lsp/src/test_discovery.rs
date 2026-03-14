//! Test Explorer: discover pytest and unittest tests from Python source files.
//!
//! Scans workspace for `test_*.py` and `*_test.py` files, parses them with
//! `basilisk-parser`, and extracts test items (functions, classes, methods)
//! without importing or executing the code.

use std::path::{Path, PathBuf};

/// A discovered test item in the workspace.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
            #[expect(
                clippy::cast_possible_truncation,
                reason = "u32 byte offset to usize is safe on 32/64-bit"
            )]
            let line = func.def_span.start as usize;
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
            #[expect(
                clippy::cast_possible_truncation,
                reason = "u32 byte offset to usize is safe on 32/64-bit"
            )]
            let class_line = class.def_span.start as usize;
            let class_line_num = byte_offset_to_line(source, class_line);

            // Find test methods by matching functions whose class_name == this class.
            let mut methods = Vec::new();
            for func in &resolved.functions {
                let is_method = func.class_name.as_ref().is_some_and(|cn| cn == &class.name);
                if is_method && func.name.starts_with("test") {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "u32 byte offset to usize is safe on 32/64-bit"
                    )]
                    let method_line = func.def_span.start as usize;
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
            #[expect(
                clippy::cast_possible_truncation,
                reason = "no truncation: line is hardcoded 0"
            )]
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

/// Run tests via pytest subprocess and return results.
///
/// # Errors
///
/// Returns error string if pytest cannot be found or fails to execute.
pub fn run_tests(
    root: &Path,
    test_ids: &[String],
    pytest_path: &str,
    extra_args: &[String],
) -> Result<TestRunResult, String> {
    let mut cmd = std::process::Command::new(pytest_path);
    let _ = cmd.current_dir(root);
    let _ = cmd.args(["--tb=short", "-q"]);

    for arg in extra_args {
        let _ = cmd.arg(arg);
    }

    // If specific tests requested, pass them as node IDs.
    for test_id in test_ids {
        let _ = cmd.arg(test_id);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run pytest: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(TestRunResult {
        stdout,
        stderr,
        exit_code,
        passed: exit_code == 0,
    })
}

/// Result of running tests.
#[derive(Debug, Clone)]
pub struct TestRunResult {
    /// stdout from pytest.
    pub stdout: String,
    /// stderr from pytest.
    pub stderr: String,
    /// Process exit code.
    pub exit_code: i32,
    /// Whether all tests passed.
    pub passed: bool,
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
    source[..clamped].chars().filter(|&c| c == '\n').count()
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
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only code: unwrap/expect acceptable in unit tests"
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
        assert_eq!(items[0].name, "TestMyCase");
        assert_eq!(items[0].kind, TestItemKind::Class);
        assert_eq!(items[0].children.len(), 1);
        assert_eq!(items[0].children[0].name, "test_something");
    }
}
