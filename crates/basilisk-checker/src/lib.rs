//! Type checker for Basilisk.
//!
//! The public API is [`check`], which takes a [`ResolvedModule`] and
//! returns a list of [`Diagnostic`]s.

pub mod diagnostic;
pub mod rules;

pub use diagnostic::{Diagnostic, ErrorCode, Severity};

/// Run all rules and filter out diagnostics on lines suppressed by `# type: ignore`.
pub fn check(module: &basilisk_resolver::ResolvedModule) -> Vec<Diagnostic> {
    let raw = rules::run_all(module);
    raw.into_iter()
        .filter(|d| !line_has_type_ignore(&module.source, d.span.start))
        .collect()
}

/// Returns `true` when the source line containing `byte_offset` has a
/// `# type: ignore` comment (with any optional bracketed error code suffix).
fn line_has_type_ignore(source: &str, byte_offset: u32) -> bool {
    let offset = (byte_offset as usize).min(source.len());
    let line_start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
    let line_end = source[offset..]
        .find('\n')
        .map_or(source.len(), |i| offset + i);
    source[line_start..line_end].contains("# type: ignore")
}
