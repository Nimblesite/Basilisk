//! Test Explorer: discover pytest and unittest tests from Python source files.
//!
//! Scans workspace for `test_*.py` and `*_test.py` files, parses them with
//! `basilisk-parser`, and extracts test items (functions, classes, methods)
//! without importing or executing the code.

mod discovery;
mod runner;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use self::discovery::{discover_test_files, discover_tests_in_file, discover_workspace_tests};
pub use self::runner::{
    parse_pytest_output, run_tests, PerTestResult, TestRunConfig, TestRunResult,
};

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
