// Shared test infrastructure for stdio-based LSP E2E tests.
//
// Each test file imports this module via `mod lsp_e2e_common;` to get
// the fixture, type alias, and helper functions.
//
// The actual fixture implementation lives in `basilisk_test_utils::lsp_stdio`.

pub use basilisk_test_utils::TestResult;

/// Type alias preserving the original name used by LSP E2E tests.
pub type LspTestFixture = basilisk_test_utils::LspStdioFixture;

/// Send an LSP request and wait for the response matching the given id.
///
/// Thin wrapper around [`LspStdioFixture::send_request`] preserving
/// the standalone-function call style used by existing tests.
///
/// # Errors
/// Returns an error if writing the request or reading the response fails.
pub fn send_request(
    fixture: &mut LspTestFixture,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> TestResult<Option<String>> {
    fixture.send_request(id, method, params)
}

/// Helper: send a `textDocument/completion` request and wait for the response.
///
/// # Errors
/// Returns an error if writing the request or reading the response fails.
pub fn request_completion(
    fixture: &mut LspTestFixture,
    uri: &str,
    line: u32,
    character: u32,
    request_id: u64,
) -> TestResult<Option<String>> {
    fixture.request_completion(uri, line, character, request_id)
}
