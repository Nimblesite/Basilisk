//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Python source parser for Basilisk.
//!
//! This crate is the single entry point into `ruff_python_parser` for the
//! entire Basilisk workspace. All other crates receive a [`ParsedModule`]
//! and work exclusively with `ruff_python_ast` types.

pub mod error;

pub use error::ParseError;
pub use ruff_python_ast::ModModule;

/// The result of parsing a single Python source file.
#[derive(Debug)]
pub struct ParsedModule {
    /// The parsed module AST node.
    pub ast: ModModule,
    /// The original source text (needed for span-to-line-col mapping).
    pub source: String,
    /// The file path from which the source was loaded (for diagnostics).
    pub path: String,
}

/// Parse a Python source string into a [`ParsedModule`].
///
/// # Errors
///
/// Returns [`ParseError::Syntax`] if the source contains syntax errors.
pub fn parse_source(source: String, path: String) -> Result<ParsedModule, ParseError> {
    ruff_python_parser::parse_module(&source)
        .map(|parsed| ParsedModule {
            ast: parsed.into_syntax(),
            source,
            path: path.clone(),
        })
        .map_err(|err| ParseError::Syntax {
            path,
            message: err.to_string(),
        })
}

/// Read a file from disk and parse it.
///
/// # Errors
///
/// Returns [`ParseError::Io`] on read failure or [`ParseError::Syntax`] on
/// parse failure.
pub fn parse_file(path: &str) -> Result<ParsedModule, ParseError> {
    // Routed through `read_tracked` so the result cache can record exactly which
    // files a check read. Inert (identical to `fs::read_to_string`) when no
    // recorder is active. See [CHKCACHE-READSET-FS].
    basilisk_common::fs::read_tracked(std::path::Path::new(path))
        .map_err(|source| ParseError::Io {
            path: path.to_owned(),
            source,
        })
        .and_then(|content| parse_source(content, path.to_owned()))
}
