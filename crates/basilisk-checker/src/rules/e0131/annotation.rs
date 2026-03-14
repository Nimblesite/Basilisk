//! Annotation parsing for BSK-E0131.
//!
//! Provides utilities for recognising and decomposing generator-like return
//! annotations such as `Generator[Y, S, R]`, `Iterator[Y]`, and `Iterable[Y]`.

/// Parsed generator return annotation.
#[expect(
    clippy::struct_field_names,
    reason = "field names intentionally mirror the type parameter names"
)]
pub(super) struct GeneratorAnnotation {
    /// The yield type (first type parameter).
    pub(super) yield_type: String,
    /// The send type (second type parameter), if present.
    pub(super) send_type: Option<String>,
    /// The return type (third type parameter), if present.
    pub(super) return_type: Option<String>,
}

/// Try to parse a return annotation as a generator-like type.
///
/// Recognises: `Generator[Y, S, R]`, `Iterator[Y]`, `Iterable[Y]`.
pub(super) fn parse_generator_annotation(ann: &str) -> Option<GeneratorAnnotation> {
    let ann = ann.trim();

    // Check for Generator[Y, S, R]
    if let Some(inner) = strip_generic_prefix(ann, "Generator") {
        let args = split_top_level_args(inner);
        if args.is_empty() {
            return None;
        }
        return Some(GeneratorAnnotation {
            yield_type: args.first()?.trim().to_owned(),
            send_type: args.get(1).map(|s| s.trim().to_owned()),
            return_type: args.get(2).map(|s| s.trim().to_owned()),
        });
    }

    // Check for Iterator[Y]
    if let Some(inner) = strip_generic_prefix(ann, "Iterator") {
        let args = split_top_level_args(inner);
        if args.is_empty() {
            return None;
        }
        return Some(GeneratorAnnotation {
            yield_type: args.first()?.trim().to_owned(),
            send_type: None,
            return_type: None,
        });
    }

    // Check for Iterable[Y]
    if let Some(inner) = strip_generic_prefix(ann, "Iterable") {
        let args = split_top_level_args(inner);
        if args.is_empty() {
            return None;
        }
        return Some(GeneratorAnnotation {
            yield_type: args.first()?.trim().to_owned(),
            send_type: None,
            return_type: None,
        });
    }

    None
}

/// Strip a generic prefix like `Generator[` and return the inner content (without trailing `]`).
pub(super) fn strip_generic_prefix<'a>(ann: &'a str, prefix: &str) -> Option<&'a str> {
    let with_bracket = format!("{prefix}[");
    if !ann.starts_with(&with_bracket) {
        return None;
    }
    let inner_start = with_bracket.len();
    let inner_end = ann.rfind(']')?;
    if inner_end <= inner_start {
        return None;
    }
    ann.get(inner_start..inner_end)
}

/// Split comma-separated type arguments at the top level (respecting bracket nesting).
pub(super) fn split_top_level_args(inner: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;

    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                if let Some(slice) = inner.get(start..idx) {
                    args.push(slice);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if let Some(slice) = inner.get(start..) {
        args.push(slice);
    }
    args
}
