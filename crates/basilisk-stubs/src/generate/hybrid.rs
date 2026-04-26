//! Hybrid stub generation.
//!
//! Tries runtime introspection first.  If that fails (e.g. the module has
//! side effects on import, or the Python binary is unavailable), falls back
//! to AST-based generation.

use std::path::Path;

use super::{GeneratedStub, StubGenError, StubGenMode};

/// Generate stubs using hybrid mode: runtime first, AST fallback.
///
/// # Errors
///
/// Returns `StubGenError` only if **both** methods fail.
pub fn generate_hybrid_stubs(
    module_name: &str,
    source_path: &Path,
    python_path: &Path,
) -> Result<GeneratedStub, StubGenError> {
    match super::runtime::generate_runtime_stubs(module_name, python_path) {
        Ok(mut stub) => {
            stub.mode = StubGenMode::Hybrid;
            Ok(stub)
        }
        Err(runtime_err) => {
            tracing::debug!(
                module = module_name,
                error = %runtime_err,
                "runtime introspection failed, falling back to AST"
            );
            let mut stub = super::ast::generate_ast_stubs(module_name, source_path)
                .map_err(|ast_err| {
                    StubGenError::Subprocess(format!(
                        "both runtime and AST generation failed: runtime={runtime_err}, ast={ast_err}"
                    ))
                })?;
            stub.mode = StubGenMode::Hybrid;
            Ok(stub)
        }
    }
}
