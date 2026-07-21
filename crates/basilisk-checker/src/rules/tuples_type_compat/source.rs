//! Implements [`tuples_type_compat`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Source text parsing helpers for `tuples_type_compat`.

use basilisk_resolver::Span;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use crate::rules::shared::split_top_level_commas;

use super::annotation::is_simple_name;

// ---------------------------------------------------------------------------
// Source text parsing helpers
// ---------------------------------------------------------------------------

/// Parse an annotated declaration line: `name: annotation` or `name: annotation = value`.
/// Returns `(name, annotation_text)` on success.
pub(super) fn parse_annotated_decl(line: &str) -> Option<(String, String)> {
    // Must contain `:` before any `=`.
    let colon_pos = line.find(':')?;
    let name = line[..colon_pos].trim();
    if !is_simple_name(name) {
        return None;
    }
    let after_colon = line[colon_pos + 1..].trim();
    // Strip `= value` part if present (at top level).
    let annotation = strip_assignment_rhs(after_colon).trim().to_owned();
    if annotation.is_empty() {
        return None;
    }
    Some((name.to_owned(), annotation))
}

/// Strip the `= value` suffix from an annotation string, respecting brackets.
fn strip_assignment_rhs(s: &str) -> &str {
    let mut depth = 0i32;
    for (i, byte) in s.bytes().enumerate() {
        match byte {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'=' if depth == 0 => return &s[..i],
            _ => {}
        }
    }
    s
}

/// Parse a bare assignment line: `name = expr`.
/// Returns `(lhs, rhs)` on success (no `:` in the line, one `=` at top level).
pub(super) fn parse_bare_assignment(line: &str) -> Option<(String, &str)> {
    // Must not be an annotated assignment.
    if line.contains(':') {
        // Could be a comment after `# E`, ignore annotation lines.
        // But a `:` in a type annotation is fine — we just don't want `name: T = val` here.
        // If the colon appears before the `=`, it's an annotated assignment.
        let colon_pos = line.find(':')?;
        let eq_pos = find_top_level_eq(line)?;
        if colon_pos < eq_pos {
            return None;
        }
    }
    let eq_pos = find_top_level_eq(line)?;
    let lhs = line[..eq_pos].trim();
    let rhs = line[eq_pos + 1..].trim();
    // Strip trailing comment.
    let rhs = strip_trailing_comment(rhs);
    if !is_simple_name(lhs) {
        return None;
    }
    Some((lhs.to_owned(), rhs))
}

/// Find the position of the first `=` at top level (not `==`).
fn find_top_level_eq(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    for (i, &byte) in bytes.iter().enumerate() {
        match byte {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                // Ensure not `==`, `!=`, `<=`, `>=`
                let prev_ok = i == 0
                    || bytes
                        .get(i - 1)
                        .is_none_or(|&b| !matches!(b, b'!' | b'<' | b'>' | b'='));
                let next_ok = bytes.get(i + 1).is_none_or(|&b| b != b'=');
                if prev_ok && next_ok {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Strip a trailing `# ...` comment from a source fragment.
fn strip_trailing_comment(s: &str) -> &str {
    // Walk forward; once we see `#` outside a string, stop.
    let mut in_str = false;
    let mut str_char = b'"';
    for (i, &byte) in s.as_bytes().iter().enumerate() {
        match byte {
            b'"' | b'\'' if !in_str => {
                in_str = true;
                str_char = byte;
            }
            c if in_str && c == str_char => {
                in_str = false;
            }
            b'#' if !in_str => return s.get(..i).unwrap_or(s).trim_end(),
            _ => {}
        }
    }
    s.trim_end()
}

/// Parse a tuple literal `(elem1, elem2, ...)` into its element strings.
/// Returns `None` if the text is not a tuple literal.
pub(super) fn parse_tuple_literal(s: &str) -> Option<Vec<String>> {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    let parts = split_top_level_commas(inner);
    Some(
        parts
            .into_iter()
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty())
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Source line iteration helpers
// ---------------------------------------------------------------------------

/// A single line of source with its indentation level and byte offset.
pub(super) struct LineInfo<'src> {
    pub(super) text: &'src str,
    pub(super) indent: usize,
    pub(super) offset: usize,
    pub(super) source_offset: usize,
}

/// Iterate over all lines of `source`, yielding `LineInfo` for each.
pub(super) fn iter_source_lines(source: &str) -> Vec<LineInfo<'_>> {
    let mut result = Vec::new();
    let mut offset = 0;
    for line in source.split('\n') {
        let indent = line.len() - line.trim_start().len();
        result.push(LineInfo {
            text: line,
            indent,
            offset,
            source_offset: offset,
        });
        offset += line.len() + 1; // +1 for the `\n` we stripped
    }
    result
}

/// Extract source lines belonging to a function body (lines after the `def` line
/// that are indented past the `def` line's own indentation).
///
/// Uses the shared line index to jump straight to the `def` line in O(log n),
/// then scans only the body — rather than rescanning the whole source from
/// offset 0 for every function, which made a file of F functions O(F · n) ≈
/// O(n²). Byte offsets and the returned lines are identical to that scan.
pub(super) fn func_body_lines<'a>(
    index: &basilisk_common::text::LineIndex,
    source: &'a str,
    def_offset: usize,
) -> Vec<LineInfo<'a>> {
    let def_line = index.line(def_offset).saturating_sub(1); // 0-based def line
    let def_start = index.line_start_of(def_line);
    let Some(rest) = source.get(def_start..) else {
        return Vec::new();
    };

    let mut lines = rest.split('\n');
    // The first segment is the `def` line itself: capture its indentation, but
    // do not emit it (the body is the lines that follow).
    let Some(def_text) = lines.next() else {
        return Vec::new();
    };
    let def_indent = def_text.len() - def_text.trim_start().len();

    let mut result = Vec::new();
    let mut offset = def_start + def_text.len() + 1;
    for line in lines {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        // Stop when we hit a non-blank, non-comment line at or before the def's indent.
        if !trimmed.is_empty() && !trimmed.starts_with('#') && indent <= def_indent {
            break;
        }
        result.push(LineInfo {
            text: line,
            indent,
            offset,
            source_offset: offset,
        });
        offset += line.len() + 1;
    }
    result
}

/// Compute a `Span` for an entire source line given the line's byte offset.
#[expect(
    clippy::cast_possible_truncation,
    reason = "byte offsets fit u32 for source files"
)]
#[expect(
    clippy::as_conversions,
    reason = "byte offsets fit u32 for source files"
)]
pub(super) fn line_span(source: &str, line_offset: usize) -> Span {
    let start = line_offset as u32;
    let end = source
        .get(line_offset..)
        .and_then(|s| s.find('\n'))
        .map_or(source.len(), |i| line_offset + i) as u32;
    Span { start, end }
}

/// Build a diagnostic from a message, span, and file path.
pub(super) fn make_diag(
    message: &'static str,
    span: Span,
    path: &str,
    code: &ErrorCode,
) -> Diagnostic {
    error_diagnostic_owned(
        code.clone(),
        format!("Tuple type compatibility violation: {message}"),
        span,
        path,
        Some("Ensure the assigned tuple matches the declared starred-unpack annotation".to_owned()),
        Some(
            "See https://typing.readthedocs.io/en/latest/spec/tuples.html#type-compatibility-rules"
                .to_owned(),
        ),
    )
}
