//! Implements [tuples_index_2] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! tuples_index_2: Tuple index out of range.
//!
//! Detects subscript access on a fixed-length `tuple[T1, T2, ...]` parameter
//! where the index is a known integer literal (either an inline `int` literal
//! or a parameter typed as `Literal[N]`) that falls outside the valid range
//! `[-len, len-1]`.
//!
//! ```python
//! def f(v: tuple[int, str, list[bool]], b: Literal[5]):
//!     v[b]   # E — index 5 out of range for 3-element tuple
//!     v[4]   # E — index 4 out of range
//!     v[-4]  # E — index -4 out of range (valid: -3..-1)
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::split_top_level_commas;
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "tuples_index_2",
    docs_url: "https://www.basilisk-python.dev/errors/tuples_index_2",
};

/// Emits tuples_index_2 when a tuple is subscripted with an out-of-range literal index.
pub(crate) struct TupleIndexOutOfRange;

impl Rule for TupleIndexOutOfRange {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;

        for func in &module.functions {
            // Build maps: param_name -> tuple length, param_name -> literal int value
            let mut tuple_params: Vec<(&str, usize)> = Vec::new();
            let mut literal_int_params: Vec<(&str, i64)> = Vec::new();

            for param in &func.parameters {
                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann) = slice_span(source, ann_span) else {
                    continue;
                };
                let ann = ann.trim();

                if let Some(len) = parse_fixed_tuple_length(ann) {
                    tuple_params.push((&param.name, len));
                } else if let Some(val) = parse_literal_int_annotation(ann) {
                    literal_int_params.push((&param.name, val));
                }
            }

            if tuple_params.is_empty() {
                continue;
            }

            // Scan lines in the function body for subscript expressions.
            // We look for patterns like `name[index]` on each source line.
            let Some(func_start) = usize::try_from(func.def_span.start).ok() else {
                continue;
            };
            let Some(body_source) = source.get(func_start..) else {
                continue;
            };

            // Find lines belonging to this function body (indented lines after the def line).
            for line in body_source.lines().skip(1) {
                // Stop at non-indented, non-empty lines (next top-level def/class/etc.)
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if !line.starts_with(' ') && !line.starts_with('\t') {
                    break;
                }

                // Look for subscript patterns: `tuple_param[index]`
                for &(tuple_name, tuple_len) in &tuple_params {
                    check_subscript_on_line(
                        trimmed,
                        tuple_name,
                        tuple_len,
                        &literal_int_params,
                        source,
                        line,
                        func_start,
                        &module.path,
                        diagnostics,
                    );
                }
            }
        }
    }
}

/// Check a single source line for out-of-range subscript access on a tuple parameter.
#[expect(
    clippy::too_many_arguments,
    reason = "tuple subscript check requires full source context"
)]
fn check_subscript_on_line(
    trimmed: &str,
    tuple_name: &str,
    tuple_len: usize,
    literal_int_params: &[(&str, i64)],
    full_source: &str,
    raw_line: &str,
    _func_start: usize,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find all occurrences of `tuple_name[` in the trimmed line
    let search_pattern = format!("{tuple_name}[");
    let mut search_from = 0usize;

    while let Some(pos) = trimmed
        .get(search_from..)
        .and_then(|s| s.find(&search_pattern))
    {
        let abs_pos = search_from + pos;
        let bracket_pos = abs_pos + tuple_name.len();

        // Ensure this is not part of a longer identifier
        if abs_pos > 0 {
            let Some(&prev_char) = trimmed.as_bytes().get(abs_pos - 1) else {
                search_from = bracket_pos + 1;
                continue;
            };
            if prev_char.is_ascii_alphanumeric() || prev_char == b'_' {
                search_from = bracket_pos + 1;
                continue;
            }
        }

        // Extract the index expression between [ and matching ]
        let Some(after_bracket) = trimmed.get(bracket_pos + 1..) else {
            search_from = bracket_pos + 1;
            continue;
        };
        let Some(close_bracket) = find_matching_bracket(after_bracket) else {
            search_from = bracket_pos + 1;
            continue;
        };
        let Some(index_slice) = after_bracket.get(..close_bracket) else {
            search_from = bracket_pos + 1;
            continue;
        };
        let index_expr = index_slice.trim();

        // Determine the integer index value
        let index_value = if let Some(val) = parse_int_literal(index_expr) {
            Some(val)
        } else {
            // Check if it's a parameter name with a Literal[N] annotation
            literal_int_params
                .iter()
                .find(|(name, _)| *name == index_expr)
                .map(|(_, val)| *val)
        };

        if let Some(idx) = index_value {
            let Some(tuple_len_i64) = i64::try_from(tuple_len).ok() else {
                search_from = bracket_pos + 1;
                continue;
            };
            let out_of_range = if idx >= 0 {
                idx >= tuple_len_i64
            } else {
                idx < -tuple_len_i64
            };

            if out_of_range {
                // Compute the span: find this line in the full source
                let line_offset_in_source = raw_line
                    .as_ptr()
                    .addr()
                    .saturating_sub(full_source.as_ptr().addr());
                let expr_start_in_line = raw_line.find(trimmed).unwrap_or(0);
                let span_start =
                    u32::try_from(line_offset_in_source + expr_start_in_line + abs_pos)
                        .unwrap_or(u32::MAX);
                // Span covers `name[index]`
                let span_end = span_start.saturating_add(
                    u32::try_from(bracket_pos + 1 + close_bracket + 1 - abs_pos)
                        .unwrap_or(u32::MAX),
                );

                let max_pos = tuple_len_i64 - 1;

                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!("Tuple index {idx} is out of range for `tuple` of length {tuple_len}"),
                    basilisk_resolver::Span {
                        start: span_start,
                        end: span_end,
                    },
                    path,
                    Some(format!(
                        "Valid indices for a {tuple_len}-element tuple are \
                         -{tuple_len}..{max_pos} (inclusive)"
                    )),
                    Some(
                        "PEP 484: indexing a fixed-length tuple with an out-of-range \
                         literal integer is a type error."
                            .to_owned(),
                    ),
                ));
            }
        }

        search_from = bracket_pos + 1;
    }
}

/// Parse the length of a fixed-length tuple annotation like `tuple[int, str, list[bool]]`.
/// Returns `None` for variadic tuples (`tuple[int, ...]`) or non-tuple annotations.
fn parse_fixed_tuple_length(ann: &str) -> Option<usize> {
    let inner = ann.strip_prefix("tuple[")?;
    // Must end with ]
    let inner = inner.strip_suffix(']')?;

    // Check it's not variadic (contains `...` at top level)
    let elements = split_top_level_commas(inner);
    if elements.iter().any(|e| e.trim() == "...") {
        return None;
    }
    if elements.is_empty()
        || (elements.len() == 1 && elements.first().is_some_and(|e| e.trim().is_empty()))
    {
        // `tuple[()]` — empty tuple
        return Some(0);
    }
    Some(elements.len())
}

/// Parse an annotation like `Literal[5]` or `Literal[-3]` into the integer value.
fn parse_literal_int_annotation(ann: &str) -> Option<i64> {
    let inner = ann.strip_prefix("Literal[")?;
    let inner = inner.strip_suffix(']')?;
    let inner = inner.trim();
    parse_int_literal(inner)
}

/// Parse a string as a Python integer literal (optionally negative).
fn parse_int_literal(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Handle negative sign
    let (negative, digits) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest.trim())
    } else {
        (false, s)
    };
    // Only decimal digits (no hex/bin/oct for now)
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let val: i64 = digits.parse().ok()?;
    Some(if negative { -val } else { val })
}

/// Find the position of the closing `]` that matches the opening one.
/// `s` is expected to start immediately after the opening `[`.
fn find_matching_bracket(s: &str) -> Option<usize> {
    let mut depth = 1i32;
    for (i, byte) in s.bytes().enumerate() {
        match byte {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}
