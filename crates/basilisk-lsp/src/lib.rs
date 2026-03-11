//! Language Server Protocol support for Basilisk.
//!
//! This crate provides both a simple `check_source` function for subprocess
//! usage and a full LSP server implementation with IDE features:
//!
//! - Diagnostics (real-time type checking)
//! - Hover (type signatures + diagnostic info)
//! - Go to Definition
//! - Go to Declaration
//! - Go to Type Definition
//! - Document Symbols (Outline)
//! - Signature Help
//! - Find All References
//! - Rename Symbol
//! - Inlay Hints (inferred types + parameter names)
//! - Completion (symbol + dot + builtins)
//! - Code Actions (quick fixes)
//! - Document Formatting (via Ruff)
//! - Document Highlight (symbol occurrences)
//! - Call Hierarchy (incoming + outgoing calls)
//! - Code Lens (reference counts)
//! - Type Hierarchy (supertypes + subtypes)
//! - Folding Ranges (functions, classes, imports)
//! - Selection Ranges (Smart Select)
//! - Semantic Tokens (syntax-aware highlighting)

pub mod call_hierarchy;
pub mod code_actions;
pub mod code_lens;
pub mod color;
pub mod completion;
pub mod config;
pub mod declaration;
pub mod definition;
pub mod folding;
pub mod formatting;
pub mod highlight;
pub mod hover;
pub mod inlay_hints;
pub mod references;
pub mod selection;
pub mod semantic_tokens;
pub mod server;
pub mod signature;
pub mod symbols;
pub mod test_discovery;
pub mod type_definition;
pub mod type_hierarchy;
pub mod util;
pub mod websocket;
pub mod workspace;

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

/// Start the Basilisk LSP server.
///
/// This function starts a JSON-RPC server on stdio that implements the
/// Language Server Protocol. It's intended to be called from the CLI.
pub use server::run_server;
pub use websocket::run_server_ws_blocking;
