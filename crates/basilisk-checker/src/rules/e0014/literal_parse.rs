//! Literal value parsing for BSK-E0014.
//!
//! Provides functions that parse source-text representations of Python literals
//! into `Literal[value]` `InferredType` variants, enabling value-level
//! compatibility checking when the declared type is itself a `Literal`.

use basilisk_resolver::RhsKind;

use crate::inference::infer_rhs;
use crate::span_util::slice_span;
use crate::types::{InferredType, LiteralValue};

use basilisk_resolver::VariableInfo;

/// Infer the RHS type, upgrading to a `Literal[value]` when the declared type
/// is itself a `Literal` and we can extract the actual value from source text.
pub(super) fn infer_with_literal_value(
    var: &VariableInfo,
    source: &str,
    declared: &InferredType,
) -> InferredType {
    let base = infer_rhs(&var.rhs_kind);

    // Only attempt value-level inference when the target is a Literal type
    let is_literal_target = matches!(declared, InferredType::Literal(_) | InferredType::Union(_));
    if !is_literal_target {
        return base;
    }

    // Extract the RHS source text
    let Some(rhs_span) = var.rhs_span else {
        return base;
    };
    let rhs_text = match slice_span(source, rhs_span) {
        Some(text) => text.trim(),
        None => return base,
    };

    // Try to parse a literal value from the source text
    match var.rhs_kind {
        RhsKind::IntLiteral => parse_int_literal(rhs_text).unwrap_or(base),
        RhsKind::StrLiteral => parse_str_literal(rhs_text).unwrap_or(base),
        RhsKind::BoolLiteral => parse_bool_literal(rhs_text).unwrap_or(base),
        RhsKind::FloatLiteral => parse_float_literal(rhs_text).unwrap_or(base),
        RhsKind::BytesLiteral => parse_bytes_literal(rhs_text).unwrap_or(base),
        _ => base,
    }
}

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
