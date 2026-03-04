//! Find All References and Rename Symbol handlers.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{Location, PrepareRenameResponse, Range, TextEdit, Url, WorkspaceEdit};

use crate::util::{
    find_symbol_at_offset, identifier_at_offset, span_to_range, SymbolHit,
};

/// Find all references to the symbol at a byte offset.
#[must_use] pub fn find_references(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    uri: &Url,
    include_declaration: bool,
) -> Vec<Location> {
    let name = symbol_name_at(resolved, source, byte_offset);
    let Some(name) = name else {
        return vec![];
    };

    let mut locations = Vec::new();

    // Find all occurrences of the name as whole-word matches in the source.
    for range in find_identifier_occurrences(source, &name) {
        locations.push(Location {
            uri: uri.clone(),
            range,
        });
    }

    // If not including declaration, try to remove it.
    if !include_declaration {
        if let Some(hit) = find_symbol_at_offset(resolved, byte_offset) {
            let def_range = definition_range(&hit, source);
            locations.retain(|loc| loc.range != def_range);
        }
    }

    locations
}

/// Prepare rename: check if the position is renameable and return the range + text.
#[must_use] pub fn prepare_rename(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Option<PrepareRenameResponse> {
    let name = symbol_name_at(resolved, source, byte_offset)?;

    // Must be on a known symbol.
    let hit = find_symbol_at_offset(resolved, byte_offset);
    let ident = identifier_at_offset(source, byte_offset);

    // Accept if cursor is on a definition or a known identifier.
    if hit.is_some() || ident.is_some() {
        let range = identifier_range_at(source, byte_offset)?;
        return Some(PrepareRenameResponse::RangeWithPlaceholder {
            range,
            placeholder: name,
        });
    }

    None
}

/// Rename the symbol at a byte offset, returning a workspace edit.
#[must_use] pub fn rename_symbol(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    uri: &Url,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let name = symbol_name_at(resolved, source, byte_offset)?;

    let edits: Vec<TextEdit> = find_identifier_occurrences(source, &name)
        .into_iter()
        .map(|range| TextEdit {
            range,
            new_text: new_name.to_owned(),
        })
        .collect();

    if edits.is_empty() {
        return None;
    }

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), edits);
    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
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

fn definition_range(hit: &SymbolHit<'_>, source: &str) -> Range {
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

/// Find all whole-word occurrences of `name` in `source`, returning LSP ranges.
pub(crate) fn find_identifier_occurrences(source: &str, name: &str) -> Vec<Range> {
    let bytes = source.as_bytes();
    let name_bytes = name.as_bytes();
    let mut results = Vec::new();
    let mut search_start = 0;

    while let Some(pos) = source[search_start..].find(name) {
        let abs_pos = search_start + pos;
        let end_pos = abs_pos + name.len();

        // Check word boundaries.
        let at_word_start =
            abs_pos == 0 || !is_ident_byte(bytes[abs_pos - 1]);
        let at_word_end =
            end_pos >= bytes.len() || !is_ident_byte(bytes[end_pos]);

        if at_word_start && at_word_end {
            // Make sure we're not inside a string or comment (simple heuristic).
            if !is_in_string_or_comment(source, abs_pos) {
                let start = crate::util::byte_offset_to_position(source, abs_pos);
                let end = crate::util::byte_offset_to_position(source, end_pos);
                results.push(Range { start, end });
            }
        }

        search_start = abs_pos + name_bytes.len().max(1);
    }

    results
}

/// Compute the LSP range of the identifier at a byte offset.
fn identifier_range_at(source: &str, offset: usize) -> Option<Range> {
    let bytes = source.as_bytes();
    if offset >= bytes.len() || !is_ident_byte(bytes[offset]) {
        return None;
    }
    let mut start = offset;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && is_ident_byte(bytes[end]) {
        end += 1;
    }
    let lsp_start = crate::util::byte_offset_to_position(source, start);
    let lsp_end = crate::util::byte_offset_to_position(source, end);
    Some(Range {
        start: lsp_start,
        end: lsp_end,
    })
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Simple heuristic: check if position is inside a `#` comment or string literal.
pub(crate) fn is_in_string_or_comment(source: &str, offset: usize) -> bool {
    // Find the start of the line containing offset.
    let line_start = source[..offset].rfind('\n').map_or(0, |p| p + 1);
    let line_before = &source[line_start..offset];

    // If there's a `#` before us on the same line (outside strings), it's a comment.
    if let Some(hash_pos) = line_before.find('#') {
        // Simple check: no string quote before the hash on this line.
        let before_hash = &line_before[..hash_pos];
        let single_quotes = before_hash.chars().filter(|&c| c == '\'').count();
        let double_quotes = before_hash.chars().filter(|&c| c == '"').count();
        if single_quotes % 2 == 0 && double_quotes % 2 == 0 {
            return true;
        }
    }

    false
}
