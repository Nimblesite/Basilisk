//! Implements [LSPARCH-FEATURES-REFS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-REFS
//!
//! Find All References and Rename Symbol handlers.
//!
//! Uses the scope tree from `scope_tree` for scope-aware reference finding
//! and rename.  Falls back to whole-file text matching for cross-file
//! references (handled by the navigation handler).

use basilisk_resolver::{FunctionInfo, ResolvedModule};
use tower_lsp::lsp_types::{Location, PrepareRenameResponse, Range, TextEdit, Url, WorkspaceEdit};

use crate::scope_tree;
use crate::source_mask::SourceMask;
use crate::util::{
    definition_range, find_symbol_at_offset, identifier_at_offset, symbol_name_at, SymbolHit,
};

/// Find all references to the symbol at a byte offset (scope-aware).
#[must_use]
pub fn find_references(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    uri: &Url,
    include_declaration: bool,
) -> Vec<Location> {
    let ranges = scope_tree::find_scoped_references(resolved, source, byte_offset);

    let mut locations: Vec<Location> = ranges
        .into_iter()
        .map(|range| Location {
            uri: uri.clone(),
            range,
        })
        .collect();

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
#[must_use]
pub fn prepare_rename(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Option<PrepareRenameResponse> {
    let name = symbol_name_at(resolved, source, byte_offset)?;

    // Must be on a known symbol or identifier.
    let hit = find_symbol_at_offset(resolved, byte_offset);
    let ident = identifier_at_offset(source, byte_offset);

    if hit.is_some() || ident.is_some() {
        let range = identifier_range_at(source, byte_offset)?;
        return Some(PrepareRenameResponse::RangeWithPlaceholder {
            range,
            placeholder: name,
        });
    }

    None
}

// Implements [LSPARCH-FEATURES-RENAME] and [REFACTOR-RENAME] (single-file core;
// the cross-file handler half lives in server/handlers/navigation.rs under
// [ANALYSIS-CROSSLSP-RENAME]).
/// Rename the symbol at a byte offset, returning a workspace edit (scope-aware).
///
/// Handles parameter keyword arguments and `__all__` entry updates.
#[must_use]
pub fn rename_symbol(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    uri: &Url,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    // Validate the rename first.
    if let Some(rejection) = scope_tree::validate_rename(new_name, resolved, source, byte_offset) {
        if rejection == scope_tree::RenameRejection::InvalidIdentifier {
            return None;
        }
    }

    let name = symbol_name_at(resolved, source, byte_offset)?;

    // Use scope-aware reference finding.
    let mut ranges = scope_tree::find_scoped_references(resolved, source, byte_offset);

    let mask = SourceMask::build(source);

    // If renaming a parameter, also find keyword argument sites and docstring refs.
    let hit = find_symbol_at_offset(resolved, byte_offset);
    if let Some(SymbolHit::Parameter { func, .. }) = &hit {
        let kwarg_ranges = find_keyword_arg_sites(source, &func.name, &name, &mask);
        ranges.extend(kwarg_ranges);
        let doc_ranges = find_docstring_param_references(source, func, &name);
        ranges.extend(doc_ranges);
    }

    // If renaming a module-level symbol, update __all__ entries.
    if matches!(
        &hit,
        Some(SymbolHit::Function(f)) if f.class_name.is_none()
    ) || matches!(&hit, Some(SymbolHit::Class(_) | SymbolHit::Variable(_)))
    {
        let all_ranges = crate::dunder_all::find_dunder_all_entries(source, &name);
        ranges.extend(all_ranges);
    }

    // The sweeps above overlap: a defaulted parameter's `def` line matches both
    // the scope-aware sweep and the keyword-argument sweep, for instance. LSP
    // forbids overlapping edit ranges within one `changes` entry — and identical
    // ranges overlap — so collapse exact duplicates before emitting edits.
    ranges.sort_by_key(|range| {
        (
            range.start.line,
            range.start.character,
            range.end.line,
            range.end.character,
        )
    });
    ranges.dedup();

    let edits: Vec<TextEdit> = ranges
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
    let _ = changes.insert(uri.clone(), edits);
    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}

/// Find keyword argument sites like `func(param_name=value)` in the source.
fn find_keyword_arg_sites(
    source: &str,
    func_name: &str,
    param_name: &str,
    mask: &SourceMask<'_>,
) -> Vec<Range> {
    let mut results = Vec::new();
    let mut line_start = 0;
    for raw_line in source.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(['\n', '\r']);
        if line.contains(func_name) {
            find_kwarg_in_line(line, param_name, line_start, source, mask, &mut results);
        }
        line_start += raw_line.len();
    }
    results
}

/// Find keyword argument occurrences within a single line.
///
/// `line_start` is the byte offset of `line` within `source`. Both the mask
/// query and the emitted range use absolute offsets, so the range goes through
/// `byte_offset_to_position` like every other range in this module — LSP
/// columns are UTF-16 code units, not bytes.
fn find_kwarg_in_line(
    line: &str,
    param_name: &str,
    line_start: usize,
    source: &str,
    mask: &SourceMask<'_>,
    results: &mut Vec<Range>,
) {
    let bytes = line.as_bytes();
    let param_len = param_name.len();
    let pattern = format!("{param_name}=");
    let mut pos = 0;

    while let Some(rel) = line.get(pos..).and_then(|s| s.find(&pattern)) {
        let abs = pos + rel;
        let after_param = abs + param_len;
        let before = line.get(..abs).unwrap_or("");
        let at_word_start = abs == 0 || bytes.get(abs - 1).is_some_and(|&b| !is_ident_byte(b));
        let is_single_eq = bytes.get(after_param + 1).is_none_or(|&b| b != b'=');

        if at_word_start
            && before.contains('(')
            && is_single_eq
            && !mask.is_masked(line_start + abs)
        {
            push_name_range(results, source, line_start + abs, line_start + after_param);
        }
        pos = abs + param_len.max(1);
    }
}

// Implements [REFACTOR-RENAME-DOCS]
/// Find docstring parameter references for a function parameter rename.
///
/// Searches for `:param param_name:` (reST), `:type param_name:`, `@param`,
/// `param_name (type):` (Google), and `param_name :` (`NumPy`) patterns within
/// the function's `def_span`. Returns ranges covering only the `param_name`.
fn find_docstring_param_references(
    source: &str,
    func: &FunctionInfo,
    param_name: &str,
) -> Vec<Range> {
    if func.docstring.is_none() {
        return Vec::new();
    }
    let start = func.def_span.start_usize();
    let end = func.def_span.end_usize().min(source.len());
    let slice = source.get(start..end).unwrap_or("");

    let mut results = Vec::new();
    find_tagged_param_refs(slice, start, param_name, source, &mut results);
    find_google_numpy_param_refs(slice, start, param_name, source, &mut results);
    results
}

/// Find reST/Javadoc-style parameter references (`:param name:`, `@param name`).
fn find_tagged_param_refs(
    slice: &str,
    base_offset: usize,
    param_name: &str,
    source: &str,
    results: &mut Vec<Range>,
) {
    let patterns = [
        format!(":param {param_name}:"),
        format!(":type {param_name}:"),
        format!("@param {param_name} "),
        format!("@type {param_name} "),
    ];
    for pattern in &patterns {
        find_pattern_param(slice, base_offset, param_name, pattern, source, results);
    }
}

/// Search `slice` for `pattern` and emit a range for the `param_name` portion.
fn find_pattern_param(
    slice: &str,
    base_offset: usize,
    param_name: &str,
    pattern: &str,
    source: &str,
    results: &mut Vec<Range>,
) {
    let Some(name_offset_in_pattern) = pattern.find(param_name) else {
        return;
    };
    let mut pos = 0;
    while let Some(rel) = slice.get(pos..).and_then(|s| s.find(pattern)) {
        let name_start = base_offset + pos + rel + name_offset_in_pattern;
        let name_end = name_start + param_name.len();
        push_name_range(results, source, name_start, name_end);
        pos += rel + pattern.len().max(1);
    }
}

/// Push the LSP range spanning the byte offsets `[name_start, name_end)` onto `results`.
fn push_name_range(results: &mut Vec<Range>, source: &str, name_start: usize, name_end: usize) {
    results.push(Range {
        start: crate::util::byte_offset_to_position(source, name_start),
        end: crate::util::byte_offset_to_position(source, name_end),
    });
}

/// Find Google-style `name (type):` and `NumPy`-style `name :` param references.
fn find_google_numpy_param_refs(
    slice: &str,
    base_offset: usize,
    param_name: &str,
    source: &str,
    results: &mut Vec<Range>,
) {
    let mut byte_pos = 0;
    for line in slice.lines() {
        let trimmed = line.trim_start();
        let whitespace_len = line.len() - trimmed.len();
        if trimmed.starts_with(param_name) {
            let after = trimmed.get(param_name.len()..).unwrap_or("");
            let is_match =
                after.starts_with(" (") || after.starts_with(':') || after.starts_with(" :");
            if is_match {
                let name_start = base_offset + byte_pos + whitespace_len;
                let name_end = name_start + param_name.len();
                push_name_range(results, source, name_start, name_end);
            }
        }
        // Advance past this line plus the newline character.
        byte_pos += line.len() + 1;
    }
}

/// Check that the byte after `pos` is not an identifier character (word boundary).
fn has_word_boundary_after(source: &str, pos: usize) -> bool {
    source
        .as_bytes()
        .get(pos)
        .is_none_or(|&b| !is_ident_byte(b))
}

/// Find all whole-word occurrences of `name` in `source`, returning LSP ranges.
///
/// This is the **non-scope-aware** version, still used by cross-file reference
/// search in the navigation handler where we don't have a resolved module for
/// the remote file's scope tree. `mask` must be built from the same `source`;
/// callers sweeping many names over one file build it once.
pub(crate) fn find_identifier_occurrences(
    source: &str,
    name: &str,
    mask: &SourceMask<'_>,
) -> Vec<Range> {
    let bytes = source.as_bytes();
    let name_bytes = name.as_bytes();
    let mut results = Vec::new();
    let mut search_start = 0;

    while let Some(pos) = source.get(search_start..).and_then(|s| s.find(name)) {
        let abs_pos = search_start + pos;
        let end_pos = abs_pos + name.len();

        // Check word boundaries.
        let at_word_start =
            abs_pos == 0 || bytes.get(abs_pos - 1).is_none_or(|&b| !is_ident_byte(b));
        let at_word_end = bytes.get(end_pos).is_none_or(|&b| !is_ident_byte(b));

        if at_word_start && at_word_end && !mask.is_masked(abs_pos) {
            let start = crate::util::byte_offset_to_position(source, abs_pos);
            let end = crate::util::byte_offset_to_position(source, end_pos);
            results.push(Range { start, end });
        }

        search_start = abs_pos + name_bytes.len().max(1);
    }

    results
}

/// Compute the LSP range of the identifier at a byte offset.
fn identifier_range_at(source: &str, offset: usize) -> Option<Range> {
    let bytes = source.as_bytes();
    if !bytes.get(offset).is_some_and(|&b| is_ident_byte(b)) {
        return None;
    }
    let mut start = offset;
    while start > 0 && bytes.get(start - 1).is_some_and(|&b| is_ident_byte(b)) {
        start -= 1;
    }
    let mut end = offset;
    while bytes.get(end).is_some_and(|&b| is_ident_byte(b)) {
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
