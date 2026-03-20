//! Shared helper functions for refactoring code actions.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Position, Range, TextEdit, Url, WorkspaceEdit,
};

/// Extract the selected text from the source using an LSP range.
pub(super) fn selected_text(source: &str, range: &Range) -> Option<String> {
    let start = lsp_position_to_byte_offset(source, range.start)?;
    let end = lsp_position_to_byte_offset(source, range.end)?;
    if start >= end || end > source.len() {
        return None;
    }
    source.get(start..end).map(String::from)
}

/// Convert an LSP `Position` to a byte offset in the source string.
fn lsp_position_to_byte_offset(source: &str, pos: Position) -> Option<usize> {
    let target_line = usize::try_from(pos.line).unwrap_or(usize::MAX);
    let target_char = usize::try_from(pos.character).unwrap_or(usize::MAX);
    let mut current_line = 0usize;
    let mut line_start_byte = 0usize;

    for (byte_idx, ch) in source.char_indices() {
        if current_line == target_line {
            let col = byte_idx - line_start_byte;
            if col == target_char {
                return Some(byte_idx);
            }
        }
        if ch == '\n' {
            if current_line == target_line {
                // Character offset exceeds line length; clamp to end of line.
                return Some(byte_idx);
            }
            current_line += 1;
            line_start_byte = byte_idx + 1;
        }
    }

    // Handle position at the very end of the source.
    if current_line == target_line {
        let col = source.len() - line_start_byte;
        if col >= target_char {
            return Some(line_start_byte + target_char);
        }
    }
    None
}

/// Return the leading whitespace of a given line number.
pub(super) fn leading_indent_of_line(source: &str, line: u32) -> String {
    let line_idx = usize::try_from(line).unwrap_or(usize::MAX);
    source
        .lines()
        .nth(line_idx)
        .map(|l| {
            let trimmed = l.trim_start();
            l.get(..l.len() - trimmed.len()).unwrap_or("").to_owned()
        })
        .unwrap_or_default()
}

/// Return the 0-based line number after the last import statement.
pub(super) fn last_import_line(source: &str) -> u32 {
    let mut insert_line: u32 = 0;
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            insert_line = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        }
    }
    insert_line
}

/// Find the position of the closing `]` that matches the bracket at `start`.
pub(super) fn find_matching_bracket(text: &str, start: usize) -> Option<usize> {
    let mut depth: u32 = 1;
    for (offset, ch) in text.get(start..)?.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split type arguments at top-level commas (respecting bracket nesting).
pub(super) fn split_type_args(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;

    for (idx, ch) in text.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = text.get(start..idx) {
                    parts.push(part);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if let Some(part) = text.get(start..) {
        parts.push(part);
    }
    parts
}

/// Find the byte offset where the type annotation begins (after `: ` or `-> `).
pub(super) fn find_annotation_start(line: &str) -> Option<usize> {
    // Try `-> ` first (return annotation).
    if let Some(arrow_pos) = line.find("-> ") {
        return Some(arrow_pos + 3);
    }
    // Try `: ` (variable/parameter annotation).
    if let Some(colon_pos) = line.find(": ") {
        return Some(colon_pos + 2);
    }
    None
}

/// Find the length of the annotation portion of a string, stopping before a
/// bare `=` (assignment) or `#` (comment) that is not inside brackets.
pub(super) fn find_annotation_end(text: &str) -> usize {
    let mut depth: u32 = 0;
    for (idx, ch) in text.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            '=' | '#' if depth == 0 => return idx,
            _ => {}
        }
    }
    text.len()
}

/// Check if the text contains a `|` that is not nested inside brackets.
pub(super) fn contains_bare_pipe(text: &str) -> bool {
    let mut depth: u32 = 0;
    for ch in text.chars() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Split text on bare `|` characters (not nested inside brackets).
pub(super) fn split_on_bare_pipe(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;

    for (idx, ch) in text.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                if let Some(part) = text.get(start..idx) {
                    parts.push(part);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if let Some(part) = text.get(start..) {
        parts.push(part);
    }
    parts
}

/// Convert a byte offset to a 0-based line number.
pub(super) fn byte_offset_to_line(source: &str, offset: u32) -> u32 {
    let offset_usize = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(source.len());
    let before = source.get(..offset_usize).unwrap_or(source);
    u32::try_from(before.chars().filter(|&c| c == '\n').count()).unwrap_or(u32::MAX)
}

// ── Workspace edit builder ──────────────────────────────────────────────────

/// Fluent builder for constructing multi-file `WorkspaceEdit`s.
///
/// Accumulates text edits per URI and produces a `CodeAction` with the
/// collected changes.  Ensures trailing-whitespace cleanup and consistent
/// newline formatting on all inserted text.
pub(super) struct WorkspaceEditBuilder {
    changes: HashMap<Url, Vec<TextEdit>>,
    title: String,
    kind: CodeActionKind,
}

impl WorkspaceEditBuilder {
    /// Create a new builder with the given action title and kind.
    #[must_use]
    pub(super) fn new(title: impl Into<String>, kind: CodeActionKind) -> Self {
        Self {
            changes: HashMap::new(),
            title: title.into(),
            kind,
        }
    }

    /// Add a text edit for a specific file URI.
    pub(super) fn edit(&mut self, uri: &Url, range: Range, new_text: &str) {
        let formatted = format_inserted_text(new_text);
        self.changes.entry(uri.clone()).or_default().push(TextEdit {
            range,
            new_text: formatted,
        });
    }

    /// Replace a range of full lines `[start_line..end_line)`.
    pub(super) fn replace_lines(&mut self, uri: &Url, start_line: u32, end_line: u32, text: &str) {
        let range = Range {
            start: Position {
                line: start_line,
                character: 0,
            },
            end: Position {
                line: end_line,
                character: 0,
            },
        };
        self.edit(uri, range, text);
    }

    /// Build the final `CodeAction` from accumulated edits.
    ///
    /// Returns `None` if no edits were added.
    #[must_use]
    pub(super) fn build(self) -> Option<CodeAction> {
        if self.changes.is_empty() {
            return None;
        }
        Some(CodeAction {
            title: self.title,
            kind: Some(self.kind),
            diagnostics: None,
            edit: Some(WorkspaceEdit {
                changes: Some(self.changes),
                ..Default::default()
            }),
            is_preferred: Some(false),
            ..Default::default()
        })
    }
}

/// Clean up inserted text: strip trailing whitespace from each line.
fn format_inserted_text(text: &str) -> String {
    let mut result: String = text
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    if text.ends_with('\n') {
        result.push('\n');
    }
    result
}
