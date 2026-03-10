//! BSK-E0129: Literal value assignment incompatibility.
//!
//! Detects two classes of Literal-related assignment errors inside function bodies:
//!
//! 1. **`Literal[0]` vs `Literal[False]` non-equivalence (PEP 586)**:
//!    `Literal[0]` and `Literal[False]` are distinct types despite `0 == False` in
//!    Python.  Assigning a `Literal[0]`-typed parameter to a `Literal[False]` local
//!    (or vice versa) is a type error.
//!
//! 2. **Augmented assignment widens a Literal type**:
//!    `a += 3` where `a` is typed `Literal[3, 4, 5]` produces an `int` result,
//!    which is not assignable back to `Literal[3, 4, 5]`.
//!
//! ```python
//! def func(a: Literal[0], b: Literal[False]):
//!     x1: Literal[False] = a  # E — int 0 ≠ bool False in Literal
//!     x2: Literal[0] = b      # E — bool False ≠ int 0 in Literal
//!
//! def func2(a: Literal[3, 4, 5]):
//!     a += 3  # E — result type is `int`, not `Literal[3, 4, 5]`
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0129",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0129",
};

/// Emits BSK-E0129 for Literal value assignment incompatibilities.
pub(crate) struct LiteralValueIncompatible;

impl Rule for LiteralValueIncompatible {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        for func in &module.functions {
            // Build a map of param_name -> literal annotation text for Literal-typed params.
            let param_literals: Vec<(&str, String)> = func
                .parameters
                .iter()
                .filter_map(|param| {
                    let ann_span = param.annotation_span?;
                    let ann = source.get(ann_span.start as usize..ann_span.end as usize)?;
                    let ann = ann.trim();
                    // Only interested in Literal[...] annotations (including L[...] alias).
                    if contains_literal_subscript(ann) {
                        Some((param.name.as_str(), ann.to_owned()))
                    } else {
                        None
                    }
                })
                .collect();

            if param_literals.is_empty() {
                continue;
            }

            // Scan the source lines in the function body region.
            let body_start = func.def_span.start as usize;
            let func_source = &source[body_start..];

            // Find the colon ending the def line, then scan body lines.
            let Some(colon_offset) = func_source.find(':') else {
                continue;
            };
            let body_text = &func_source[colon_offset + 1..];
            let body_byte_start = body_start + colon_offset + 1;

            // Determine function body indentation from first non-empty line.
            let body_indent = detect_body_indent(body_text);
            if body_indent == 0 {
                continue;
            }

            for line in LineIter::new(body_text, body_byte_start) {
                let trimmed = line.text.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                // Stop when indentation drops below the function body level.
                let line_indent = count_leading_spaces(line.text);
                if !trimmed.is_empty() && line_indent < body_indent {
                    break;
                }

                // Check for annotated assignment: `x: Literal[V] = param_name`
                check_annotated_assignment(
                    trimmed,
                    line.byte_offset,
                    &param_literals,
                    diagnostics,
                    path,
                    source,
                );

                // Check for augmented assignment: `param_name += expr`
                check_augmented_assignment(
                    trimmed,
                    line.byte_offset,
                    &param_literals,
                    diagnostics,
                    path,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Annotated assignment check (lines 24-25 pattern)
// ---------------------------------------------------------------------------

/// Check `x: Literal[V] = rhs_name` where `rhs_name` has a different Literal type.
#[allow(clippy::cast_possible_truncation)]
fn check_annotated_assignment(
    trimmed: &str,
    line_byte_offset: usize,
    param_literals: &[(&str, String)],
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
    _source: &str,
) {
    // Pattern: `var_name: <annotation> = <rhs>`
    let Some(colon_pos) = trimmed.find(':') else {
        return;
    };
    let var_name = trimmed[..colon_pos].trim();
    if !is_simple_identifier(var_name) {
        return;
    }

    let after_colon = &trimmed[colon_pos + 1..];
    let Some(eq_pos) = find_top_level_eq(after_colon) else {
        return;
    };

    let annotation = after_colon[..eq_pos].trim();
    let rhs = after_colon[eq_pos + 1..].trim();

    // Strip trailing comments from rhs.
    let rhs = rhs.split('#').next().unwrap_or(rhs).trim();

    // Both annotation and the param must be Literal types.
    if !contains_literal_subscript(annotation) {
        return;
    }

    // RHS must be a simple name that matches a Literal-typed parameter.
    if !is_simple_identifier(rhs) {
        return;
    }

    let Some((_param_name, param_ann)) = param_literals.iter().find(|(name, _)| *name == rhs)
    else {
        return;
    };

    // Extract the Literal inner values from both annotations.
    let Some(target_values) = extract_literal_values(annotation) else {
        return;
    };
    let Some(source_values) = extract_literal_values(param_ann) else {
        return;
    };

    // Check if every source value is in the target values set (considering
    // that int 0 ≠ bool False, int 1 ≠ bool True).
    if !literal_values_assignable(&source_values, &target_values) {
        let var_byte_start = find_var_in_line(trimmed, var_name, line_byte_offset);

        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Type mismatch: `{var_name}` is annotated `{annotation}` but assigned \
                 parameter `{rhs}` with type `{param_ann}`"
            ),
            span: basilisk_resolver::Span {
                start: var_byte_start as u32,
                end: (var_byte_start + var_name.len()) as u32,
            },
            path: path.to_owned(),
            help: Some("`Literal[0]` and `Literal[False]` are not equivalent (PEP 586)".to_owned()),
            note: Some(
                "int and bool Literal values are distinct even when numerically equal".to_owned(),
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// Augmented assignment check (line 33 pattern)
// ---------------------------------------------------------------------------

/// Check `param_name += expr` where param has a Literal type.
#[allow(clippy::cast_possible_truncation)]
fn check_augmented_assignment(
    trimmed: &str,
    line_byte_offset: usize,
    param_literals: &[(&str, String)],
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
) {
    // Detect augmented assignment operators.
    let aug_ops = [
        "+=", "-=", "*=", "/=", "//=", "%=", "**=", "&=", "|=", "^=", "<<=", ">>=",
    ];
    let mut found_op = None;
    for op in &aug_ops {
        if let Some(pos) = trimmed.find(op) {
            // Make sure the char before the op is not part of another operator.
            let before = &trimmed[..pos];
            let target = before.trim();
            if is_simple_identifier(target) {
                found_op = Some((target, *op, pos));
                break;
            }
        }
    }

    let Some((target_name, op, _op_pos)) = found_op else {
        return;
    };

    // Check if the target is a Literal-typed parameter.
    let Some((_param_name, param_ann)) =
        param_literals.iter().find(|(name, _)| *name == target_name)
    else {
        return;
    };

    let var_byte_start = find_var_in_line(trimmed, target_name, line_byte_offset);

    diagnostics.push(Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Augmented assignment `{target_name} {op} ...` is incompatible with \
             declared type `{param_ann}`"
        ),
        span: basilisk_resolver::Span {
            start: var_byte_start as u32,
            end: (var_byte_start + target_name.len()) as u32,
        },
        path: path.to_owned(),
        help: Some(format!(
            "The result of `{op}` on `{param_ann}` widens to `int`, which is not \
             assignable back to `{param_ann}`"
        )),
        note: Some(
            "Augmented assignment re-assigns the target, so the result type must be \
             compatible with the declared Literal type"
                .to_owned(),
        ),
    });
}

// ---------------------------------------------------------------------------
// Literal value extraction and comparison
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation text contains `Literal[` (or an alias like `L[`
/// when imported as `from typing import Literal as L`).
fn contains_literal_subscript(ann: &str) -> bool {
    ann.starts_with("Literal[") || ann.contains(".Literal[") || ann.starts_with("L[")
}

/// Extract individual values from a `Literal[v1, v2, ...]` annotation.
/// Returns normalized string representations.
fn extract_literal_values(ann: &str) -> Option<Vec<String>> {
    let inner = extract_literal_inner(ann)?;
    Some(
        split_top_level_commas(inner)
            .into_iter()
            .map(|v| v.trim().to_owned())
            .collect(),
    )
}

/// Extract the content between `Literal[` and the matching `]`.
fn extract_literal_inner(ann: &str) -> Option<&str> {
    // Support both `Literal[` and `L[`.
    let start_bracket = if let Some(pos) = ann.find("Literal[") {
        pos + "Literal[".len()
    } else if ann.starts_with("L[") {
        2
    } else {
        return None;
    };

    let mut depth = 1i32;
    let bytes = ann.as_bytes();
    let mut idx = start_bracket;
    while idx < bytes.len() {
        match bytes[idx] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&ann[start_bracket..idx]);
                }
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

/// Split a string by commas, respecting bracket nesting.
fn split_top_level_commas(source: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (idx, byte) in source.bytes().enumerate() {
        match byte {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                result.push(&source[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    result.push(&source[start..]);
    result
}

/// Check if source Literal values are all assignable to target Literal values.
/// `Literal[0]` is NOT assignable to `Literal[False]` and vice versa.
fn literal_values_assignable(source_values: &[String], target_values: &[String]) -> bool {
    source_values.iter().all(|source_val| {
        target_values
            .iter()
            .any(|target_val| literal_value_eq(source_val, target_val))
    })
}

/// Two Literal values are equal only if they are textually identical
/// (after normalization). `0` and `False` are NOT equal.
fn literal_value_eq(left: &str, right: &str) -> bool {
    let left = normalize_literal_value(left);
    let right = normalize_literal_value(right);
    left == right
}

/// Normalize a literal value for comparison.
/// - Convert hex/octal/binary integers to decimal.
/// - Keep `True`, `False`, `None`, strings as-is.
fn normalize_literal_value(value: &str) -> String {
    let trimmed = value.trim();

    // Boolean and None stay as-is.
    if matches!(trimmed, "True" | "False" | "None") {
        return trimmed.to_owned();
    }

    // Try to parse as integer (with optional sign and base prefix).
    if let Some(int_val) = parse_int_literal(trimmed) {
        return int_val.to_string();
    }

    trimmed.to_owned()
}

/// Parse a Python integer literal (decimal, hex, octal, binary, optionally signed).
fn parse_int_literal(source: &str) -> Option<i64> {
    let trimmed = source.trim();
    let (is_negative, digits) = if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest.trim())
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest.trim())
    } else {
        (false, trimmed)
    };

    let value = if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        i64::from_str_radix(hex, 16).ok()?
    } else if let Some(bin) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
        i64::from_str_radix(bin, 2).ok()?
    } else if let Some(oct) = digits
        .strip_prefix("0o")
        .or_else(|| digits.strip_prefix("0O"))
    {
        i64::from_str_radix(oct, 8).ok()?
    } else {
        digits.parse::<i64>().ok()?
    };

    if is_negative {
        Some(-value)
    } else {
        Some(value)
    }
}

// ---------------------------------------------------------------------------
// Source scanning helpers
// ---------------------------------------------------------------------------

/// A single line from the source with its byte offset.
struct LineInfo<'a> {
    text: &'a str,
    byte_offset: usize,
}

/// Iterator over lines with byte offsets.
struct LineIter<'a> {
    remaining: &'a str,
    offset: usize,
}

impl<'a> LineIter<'a> {
    fn new(text: &'a str, start_offset: usize) -> Self {
        Self {
            remaining: text,
            offset: start_offset,
        }
    }
}

impl<'a> Iterator for LineIter<'a> {
    type Item = LineInfo<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        let newline_pos = self.remaining.find('\n').unwrap_or(self.remaining.len());
        let line = &self.remaining[..newline_pos];
        let info = LineInfo {
            text: line,
            byte_offset: self.offset,
        };
        let consumed = if newline_pos < self.remaining.len() {
            newline_pos + 1
        } else {
            newline_pos
        };
        self.remaining = &self.remaining[consumed..];
        self.offset += consumed;
        Some(info)
    }
}

/// Count leading spaces in a line.
fn count_leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|b| *b == b' ').count()
}

/// Detect body indentation from the first non-empty line.
fn detect_body_indent(body_text: &str) -> usize {
    for line in body_text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            return count_leading_spaces(line);
        }
    }
    0
}

/// Find the `=` sign at the top level (not inside brackets) after the annotation.
fn find_top_level_eq(text: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (idx, byte) in text.bytes().enumerate() {
        match byte {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                // Make sure it's not `==`.
                if text.as_bytes().get(idx + 1) != Some(&b'=')
                    && (idx == 0 || text.as_bytes().get(idx.wrapping_sub(1)) != Some(&b'='))
                    // Also not `!=`, `<=`, `>=`
                    && (idx == 0 || !matches!(text.as_bytes().get(idx.wrapping_sub(1)), Some(b'!' | b'<' | b'>')))
                {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

/// Simple identifier check.
fn is_simple_identifier(source: &str) -> bool {
    let trimmed = source.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        && trimmed.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Find the byte offset of a variable name within a line at a given base offset.
fn find_var_in_line(line: &str, var_name: &str, line_byte_offset: usize) -> usize {
    line.find(var_name)
        .map_or(line_byte_offset, |pos| line_byte_offset + pos)
}
