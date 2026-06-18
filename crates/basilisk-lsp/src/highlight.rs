//! Implements [LSPARCH-FEATURES-HIGHLIGHT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HIGHLIGHT
//!
//! Document Highlight handler.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{DocumentHighlight, DocumentHighlightKind};

use crate::util::{definition_range, find_symbol_at_offset, symbol_name_at};

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
    let definition_range =
        find_symbol_at_offset(resolved, byte_offset).map(|hit| definition_range(&hit, source));

    let mut highlights = Vec::new();

    for range in occurrences {
        // Determine if this is the definition (WRITE) or a reference (READ)
        let kind = if Some(range) == definition_range {
            Some(DocumentHighlightKind::WRITE)
        } else {
            Some(DocumentHighlightKind::READ)
        };

        highlights.push(DocumentHighlight { range, kind });
    }

    highlights
}
