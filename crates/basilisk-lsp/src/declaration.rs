//! Go to Declaration handler.
//!
//! For single-file analysis, declaration and definition are equivalent.
//! This module delegates to `definition::goto_definition` so that the
//! LSP server advertises `declarationProvider` and editors wire up
//! "Go to Declaration" correctly.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{GotoDefinitionResponse, Url};

/// Compute go-to-declaration for a byte offset.
///
/// In single-file mode, declaration = definition.
#[must_use]
pub fn goto_declaration(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    uri: &Url,
) -> Option<GotoDefinitionResponse> {
    crate::definition::goto_definition(resolved, source, byte_offset, uri)
}
