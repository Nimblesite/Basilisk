//! Document Highlight handler.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{DocumentHighlight, DocumentHighlightKind};

use crate::util::{find_symbol_at_offset, identifier_at_offset, span_to_range, SymbolHit};

/// Find all document highlights for the symbol at a byte offset.
#[must_use]
pub fn document_highlights(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Vec<DocumentHighlight> {
    let name = symbol_name_at(resolved, source, byte_offset);
    let Some(name) = name else {
        return vec![];
    };

    // Find all occurrences of the name as whole-word matches in the source.
    let occurrences = crate::references::find_identifier_occurrences(source, &name);
    
    // Get the definition range if we're on a symbol
    let definition_range = find_symbol_at_offset(resolved, byte_offset)
        .map(|hit| definition_range(&hit, source));

    let mut highlights = Vec::new();
    
    for range in occurrences {
        // Determine if this is the definition (WRITE) or a reference (READ)
        let kind = if Some(range) == definition_range {
            Some(DocumentHighlightKind::WRITE)
        } else {
            Some(DocumentHighlightKind::READ)
        };
        
        highlights.push(DocumentHighlight {
            range,
            kind,
        });
    }

    highlights
}

/// Get the symbol name at a byte offset, either from the symbol table or from
/// the identifier under the cursor.
fn symbol_name_at(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Option<String> {
    if let Some(hit) = find_symbol_at_offset(resolved, byte_offset) {
        return Some(symbol_hit_name(&hit).to_owned());
    }
    identifier_at_offset(source, byte_offset)
}

fn symbol_hit_name<'a>(hit: &'a SymbolHit<'a>) -> &'a str {
    match hit {
        SymbolHit::Function(f) => &f.name,
        SymbolHit::Class(c) => &c.name,
        SymbolHit::Variable(v) => &v.name,
        SymbolHit::Parameter { param, .. } => &param.name,
        SymbolHit::Attribute { attr, .. } => &attr.name,
        SymbolHit::Import(i) => &i.module,
    }
}

fn definition_range(hit: &SymbolHit<'_>, source: &str) -> tower_lsp::lsp_types::Range {
    let span = match hit {
        SymbolHit::Function(f) => f.name_span,
        SymbolHit::Class(c) => c.name_span,
        SymbolHit::Variable(v) => v.name_span,
        SymbolHit::Parameter { param, .. } => param.name_span,
        SymbolHit::Attribute { attr, .. } => attr.name_span,
        SymbolHit::Import(i) => i.span,
    };
    span_to_range(source, span)
}