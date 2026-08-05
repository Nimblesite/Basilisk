//! ⚠️ LEGACY — condemned under [TYPEINF-LEGACY]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-LEGACY.
//!
//! Annotation-**string** parsing into [`InferredType`]. NOT the engine's
//! path — an annotation is a type expression resolved through the
//! [TYPEINF-ANNOTATION-RESOLUTION] cascade, never text a rule slices out of
//! the file. No new code may call into this module; existing consumers are
//! deleted per [NARROWPLAN-INTEGRATION], and this parser dies with them.

use super::types::InferredType;

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
            // Implements [TYPEINF-SPECIAL-ANY] — `object` (and bare gradual
            // forms) parse to the Any escape hatch for assignment purposes.
            "object" | "tuple" | "type" => InferredType::Any,
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
    // PEP 604 unions split first (bracket-aware).
    let union_parts = split_top_level_pipe(annotation);
    if union_parts.len() > 1 {
        return InferredType::Union(
            union_parts
                .iter()
                .map(|part| InferredType::from_annotation(part.trim()))
                .collect(),
        );
    }
    parse_container_annotation(annotation)
}

/// Parse container types (list, dict, set, tuple).
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
    InferredType::Named(annotation.to_owned())
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
/// string literals — a comma inside quotes is part of the quoted text, not a
/// separator (issue #316).
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
