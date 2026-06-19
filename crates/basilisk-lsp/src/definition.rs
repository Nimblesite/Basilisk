//! Implements [LSPARCH-FEATURES-DEFINITION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-DEFINITION
//!
//! Go to Definition handler.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Url};

use crate::util::{
    definition_span, find_definition_by_name, find_symbol_at_offset, identifier_at_offset,
    span_to_range, SymbolHit,
};

/// Compute go-to-definition for a byte offset.
///
/// If the cursor is on a definition site, returns that location.
/// If the cursor is on a reference (call, variable use), finds the definition.
#[must_use]
pub fn goto_definition(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    uri: &Url,
) -> Option<GotoDefinitionResponse> {
    // First, check if cursor is directly on a known symbol definition.
    if let Some(hit) = find_symbol_at_offset(resolved, byte_offset) {
        // Imports with resolved paths should be handled by cross-file lookup.
        if let SymbolHit::Import(imp) = &hit {
            if imp.resolved_path.is_some() {
                return None;
            }
        }
        let range = span_to_range(source, definition_span(&hit));
        return Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range,
        }));
    }

    // Cursor might be on a reference (call site, variable use).
    // Extract the identifier under the cursor and look up its definition.
    let name = identifier_at_offset(source, byte_offset)?;
    let hit = find_definition_by_name(resolved, &name)?;

    // Imports with resolved paths should be handled by cross-file lookup.
    if let SymbolHit::Import(imp) = &hit {
        if imp.resolved_path.is_some() {
            return None;
        }
    }

    let range = span_to_range(source, definition_span(&hit));
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range,
    }))
}
