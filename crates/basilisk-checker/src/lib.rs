//! Type checker for Basilisk.
//!
//! The public API is [`check`], which takes a [`ResolvedModule`] and
//! returns a list of [`Diagnostic`]s.
//!
//! ## Suppression and Mode Override
//!
//! Basilisk supports a rich set of inline directives for controlling diagnostic
//! severity. See SPEC.md Section 4.1.3 for the full specification.
//!
//! - `# type: ignore` — suppress all diagnostics (PEP 484 compatible)
//! - `# type: ignore[BSK-E0010]` — suppress specific codes
//! - `# type: warning[BSK-E0010]` — demote to warning
//! - `# type: info[BSK-E0010]` — demote to info
//! - `# type: disabled[BSK-E0010]` — disable rule on this line
//! - `# type: disabled[BSK-E0010]` ... `# type: end-disabled[BSK-E0010]` — block
//! - `# basilisk: relaxed` — per-file: all errors become warnings
//! - `# basilisk: file-disabled[CODE]` — per-file: disable specific rules

pub mod collection_inference;
pub mod diagnostic;
pub mod inference;
pub mod rules;
pub mod span_util;
pub mod suppression;
pub mod types;

pub use diagnostic::{Diagnostic, ErrorCode, Severity};

/// Run all rules and apply inline suppression / mode overrides.
#[must_use]
pub fn check(module: &basilisk_resolver::ResolvedModule) -> Vec<Diagnostic> {
    let overrides = suppression::parse_source_overrides(&module.source);
    let source = &module.source;
    let raw = rules::run_all(module);
    raw.into_iter()
        .filter_map(|d| {
            let diag_line = suppression::byte_offset_to_line_in_source(source, d.span.start);
            suppression::apply_overrides_at_line(d, diag_line, &overrides)
        })
        .collect()
}
