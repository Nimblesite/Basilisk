//! ⚠️ LEGACY — condemned under [TYPEINF-LEGACY]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-LEGACY.
//!
//! Annotation-**string** parsing into [`InferredType`]. NOT the engine's
//! path — an annotation is a type expression resolved through the
//! [TYPEINF-ANNOTATION-RESOLUTION] cascade, never text a rule slices out of
//! the file. No new code may call into this module; existing consumers are
//! deleted per [NARROWPLAN-INTEGRATION], and this parser dies with them.

use super::types::{CallableInfo, InferredType, LiteralValue};

impl InferredType {
    /// Parses annotation text into an `InferredType`.
    ///
    /// This is a simplified parser that handles basic type annotations.
    /// For complex types, it returns Named(String) as a fallback.
    #[must_use]
    pub fn from_annotation(annotation: &str) -> InferredType {
        let annotation = annotation.trim().to_ascii_lowercase();

        match annotation.as_str() {
            "int" => InferredType::Int,
            "str" => InferredType::Str,
            "float" | "complex" => InferredType::Float, // complex ⊃ float ⊃ int
            "bool" => InferredType::Bool,
            "bytes" => InferredType::Bytes,
            "none" => InferredType::None_,
            // Bare `tuple` is `tuple[Any, ...]` — equivalent to Any for assignment.
            // Bare `type` is `type[Any]` (any class object) — Any-like for assignment.
            // Implements [TYPEINF-SPECIAL-ANY] — `Any`/`object` (and bare gradual
            // forms) parse to the Any escape hatch for assignment purposes.
            "any" | "object" | "final" | "tuple" | "type" => InferredType::Any,
            // Implements [TYPEINF-SPECIAL-NEVER].
            "never" => InferredType::Never,
            // Implements [TYPEINF-SPECIAL-LITERALSTRING].
            "literalstring" => InferredType::LiteralString,
            // A bare `Callable` annotation is `Callable[..., Any]` (PEP 484):
            // empty `param_types` represents the arbitrary-parameter form.
            "callable" => InferredType::Callable(CallableInfo {
                param_types: Vec::new(),
                return_type: Box::new(InferredType::Any),
            }),
            "generator" => InferredType::Generator(
                Box::new(InferredType::Any),
                Box::new(InferredType::None_),
                Box::new(InferredType::None_),
            ),
            // Bare generics without `[...]` are implicitly parameterised with Any.
            "list" => InferredType::List(Box::new(InferredType::Any)),
            "dict" => InferredType::Dict(Box::new(InferredType::Any), Box::new(InferredType::Any)),
            "set" | "frozenset" => InferredType::Set(Box::new(InferredType::Any)),
            _ => parse_complex_annotation(&annotation),
        }
    }
}

/// Parse complex (non-primitive) annotation text.
///
/// Implements [TYPEINF-SUBTYPING-UNION] — `X | Y` (PEP 604) and `Union[...]`
/// annotations are lowered to [`InferredType::Union`] here.
fn parse_complex_annotation(annotation: &str) -> InferredType {
    // PEP 604 unions split first (bracket-aware) so `L[1] | L[2]` parses as a
    // union of `Literal`s rather than one malformed subscript.
    let union_parts = split_top_level_pipe(annotation);
    if union_parts.len() > 1 {
        return InferredType::Union(
            union_parts
                .iter()
                .map(|part| InferredType::from_annotation(part.trim()))
                .collect(),
        );
    }
    // `Literal[...]`, including the common `from typing import Literal as L`
    // alias spelled `L[...]` (lowercased to `l[`).
    if let Some(inner) =
        strip_subscript(annotation, "literal[").or_else(|| strip_subscript(annotation, "l["))
    {
        return parse_literal_annotation(inner.trim());
    }
    if annotation.starts_with("callable[") && annotation.ends_with(']') {
        let inner = annotation["callable[".len()..annotation.len() - 1].trim();
        return parse_callable_annotation(inner);
    }
    if annotation.starts_with("generator[") && annotation.ends_with(']') {
        let inner = &annotation["generator[".len()..annotation.len() - 1];
        let parts = split_type_params(inner);
        if let [yield_type, send_type, return_type] = parts.as_slice() {
            return InferredType::Generator(
                Box::new(InferredType::from_annotation(yield_type.trim())),
                Box::new(InferredType::from_annotation(send_type.trim())),
                Box::new(InferredType::from_annotation(return_type.trim())),
            );
        }
        return InferredType::Named(annotation.to_owned());
    }
    if annotation == "typeform" {
        return InferredType::TypeForm(Box::new(InferredType::Any));
    }
    if annotation.starts_with("typeform[") && annotation.ends_with(']') {
        let inner = &annotation["typeform[".len()..annotation.len() - 1];
        return InferredType::TypeForm(Box::new(InferredType::from_annotation(inner)));
    }
    if annotation.starts_with("final[") && annotation.ends_with(']') {
        let inner = &annotation["final[".len()..annotation.len() - 1];
        return InferredType::from_annotation(inner);
    }
    if annotation.starts_with("union[") && annotation.ends_with(']') {
        let inner = &annotation["union[".len()..annotation.len() - 1];
        let types: Vec<InferredType> = split_type_params(inner)
            .iter()
            .map(|part| InferredType::from_annotation(part.trim()))
            .collect();
        return InferredType::Union(types);
    }
    parse_container_annotation(annotation)
}

/// Parse container types (list, dict, set, tuple, optional, union via `|`).
///
/// Implements [TYPEINF-COLLECTIONS-LISTS], [TYPEINF-COLLECTIONS-DICTS],
/// [TYPEINF-COLLECTIONS-SETS] and [TYPEINF-COLLECTIONS-TUPLES] at the annotation
/// level — `list[T]`/`dict[K, V]`/`set[T]`/`tuple[...]` (including `tuple[()]` and
/// `tuple[X, ...]`) parse into the corresponding [`InferredType`] container.
fn parse_container_annotation(annotation: &str) -> InferredType {
    if annotation.starts_with("list[") && annotation.ends_with(']') {
        let inner = &annotation[5..annotation.len() - 1];
        return InferredType::List(Box::new(InferredType::from_annotation(inner)));
    }
    if annotation.starts_with("dict[") && annotation.ends_with(']') {
        let inner = &annotation[5..annotation.len() - 1];
        // Bracket-aware split so a nested key type (e.g. `tuple[str, str]`) is
        // not severed at its inner comma — same splitter `tuple[`/`union[` use.
        return match parse_key_value_args(inner) {
            Some((key_type, value_type)) => {
                InferredType::Dict(Box::new(key_type), Box::new(value_type))
            }
            None => InferredType::Named(annotation.to_owned()),
        };
    }
    if annotation.starts_with("set[") && annotation.ends_with(']') {
        let inner = &annotation[4..annotation.len() - 1];
        return InferredType::Set(Box::new(InferredType::from_annotation(inner)));
    }
    if annotation.starts_with("tuple[") && annotation.ends_with(']') {
        let inner = &annotation[6..annotation.len() - 1];
        // `tuple[()]` is the PEP 484 spelling of the empty-tuple type.
        if inner.trim() == "()" {
            return InferredType::Tuple(Vec::new());
        }
        let parts = split_type_params(inner);
        let elem_types: Vec<InferredType> = parts
            .iter()
            .map(|part| InferredType::from_annotation(part.trim()))
            .collect();
        return InferredType::Tuple(elem_types);
    }
    if annotation.starts_with("optional[") && annotation.ends_with(']') {
        let inner = &annotation[9..annotation.len() - 1];
        return InferredType::Optional(Box::new(InferredType::from_annotation(inner)));
    }
    InferredType::Named(annotation.to_owned())
}

/// Strip a `prefix...]` subscript wrapper, returning the inner text when the
/// annotation both starts with `prefix` and ends with `]`.
fn strip_subscript<'a>(annotation: &'a str, prefix: &str) -> Option<&'a str> {
    if annotation.starts_with(prefix) && annotation.ends_with(']') {
        annotation.get(prefix.len()..annotation.len() - 1)
    } else {
        None
    }
}

/// Split an annotation on top-level `|` (PEP 604 union), ignoring any `|` nested
/// inside `[...]` or `(...)`. Returns a single-element vector when there is no
/// top-level `|`.
fn split_top_level_pipe(annotation: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (idx, ch) in annotation.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            '|' if depth == 0 => {
                if let Some(part) = annotation.get(start..idx) {
                    parts.push(part);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if let Some(part) = annotation.get(start..) {
        parts.push(part);
    }
    parts
}

/// Parses the inner content of a `Literal[...]` annotation into an `InferredType`.
///
/// Handles integer, string, boolean, bytes, and None literals, as well as
/// multi-valued Literal types (treated as unions).
fn parse_literal_annotation(inner: &str) -> InferredType {
    let inner = inner.trim();

    if inner.contains(',') {
        let parts = split_type_params(inner);
        if parts.len() > 1 {
            let types: Vec<InferredType> = parts
                .iter()
                .map(|p| parse_single_literal(p.trim()))
                .collect();
            return InferredType::Union(types);
        }
    }

    parse_single_literal(inner)
}

/// Parse a single literal value from a Literal annotation argument.
fn parse_single_literal(val: &str) -> InferredType {
    let val = val.trim();

    if val == "true" {
        return InferredType::Literal(LiteralValue::Bool(true));
    }
    if val == "false" {
        return InferredType::Literal(LiteralValue::Bool(false));
    }
    if val == "none" {
        return InferredType::None_;
    }

    if let Some(n) = val.strip_prefix('-') {
        if let Ok(num) = n.trim().parse::<i64>() {
            return InferredType::Literal(LiteralValue::Int(-num));
        }
    }
    if let Ok(num) = val.parse::<i64>() {
        return InferredType::Literal(LiteralValue::Int(num));
    }
    // Non-decimal integer literals: `Literal[0x14]`, `Literal[0o24]`, `Literal[0b10100]`
    // are all the value `20` and must compare equal across spellings.
    for (prefix_lower, prefix_upper, radix) in [("0x", "0X", 16), ("0o", "0O", 8), ("0b", "0B", 2)]
    {
        if let Some(digits) = val
            .strip_prefix(prefix_lower)
            .or_else(|| val.strip_prefix(prefix_upper))
        {
            if let Ok(num) = i64::from_str_radix(&digits.replace('_', ""), radix) {
                return InferredType::Literal(LiteralValue::Int(num));
            }
        }
    }

    // Length guards keep the slices in range: a lone `'` both starts AND ends
    // with a quote (same byte), and `val[1..0]` panics (issue #316).
    if val.len() >= 2
        && ((val.starts_with('"') && val.ends_with('"'))
            || (val.starts_with('\'') && val.ends_with('\'')))
    {
        let content = &val[1..val.len() - 1];
        return InferredType::Literal(LiteralValue::Str(content.to_owned()));
    }

    if val.len() >= 3
        && (val.starts_with("b\"") || val.starts_with("b'"))
        && (val.ends_with('"') || val.ends_with('\''))
    {
        let content = &val[2..val.len() - 1];
        return InferredType::Literal(LiteralValue::Bytes(content.as_bytes().to_vec()));
    }

    InferredType::Named(val.to_owned())
}

/// Parse the `K, V` inside a two-argument subscript (`dict[K, V]`,
/// `Mapping[K, V]`) into key and value [`InferredType`]s. Returns `None` unless
/// exactly two top-level, bracket-aware arguments are present.
pub(super) fn parse_key_value_args(inner: &str) -> Option<(InferredType, InferredType)> {
    let parts = split_type_params(inner);
    if parts.len() != 2 {
        return None;
    }
    let key = InferredType::from_annotation(parts.first().map_or("", |s| s.trim()));
    let value = InferredType::from_annotation(parts.get(1).map_or("", |s| s.trim()));
    Some((key, value))
}

/// Split type parameters by top-level commas, respecting bracket nesting and
/// string literals — a comma inside quotes (`Literal[',']`) is part of the
/// literal value, not a separator (issue #316).
pub(super) fn split_type_params(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut in_string: Option<char> = None;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match in_string {
            Some(quote) => {
                if ch == quote {
                    in_string = None;
                }
            }
            None => match ch {
                '\'' | '"' => in_string = Some(ch),
                '[' | '(' | '{' => depth = depth.saturating_add(1),
                ']' | ')' | '}' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    parts.push(&inner[start..idx]);
                    start = idx + 1;
                }
                _ => {}
            },
        }
    }
    let remainder = &inner[start..];
    if !remainder.trim().is_empty() {
        parts.push(remainder);
    }
    parts
}

/// Parses the inner content of a `Callable[...]` annotation into an `InferredType`.
///
/// Handles `Callable[[param_types...], return_type]` and `Callable[..., return_type]`.
///
/// Implements [TYPEINF-SUBTYPING-CALLABLE] at the annotation level — produces the
/// [`CallableInfo`] (param list + return type) whose variance is checked by
/// [`InferredType::is_assignable_to`].
fn parse_callable_annotation(inner: &str) -> InferredType {
    let inner = inner.trim();
    let mut depth = 0u32;
    let mut comma_pos = None;

    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' | '{' => depth = depth.saturating_add(1),
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                comma_pos = Some(idx);
                break;
            }
            _ => {}
        }
    }

    let Some(comma_idx) = comma_pos else {
        return InferredType::Named(format!("callable[{inner}]"));
    };

    let param_spec = inner[..comma_idx].trim();
    let return_type_str = inner[comma_idx + 1..].trim();
    let return_type = InferredType::from_annotation(return_type_str);

    if param_spec == "..." {
        return InferredType::Callable(CallableInfo {
            param_types: Vec::new(),
            return_type: Box::new(return_type),
        });
    }

    if param_spec.starts_with('[') && param_spec.ends_with(']') {
        let param_list = param_spec[1..param_spec.len() - 1].trim();

        if param_list.is_empty() {
            return InferredType::Callable(CallableInfo {
                param_types: Vec::new(),
                return_type: Box::new(return_type),
            });
        }

        let param_types = split_callable_params(param_list)
            .iter()
            .map(|param| InferredType::from_annotation(param.trim()))
            .collect();

        return InferredType::Callable(CallableInfo {
            param_types,
            return_type: Box::new(return_type),
        });
    }

    let param_type = InferredType::from_annotation(param_spec);
    InferredType::Callable(CallableInfo {
        param_types: vec![param_type],
        return_type: Box::new(return_type),
    })
}

/// Split Callable parameter list by top-level commas.
fn split_callable_params(param_list: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;

    for (idx, ch) in param_list.char_indices() {
        match ch {
            '[' | '(' | '{' => depth = depth.saturating_add(1),
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&param_list[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }

    let remainder = &param_list[start..];
    if !remainder.trim().is_empty() {
        parts.push(remainder);
    }
    parts
}
