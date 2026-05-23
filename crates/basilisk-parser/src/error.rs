//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Parser error types.

use thiserror::Error;

/// Errors produced during parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The source could not be parsed as a Python module.
    #[error("syntax error in {path}: {message}")]
    Syntax {
        /// The file path where the error occurred.
        path: String,
        /// Message from the underlying parser.
        message: String,
    },

    /// An I/O error occurred while reading the source file.
    #[error("I/O error reading {path}: {source}")]
    Io {
        /// The path being read.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}
