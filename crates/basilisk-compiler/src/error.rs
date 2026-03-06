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
    /// Execution failed.
    #[error("execution error: {0}")]
    Execution(String),
}
