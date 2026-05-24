//! Implements [COMPARCH]. See docs/specs/COMPILER-ARCHITECTURE-SPEC.md#COMPARCH
//! Compiler error types.

/// Errors that can occur during compilation.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// Source failed to parse.
    #[error("parse error: {0}")]
    Parse(String),
    /// Name resolution failed.
    #[error("resolve error: {0}")]
    Resolve(String),
    /// Code generation failed.
    #[error("codegen error: {0}")]
    Codegen(String),
}
