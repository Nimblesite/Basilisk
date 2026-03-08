//! Test Explorer provider for Basilisk LSP.
//!
//! Implements the LSP Testing API to discover and run pytest/unittest tests.
//! Uses the `test_discovery` module to scan for tests and run them via subprocess.

use std::path::PathBuf;
use std::sync::Arc;

use tower_lsp::Client;

use crate::test_discovery;

/// Parameters for running tests.
#[derive(Debug, Clone)]
pub struct TestRunParams {
    /// Unique ID for this run.
    pub id: String,
    /// Test items to run.
    pub tests: Vec<TestRunItem>,
}

/// A single test to run.
#[derive(Debug, Clone)]
pub struct TestRunItem {
    /// Qualified ID of the test.
    pub id: String,
}

/// Result of a test run.
#[derive(Debug, Clone)]
pub struct TestRunResult {
    /// Run ID.
    pub id: String,
    /// Whether all tests passed.
    pub passed: bool,
    /// Combined output.
    pub output: Option<String>,
    /// Process exit code.
    pub exit_code: Option<i32>,
}

/// Parameters for a test run request.
#[derive(Debug, Clone)]
pub struct TestRunRequestParams {
    /// The run parameters.
    pub run: TestRunParams,
}

/// Test provider state.
pub struct TestProvider {
    /// LSP client handle.
    client: Client,
    /// Workspace root directories.
    workspace_roots: Arc<Vec<PathBuf>>,
}

impl TestProvider {
    /// Create a new test provider.
    #[must_use]
    pub fn new(client: Client, workspace_roots: Arc<Vec<PathBuf>>) -> Self {
        Self {
            client,
            workspace_roots,
        }
    }

    /// Get the LSP client reference.
    #[must_use]
    pub const fn client(&self) -> &Client {
        &self.client
    }

    /// Discover test items for all workspace folders.
    #[must_use]
    pub fn discover_tests(&self) -> Vec<test_discovery::TestItem> {
        let mut all_items = Vec::new();

        for root in self.workspace_roots.iter() {
            let discovered = test_discovery::discover_workspace_tests(root);
            all_items.extend(discovered);
        }

        all_items
    }

    /// Run tests identified by test IDs.
    #[must_use]
    pub fn run_tests(&self, params: TestRunParams) -> TestRunResult {
        let test_ids: Vec<String> = params.tests.iter().map(|t| t.id.clone()).collect();

        // Use the first workspace root as the working directory
        let root = self
            .workspace_roots
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));

        match test_discovery::run_tests(&root, &test_ids, "pytest", &[]) {
            Ok(run_result) => TestRunResult {
                id: params.id,
                passed: run_result.passed,
                output: Some(run_result.stdout + &run_result.stderr),
                exit_code: Some(run_result.exit_code),
            },
            Err(err) => TestRunResult {
                id: params.id,
                passed: false,
                output: Some(err),
                exit_code: Some(-1),
            },
        }
    }
}

/// Handle test run request.
#[must_use]
pub fn handle_test_run_request(
    client: &Client,
    params: TestRunRequestParams,
    workspace_roots: Arc<Vec<PathBuf>>,
) -> TestRunResult {
    let provider = TestProvider::new(client.clone(), workspace_roots);
    provider.run_tests(params.run)
}
