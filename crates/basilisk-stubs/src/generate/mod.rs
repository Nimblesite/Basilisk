//! Implements [STUBRES-ENGINE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ENGINE
//! Auto-stub generation for untyped Python packages.
//!
//! Generates best-effort `.pyi` stub files from installed packages.
//! Generated stubs are tagged as [`StubTier::Tier3`] — downstream
//! diagnostics produce warnings, not false confidence.
//!
//! Three generation modes:
//! - **Runtime** — `inspect.signature()` via Python subprocess (highest accuracy)
//! - **AST** — parse `.py` source with `basilisk-parser` (no subprocess)
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
    /// Parse `.py` source files with `basilisk-parser`.
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

impl GeneratedStub {
    /// Whether this stub declares any usable type information — at least one
    /// function, class, variable, or overload.
    ///
    /// A stub with none carries nothing a checker can use. Caching it would
    /// report a false success and let the empty `.pyi` satisfy BSK-0152 as
    /// though the module were typed (GitHub #336), so the CLI treats an
    /// declaration-free result as "nothing generated" rather than a win. A stub
    /// whose content fails to parse is conservatively reported as having
    /// declarations so a genuine (if unparseable) result is never silently
    /// dropped.
    #[must_use]
    pub fn has_declarations(&self) -> bool {
        crate::pyi_parser::parse_pyi_source(
            &self.pyi_content,
            Path::new("<generated>"),
            &self.module_name,
            crate::types::StubSource::UserStub,
            crate::types::StubTier::Tier3,
        )
        .map_or(true, |module| {
            !module.functions.is_empty()
                || !module.classes.is_empty()
                || !module.variables.is_empty()
                || !module.overloads.is_empty()
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn stub(pyi_content: &str) -> GeneratedStub {
        GeneratedStub {
            module_name: "m".to_owned(),
            pyi_content: pyi_content.to_owned(),
            mode: StubGenMode::Runtime,
        }
    }

    /// GitHub #336: a stub that declares nothing must be recognised as empty so
    /// the CLI never caches it or reports a false success.
    #[test]
    fn has_declarations_reports_a_header_only_stub_as_empty() {
        let empty = stub(
            "# Auto-generated stub for `m` (runtime introspection)\n\nfrom typing import Any\n",
        );
        assert!(
            !empty.has_declarations(),
            "a header-only stub declares no usable type information"
        );
    }

    #[test]
    fn has_declarations_reports_a_populated_stub_as_non_empty() {
        assert!(stub("def f() -> int: ...\n").has_declarations(), "function");
        assert!(stub("class C: ...\n").has_declarations(), "class");
        assert!(stub("VERSION: str\n").has_declarations(), "variable");
    }
}
