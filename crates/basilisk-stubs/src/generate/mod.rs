//! Implements [STUBRES-ENGINE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ENGINE
//! Auto-stub generation for untyped Python packages.
//!
//! Generates best-effort `.pyi` stub files from installed packages.
//! Generated stubs are tagged as [`StubTier::Tier3`] — downstream
//! diagnostics produce warnings, not false confidence.
//!
//! Three generation modes:
//! - **Runtime** — `inspect.signature()` via Python subprocess (highest accuracy)
//! - **AST** — parse `.py` source with `ruff_python_parser` (no subprocess)
//! - **Hybrid** — try runtime first, fall back to AST per-function

pub mod ast;
pub mod cache;
pub mod hybrid;
pub mod runtime;

use std::path::Path;

/// Generation mode for auto-stub creation.
// Implements [STUBRES-AUTOGEN-MODES] — the three documented modes (runtime
// introspection, AST-based inference, hybrid).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StubGenMode {
    /// Use `inspect.signature()` via Python subprocess.
    Runtime,
    /// Parse `.py` source files with `ruff_python_parser`.
    Ast,
    /// Try runtime first, fall back to AST per-function.
    Hybrid,
}

/// Error type for stub generation failures.
#[derive(Debug, thiserror::Error)]
pub enum StubGenError {
    /// I/O error reading source files.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Python subprocess failed or timed out.
    #[error("Python subprocess error: {0}")]
    Subprocess(String),
    /// Could not parse the Python source file.
    #[error("Parse error: {0}")]
    Parse(String),
    /// Module could not be imported at runtime.
    #[error("Import error: {0}")]
    Import(String),
}

/// Result of generating stubs for a single module.
#[derive(Debug)]
pub struct GeneratedStub {
    /// The dotted module name (e.g. `"requests"`).
    pub module_name: String,
    /// The generated `.pyi` content.
    pub pyi_content: String,
    /// Which mode was used to generate this stub.
    pub mode: StubGenMode,
}

/// Generate stubs for a module using the specified mode.
///
/// # Errors
///
/// Returns `StubGenError` if generation fails for any reason.
// Implements [STUBRES-AUTOGEN] — dispatches `basilisk stubs generate` to the
// chosen [STUBRES-AUTOGEN-MODES] backend; output is tagged Tier 3 so the
// provenance system reports warnings, never false confidence.
pub fn generate_stubs(
    module_name: &str,
    source_path: &Path,
    python_path: &Path,
    mode: StubGenMode,
) -> Result<GeneratedStub, StubGenError> {
    match mode {
        StubGenMode::Runtime => runtime::generate_runtime_stubs(module_name, python_path),
        StubGenMode::Ast => ast::generate_ast_stubs(module_name, source_path),
        StubGenMode::Hybrid => hybrid::generate_hybrid_stubs(module_name, source_path, python_path),
    }
}
