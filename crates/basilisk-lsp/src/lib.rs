//! Implements [LSPARCH]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH
//!
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

pub mod ai_typing;
pub mod auto_import;
pub mod call_hierarchy;
pub mod code_actions;
pub mod code_lens;
pub mod color;
pub mod completion;
pub mod config;
pub mod coverage;
pub mod debug;
pub mod declaration;
pub mod definition;
pub mod folding;
pub mod formatting;
pub mod highlight;
pub mod hover;
pub mod import_graph;
pub mod import_resolver;
pub mod inlay_hints;
pub mod profiler;
pub mod references;
pub mod salsa_engine;
pub mod scope_tree;
pub mod selection;
pub mod semantic_tokens;
pub mod server;
pub mod signature;
pub mod symbols;
pub mod test_discovery;
pub mod type_definition;
pub mod type_hierarchy;
pub mod util;
pub mod uv_commands;
pub mod uv_failure;
pub mod websocket;
pub mod workspace;
pub mod workspace_analysis;
pub mod workspace_scan;

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
    check_source_with_config(source, &basilisk_config::BasiliskConfig::default())
}

// Implements [LSPARCH-ARCH-PIPELINE]
/// Like [`check_source`] but honoring an explicit project configuration.
///
/// House rules (e.g. require-annotation `BSK-E0001`) are off by default — the
/// default config is pure PEP conformance — so pass a config that opts them in
/// to observe them. The checker does exactly what the config says; there are no
/// modes. See [CHKARCH-CONFIGURATION-ONLY].
#[must_use]
pub fn check_source_with_config(
    source: &str,
    config: &basilisk_config::BasiliskConfig,
) -> Vec<String> {
    let parsed = match basilisk_parser::parse_source(source.to_owned(), "<stdin>".to_owned()) {
        Ok(p) => p,
        Err(e) => return vec![format!("parse-error: {e}")],
    };

    let resolved = match basilisk_resolver::resolve(&parsed) {
        Ok(r) => r,
        Err(e) => return vec![format!("resolve-error: {e}")],
    };

    let diagnostics = basilisk_checker::check_with_config(&resolved, config);

    diagnostics
        .into_iter()
        .map(|d| {
            // Compute 1-based line/col from the byte span.
            let (line, col) = basilisk_common::text::line_col(source, d.span.start_usize());
            format!("{}:{}:{}: {}", d.code.code, line, col, d.message)
        })
        .collect()
}

/// Start the Basilisk LSP server.
///
/// This function starts a JSON-RPC server on stdio that implements the
/// Language Server Protocol. It's intended to be called from the CLI.
pub use server::run_server;
pub use websocket::run_server_ws_blocking;
