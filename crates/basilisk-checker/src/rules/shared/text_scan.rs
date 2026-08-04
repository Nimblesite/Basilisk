//! ⚠️ LEGACY — condemned under [TYPEINF-LEGACY]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-LEGACY.
//!
//! Source-text geometry, top-level splitting, and line tokenisation shared by
//! rules that still scan annotation or source text ([CHKARCH-DIAG]). Text
//! scanning is not a type mechanism: types come from the engine
//! ([TYPEINF-ALGO]). No new code may call into this module — it is deleted
//! outright per [NARROWPLAN-INTEGRATION] when its last consumer migrates.

use basilisk_resolver::Span;

/// Number of leading whitespace bytes on `line`. Identical to what every rule
/// re-implemented as `line.len() - line.trim_start().len()`.
pub(crate) fn leading_indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Return the byte offset (as `u32`) of the start of the given 1-based line.
/// If `target_line` is past the end of `source`, returns `source.len()`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::as_conversions,
    reason = "byte offsets fit u32 for source files"
)]
fn line_to_byte_offset(source: &str, target_line: usize) -> u32 {
    let mut current = 1usize;
    for (byte_idx, ch) in source.char_indices() {
        if current == target_line {
            return byte_idx as u32;
        }
        if ch == '\n' {
            current += 1;
        }
    }
    source.len() as u32
}

/// Returns `true` when `inner` contains a comma at bracket-depth zero.
///
/// Bracket-depth tracks `[`/`(`/`{` openers and their matching closers. Used
/// by rules that need to decide whether a parenthesised expression like
/// `(a, b)` is a tuple at top level versus a single bracketed group.
pub(crate) fn contains_top_level_comma(inner: &str) -> bool {
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Returns `true` when `s` is a `(...)` parenthesised expression whose
/// contents contain a top-level comma (i.e. a tuple expression).
pub(crate) fn paren_has_top_level_comma(s: &str) -> bool {
    if s.len() < 2 || !s.starts_with('(') || !s.ends_with(')') {
        return false;
    }
    contains_top_level_comma(&s[1..s.len() - 1])
}

/// Build a `Span` covering the trimmed content of a given 1-based line.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "u32<->usize safe on 32-bit+"
)]
pub(crate) fn span_for_line(source: &str, line_number: usize) -> Span {
    let start = line_to_byte_offset(source, line_number) as usize;
    let line_text = source
        .get(start..)
        .and_then(|s| s.lines().next())
        .unwrap_or("");
    let trimmed_start = start + (line_text.len() - line_text.trim_start().len());
    let trimmed_end = start + line_text.trim_end().len();
    Span {
        start: trimmed_start as u32,
        end: trimmed_end as u32,
    }
}

/// Split `s` at every top-level comma, respecting bracket nesting and string
/// literals — a comma inside quotes (`Literal[',']`) is part of the literal
/// value, not a separator (issue #316).
///
/// Returns slices into the original string (no allocation for the parts
/// themselves). Callers that need trimmed/owned values can chain
/// `.iter().map(|p| p.trim().to_owned())`.
pub(crate) fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut in_string: Option<char> = None;
    let mut start = 0;
    for (idx, ch) in s.char_indices() {
        match in_string {
            Some(quote) => {
                if ch == quote {
                    in_string = None;
                }
            }
            None => match ch {
                '\'' | '"' => in_string = Some(ch),
                '[' | '(' | '{' => depth += 1,
                ']' | ')' | '}' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    parts.push(&s[start..idx]);
                    start = idx + 1;
                }
                _ => {}
            },
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Yield `(identifier, index_after_delimiter)` for every identifier token in
/// `line` that is immediately followed by `delim` (e.g. `[` for subscripts,
/// `(` for calls).
///
/// Rules that scan source lines for `ClassName[...]` / `ClassName(...)`
/// patterns use this to dispatch each line's tokens through a hash lookup —
/// O(tokens) per line — instead of running a formatted substring search per
/// known class per line, which is O(classes × line length) and dominated
/// whole-file checks on class-heavy modules.
pub(crate) fn identifiers_followed_by(
    line: &str,
    delim: char,
) -> impl Iterator<Item = (&str, usize)> + '_ {
    let mut chars = line.char_indices().peekable();
    std::iter::from_fn(move || {
        while let Some((start, ch)) = chars.next() {
            if !(ch.is_alphanumeric() || ch == '_') {
                continue;
            }
            let mut end = start + ch.len_utf8();
            while let Some(&(idx, next)) = chars.peek() {
                if next.is_alphanumeric() || next == '_' {
                    let _ = chars.next();
                    end = idx + next.len_utf8();
                } else {
                    break;
                }
            }
            if let Some(&(idx, next)) = chars.peek() {
                if next == delim {
                    return Some((&line[start..end], idx + next.len_utf8()));
                }
            }
        }
        None
    })
}
