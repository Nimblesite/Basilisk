//! Language Server Protocol support for Basilisk.
//!
//! # Current state
//!
//! This crate exposes [`check_source`], which runs the full checker pipeline
//! on an in-memory Python source string and returns diagnostic messages.
//! It is used directly by the VS Code extension (subprocess approach) and
//! will become the foundation for a full LSP server in a later phase.
//!
//! # LSP server
//!
//! A full `textDocument/publishDiagnostics` server is deferred — see
//! `docs/lsp-plan.md` for the implementation plan.

/// Run the Basilisk checker on a Python source string.
///
/// Returns one formatted string per diagnostic in the form
/// `"BSK-E0001:1:9: Missing parameter type annotation for `x`"`.
///
/// Returns an empty `Vec` when the source has no type errors.
///
/// # Errors (parse failures)
///
/// If the source cannot be parsed, returns a single string starting with
/// `"parse-error:"`.
#[must_use]
pub fn check_source(source: &str) -> Vec<String> {
    let parsed = match basilisk_parser::parse_source(source.to_owned(), "<stdin>".to_owned()) {
        Ok(p) => p,
        Err(e) => return vec![format!("parse-error: {e}")],
    };

    let resolved = match basilisk_resolver::resolve(&parsed) {
        Ok(r) => r,
        Err(e) => return vec![format!("resolve-error: {e}")],
    };

    let diagnostics = basilisk_checker::check(&resolved);

    diagnostics
        .into_iter()
        .map(|d| {
            // Compute 1-based line/col from the byte span.
            let (line, col) = byte_offset_to_line_col(source, d.span.start as usize);
            format!("{}:{}:{}: {}", d.code.code, line, col, d.message)
        })
        .collect()
}

fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let clamped = offset.min(source.len());
    let before = &source[..clamped];
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = before.rfind('\n').map_or(clamped, |pos| clamped - pos - 1) + 1;
    (line, col)
}
