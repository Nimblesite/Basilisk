//! Implements [`annotations_generators_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Yield expression scanning for `annotations_generators_2`.
//!
//! Provides a byte-level scanner that finds `yield` and `yield from`
//! expressions in a function body substring, skipping string literals and
//! comments so that only real yield statements are reported.

/// A yield expression found in a function body.
pub(super) struct YieldExpr {
    /// The byte offset of the `yield` keyword in the source.
    pub(super) offset: u32,
    /// The text of the yielded expression (after `yield`).
    pub(super) expr_text: String,
    /// Whether this is a `yield from` expression.
    pub(super) is_yield_from: bool,
}

/// Find all yield expressions in a function body substring.
pub(super) fn find_yield_expressions(body: &str, body_offset: usize) -> Vec<YieldExpr> {
    let mut results = Vec::new();
    let bytes = body.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        let Some(&current_byte) = bytes.get(pos) else {
            break;
        };

        // Skip string literals (single/double/triple quoted)
        if current_byte == b'\'' || current_byte == b'"' {
            pos = skip_string(body, pos);
            continue;
        }

        // Skip comments
        if current_byte == b'#' {
            while pos < bytes.len() && bytes.get(pos).copied() != Some(b'\n') {
                pos += 1;
            }
            continue;
        }

        // Look for `yield` keyword
        if pos + 5 <= bytes.len() && body.get(pos..pos + 5) == Some("yield") {
            // Make sure it's a standalone keyword (not part of a larger identifier)
            let before_ok = pos == 0
                || bytes
                    .get(pos.wrapping_sub(1))
                    .is_none_or(|&b| !is_identifier_char(b));
            let after_pos = pos + 5;

            if before_ok && after_pos <= bytes.len() {
                // Check for `yield from`
                let is_yield_from = after_pos + 5 <= bytes.len()
                    && body.get(after_pos..after_pos + 5) == Some(" from")
                    && bytes
                        .get(after_pos + 5)
                        .is_none_or(|&b| !is_identifier_char(b));

                if is_yield_from {
                    let expr_start = after_pos + 5;
                    let expr_text = extract_yield_expr(body, expr_start);
                    if let Ok(offset) = u32::try_from(body_offset + pos) {
                        results.push(YieldExpr {
                            offset,
                            expr_text,
                            is_yield_from: true,
                        });
                    }
                } else if bytes
                    .get(after_pos)
                    .is_some_and(|&b| (b == b' ' || b == b'\n') && !is_identifier_char(b))
                {
                    let expr_text = extract_yield_expr(body, after_pos);
                    if let Ok(offset) = u32::try_from(body_offset + pos) {
                        results.push(YieldExpr {
                            offset,
                            expr_text,
                            is_yield_from: false,
                        });
                    }
                }
            }
        }

        pos += 1;
    }

    results
}

/// Extract the expression text after a yield keyword.
///
/// Only horizontal whitespace is skipped: a bare `yield` at end of line must
/// yield the empty expression (`None`), never the next statement's text.
fn extract_yield_expr(body: &str, start: usize) -> String {
    let rest = body
        .get(start..)
        .unwrap_or("")
        .trim_start_matches([' ', '\t']);
    // Find the end of the expression: newline, comment, or end of string
    let mut depth = 0i32;
    let mut end = rest.len();
    for (idx, ch) in rest.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    end = idx;
                    break;
                }
            }
            '#' | '\n' if depth == 0 => {
                end = idx;
                break;
            }
            _ => {}
        }
    }
    rest.get(..end).unwrap_or("").trim().to_owned()
}

/// Returns `true` if the byte is a valid Python identifier character.
pub(super) fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Skip past a string literal starting at `start`, returning the position after the closing quote.
pub(super) fn skip_string(body: &str, start: usize) -> usize {
    let bytes = body.as_bytes();
    let Some(&quote) = bytes.get(start) else {
        return start;
    };

    // Check for triple quotes
    if start + 2 < bytes.len()
        && bytes.get(start + 1).copied() == Some(quote)
        && bytes.get(start + 2).copied() == Some(quote)
    {
        let mut pos = start + 3;
        while pos + 2 < bytes.len() {
            if bytes.get(pos).copied() == Some(quote)
                && bytes.get(pos + 1).copied() == Some(quote)
                && bytes.get(pos + 2).copied() == Some(quote)
            {
                return pos + 3;
            }
            pos += 1;
        }
        return bytes.len();
    }

    // Single quoted string
    let mut pos = start + 1;
    while pos < bytes.len() {
        let Some(&byte) = bytes.get(pos) else {
            break;
        };
        if byte == b'\\' {
            pos += 2;
            continue;
        }
        if byte == quote {
            return pos + 1;
        }
        if byte == b'\n' {
            return pos;
        }
        pos += 1;
    }
    bytes.len()
}
