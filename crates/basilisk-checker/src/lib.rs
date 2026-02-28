//! Type checker for Basilisk.
//!
//! The public API is [`check`], which takes a [`ResolvedModule`] and
//! returns a list of [`Diagnostic`]s.

pub mod diagnostic;
pub mod rules;

pub use diagnostic::{Diagnostic, ErrorCode, Severity};
pub use rules::run_all as check;
