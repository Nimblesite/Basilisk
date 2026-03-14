//! Helper functions for BSK-E0014: Assignment type incompatibility.
//!
//! Provides literal parsing, tuple annotation/literal splitting, element
//! compatibility checks, and RHS classification utilities.

use basilisk_resolver::{RhsKind, Span};

use crate::span_util::slice_span;
use crate::types::{InferredType, LiteralValue};

// ---------------------------------------------------------------------------
// Literal value parsers
// ---------------------------------------------------------------------------

/// Parse an integer literal from source text into `Literal[value]`.
pub(super) fn parse_int_literal(text: &str) -> Option<InferredType> {
    let text = text.trim().replace('_', "");
    // Handle hex, octal, binary
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        let val = i64::from_str_radix(hex, 16).ok()?;
        return Some(InferredType::Literal(LiteralValue::Int(val)));
    }
    if let Some(oct) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
        let val = i64::from_str_radix(oct, 8).ok()?;
        return Some(InferredType::Literal(LiteralValue::Int(val)));
    }
    if let Some(bin) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        let val = i64::from_str_radix(bin, 2).ok()?;
        return Some(InferredType::Literal(LiteralValue::Int(val)));
    }
    // Handle negative
    if let Some(neg) = text.strip_prefix('-') {
        let val = neg.trim().parse::<i64>().ok()?;
        return Some(InferredType::Literal(LiteralValue::Int(-val)));
    }
    let val = text.parse::<i64>().ok()?;
    Some(InferredType::Literal(LiteralValue::Int(val)))
}

/// Parse a string literal from source text into `Literal[value]`.
pub(super) fn parse_str_literal(text: &str) -> Option<InferredType> {
    let text = text.trim();
    if (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('\'') && text.ends_with('\''))
    {
        let content = text.get(1..text.len().saturating_sub(1))?;
        return Some(InferredType::Literal(LiteralValue::Str(content.to_owned())));
    }
    None
}

/// Parse a boolean literal from source text into `Literal[value]`.
pub(super) fn parse_bool_literal(text: &str) -> Option<InferredType> {
    match text.trim() {
        "True" => Some(InferredType::Literal(LiteralValue::Bool(true))),
        "False" => Some(InferredType::Literal(LiteralValue::Bool(false))),
        _ => None,
    }
}

/// Parse a float literal from source text into `Literal[value]`.
pub(super) fn parse_float_literal(text: &str) -> Option<InferredType> {
    let text = text.trim().replace('_', "");
    let val = text.parse::<f64>().ok()?;
    Some(InferredType::Literal(LiteralValue::Float(val)))
}

/// Parse a bytes literal from source text into `Literal[value]`.
pub(super) fn parse_bytes_literal(text: &str) -> Option<InferredType> {
    let text = text.trim();
    if (text.starts_with("b\"") || text.starts_with("b'"))
        && (text.ends_with('"') || text.ends_with('\''))
    {
        let content = text.get(2..text.len().saturating_sub(1))?;
        return Some(InferredType::Literal(LiteralValue::Bytes(
            content.as_bytes().to_vec(),
        )));
    }
    None
}

// ---------------------------------------------------------------------------
// Tuple helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the annotation is a simple tuple type (no starred components).
///
/// Skips complex types like `tuple[int, *tuple[str, ...]]` that require variadic analysis.
pub(super) fn is_tuple_annotation(ann: &str) -> bool {
    if !ann.starts_with("tuple[") || !ann.ends_with(']') {
        return false;
    }
    // Skip annotations with starred components (TypeVarTuple unpacks)
    match ann.get("tuple[".len()..ann.len().saturating_sub(1)) {
        Some(inner) => !inner.contains('*'),
        None => false,
    }
}

/// Returns `true` if the source text looks like a tuple literal `(...)`.
pub(super) fn is_tuple_literal(text: &str) -> bool {
    text.starts_with('(') && text.ends_with(')')
}

/// Returns `Some(description)` when the tuple literal is incompatible with the annotation.
pub(super) fn check_tuple_literal_mismatch(rhs: &str, ann: &str) -> Option<String> {
    let inner_ann = ann.strip_prefix("tuple[")?.strip_suffix(']')?;

    // Inner content of the tuple literal `(...)`.
    let rhs_inner = rhs.strip_prefix('(')?.strip_suffix(')')?;
    let rhs_elems = split_tuple_literal_elems(rhs_inner);

    // Homogeneous variable-length tuple: `tuple[T, ...]`
    if let Some(elem_type) = inner_ann.strip_suffix(", ...") {
        let elem_type = elem_type.trim();
        for elem in &rhs_elems {
            let elem = elem.trim();
            if !elem.is_empty() && !literal_elem_matches(elem, elem_type) {
                return Some(format!(
                    "a tuple containing `{elem}` (incompatible with `{elem_type}`)"
                ));
            }
        }
        return None;
    }

    // Empty tuple: `tuple[()]`
    if inner_ann.trim() == "()" {
        if !(rhs_elems.is_empty()
            || rhs_elems.len() == 1
                && rhs_elems.first().is_some_and(|elem| elem.trim().is_empty()))
        {
            return Some(format!(
                "a tuple with {} element(s) (expected empty tuple)",
                rhs_elems.len()
            ));
        }
        return None;
    }

    // Fixed-length tuple: split annotation into element types.
    let ann_elems = split_type_list(inner_ann);

    // Count mismatch.
    if rhs_elems.len() != ann_elems.len() {
        return Some(format!(
            "a {}-element tuple (expected {} element(s))",
            rhs_elems.len(),
            ann_elems.len()
        ));
    }

    // Element type mismatches.
    for (idx, (rhs_elem, ann_elem)) in rhs_elems.iter().zip(ann_elems.iter()).enumerate() {
        let rhs_e = rhs_elem.trim();
        let ann_e = ann_elem.trim();
        if !rhs_e.is_empty() && !literal_elem_matches(rhs_e, ann_e) {
            return Some(format!(
                "a tuple with element {idx} `{rhs_e}` (expected type `{ann_e}`)"
            ));
        }
    }

    None
}

/// Split the inner content of a tuple literal by top-level commas.
/// Handles trailing commas: `1,` → `["1"]`, `1, 2` → `["1", "2"]`.
pub(super) fn split_tuple_literal_elems(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = inner.get(start..idx) {
                    parts.push(part.trim());
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = inner.get(start..).unwrap_or_default().trim();
    if !remainder.is_empty() {
        parts.push(remainder);
    }
    parts
}

/// Split a comma-separated type list respecting bracket nesting.
pub(super) fn split_type_list(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = inner.get(start..idx) {
                    let part = part.trim();
                    if !part.is_empty() {
                        parts.push(part);
                    }
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = inner.get(start..).unwrap_or_default().trim();
    if !remainder.is_empty() {
        parts.push(remainder);
    }
    parts
}

/// Returns `true` if a literal element (source text) is compatible with `expected_type`.
pub(super) fn literal_elem_matches(elem: &str, expected: &str) -> bool {
    let expected_lower = expected.to_ascii_lowercase();
    let expected_base = expected_lower
        .split('[')
        .next()
        .unwrap_or(expected_lower.as_str())
        .trim();

    if expected_base == "any" || expected_base == "object" {
        return true;
    }

    let is_int_lit = elem
        .chars()
        .all(|c| c.is_ascii_digit() || c == '_' || c == 'x' || c == 'o' || c == 'b')
        && elem.chars().next().is_some_and(|c| c.is_ascii_digit());
    let is_str_lit = (elem.starts_with('"') && elem.ends_with('"'))
        || (elem.starts_with('\'') && elem.ends_with('\''));
    let is_float_lit =
        elem.contains('.') && elem.chars().next().is_some_and(|c| c.is_ascii_digit());
    let is_bytes_lit = (elem.starts_with("b\"") || elem.starts_with("b'"))
        && (elem.ends_with('"') || elem.ends_with('\''));
    let is_bool_lit = elem == "True" || elem == "False";
    let is_none_lit = elem == "None";

    match expected_base {
        "int" => is_int_lit || is_bool_lit,
        "float" | "complex" => is_float_lit || is_int_lit || is_bool_lit,
        "str" => is_str_lit,
        "bytes" => is_bytes_lit,
        "bool" => is_bool_lit,
        "none" => is_none_lit,
        _ => true, // Unknown types: don't flag
    }
}

// ---------------------------------------------------------------------------
// Dataclass RHS classification
// ---------------------------------------------------------------------------

/// Returns `Some(description)` when the annotation text and RHS kind are
/// clearly incompatible; `None` when the pairing is acceptable or unknown.
pub(super) fn annotation_rhs_mismatch_simple(
    annotation: &str,
    rhs: &RhsKind,
) -> Option<&'static str> {
    // Normalise: strip generic parameters and whitespace, lower-case.
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    match (base.as_str(), rhs) {
        ("int" | "bool" | "float" | "bytes", RhsKind::StrLiteral) => Some("a `str` literal"),
        ("int" | "str" | "float", RhsKind::BytesLiteral) => Some("a `bytes` literal"),
        ("int" | "str" | "bool", RhsKind::FloatLiteral) => Some("a `float` literal"),
        ("str" | "bytes", RhsKind::IntLiteral) => Some("an `int` literal"),
        _ => None,
    }
}

/// Extracts the RHS literal kind from a module-level attribute assignment line.
///
/// Given the target span of `obj.attr` in `obj.attr = value`, finds the `= value`
/// portion and determines the literal kind.
pub(super) fn extract_rhs_kind_from_assign(source: &str, target_span: Span) -> Option<RhsKind> {
    let target_end = target_span.end_usize();
    let line_end = source
        .get(target_end..)?
        .find('\n')
        .map_or(source.len(), |pos| target_end + pos);
    let after_target = source.get(target_end..line_end)?;

    // Find `=` after the target
    let eq_pos = after_target.find('=')?;
    let rhs = after_target.get(eq_pos + 1..)?.trim();

    classify_literal(rhs)
}

/// Classifies a simple literal token into a `RhsKind`.
pub(super) fn classify_literal(text: &str) -> Option<RhsKind> {
    if text.is_empty() {
        return None;
    }

    // Integer literal: starts with digit, no dot
    if text.bytes().next()?.is_ascii_digit() {
        if text.contains('.') {
            return Some(RhsKind::FloatLiteral);
        }
        return Some(RhsKind::IntLiteral);
    }

    // String literal
    if text.starts_with('"')
        || text.starts_with('\'')
        || text.starts_with("f\"")
        || text.starts_with("f'")
    {
        return Some(RhsKind::StrLiteral);
    }

    // Bytes literal
    if text.starts_with("b\"") || text.starts_with("b'") {
        return Some(RhsKind::BytesLiteral);
    }

    // None
    if text.starts_with("None") {
        return Some(RhsKind::NoneValue);
    }

    // Negative numbers
    if text.starts_with('-') {
        return classify_literal(text.get(1..)?.trim_start());
    }

    None
}

// ---------------------------------------------------------------------------
// Annotation extraction
// ---------------------------------------------------------------------------

/// Extract the annotation text from the source line containing `name_span`.
///
/// Looks for `: <annotation>` on the same source line as the variable name,
/// stopping at the `=` sign that introduces the RHS.  Returns `None` if no
/// such pattern is found.
pub(super) fn extract_annotation(source: &str, name_span: Span) -> Option<&str> {
    // Find the byte offset of the start of the line containing the name.
    let start = usize::try_from(name_span.start).ok()?;
    let line_start = source.get(..start)?.rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |pos| start + pos);

    let line = source.get(line_start..line_end)?;

    // Position of the name within the line.
    let name_offset = start.checked_sub(line_start)?;

    // Find `: ` after the name position on this line.
    let colon_pos = line.get(name_offset..)?.find(": ")? + name_offset;
    let after_colon = colon_pos + 2; // skip ': '

    // Find `=` that ends the annotation (must be after the colon).
    let annotation_end = line
        .get(after_colon..)?
        .find('=')
        .map_or(line.len(), |p| after_colon + p);

    let annotation = line.get(after_colon..annotation_end)?.trim();

    if annotation.is_empty() {
        None
    } else {
        Some(annotation)
    }
}

// ---------------------------------------------------------------------------
// Span-based annotation slicing (re-export wrapper)
// ---------------------------------------------------------------------------

/// Slice a span from source text (thin wrapper around `slice_span`).
pub(super) fn slice_annotation_span<'a>(
    source: &'a str,
    span: basilisk_resolver::Span,
) -> Option<&'a str> {
    slice_span(source, span)
}
