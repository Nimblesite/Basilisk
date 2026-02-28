//! Name resolution and scope analysis for Basilisk.
//!
//! The resolver walks the parsed AST and produces a [`ResolvedModule`]
//! containing structured information about every function definition.
//! The checker operates on [`ResolvedModule`] without touching the raw AST.

pub mod scope;
mod visitor;

pub use scope::{FunctionInfo, ParameterInfo, ResolvedModule, Span};

use basilisk_parser::ParsedModule;

/// Errors produced during resolution.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// A resolution invariant was violated (reserved for future phases).
    #[error("internal resolve error: {0}")]
    Internal(String),
}

/// Resolve all function definitions in a parsed module.
///
/// Returns a [`ResolvedModule`] describing every function and its
/// annotation completeness.
///
/// # Errors
///
/// Currently infallible in Phase 1; future phases may add import resolution
/// errors.
pub fn resolve(module: &ParsedModule) -> Result<ResolvedModule, ResolveError> {
    visitor::collect_functions(module)
}
