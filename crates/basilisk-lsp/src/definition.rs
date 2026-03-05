//! Go to Definition handler.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Url};

use crate::util::{
    find_definition_by_name, find_symbol_at_offset, identifier_at_offset, span_to_range, SymbolHit,
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
    let range = span_to_range(source, definition_span(&hit));
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range,
    }))
}

/// Extract the definition span for a symbol hit.
fn definition_span(hit: &SymbolHit<'_>) -> basilisk_resolver::Span {
    match hit {
        SymbolHit::Function(f) => f.name_span,
        SymbolHit::Class(c) => c.name_span,
        SymbolHit::Variable(v) => v.name_span,
        SymbolHit::Parameter { param, .. } => param.name_span,
        SymbolHit::Attribute { attr, .. } => attr.name_span,
        SymbolHit::Import(i) => i.span,
    }
}
