//! Tuple annotation parsing and compatibility helpers for BSK-E0147.

use crate::rules::shared::split_top_level_commas;

// ---------------------------------------------------------------------------
// Parsed tuple annotation representation
// ---------------------------------------------------------------------------

/// A parsed representation of a tuple type annotation.
#[derive(Debug)]
pub(super) enum TupleAnnotation {
    /// `tuple[T1, T2, ..., Tn]` — fully fixed length.
    Fixed { count: usize },
    /// `tuple[T, ...]` — homogeneous unbounded.
    Homogeneous { element_type: String },
    /// Mixed form with a starred unpack in the middle:
    /// `tuple[P1, ..., Pm, *tuple[M, ...], S1, ..., Sk]`
    /// or `tuple[P1, ..., Pm, *tuple[S1, ..., Sk]]` (fixed unpack, `has_unbounded=false`).
    Mixed {
        fixed_prefix: usize,
        fixed_suffix: usize,
        has_unbounded: bool,
        prefix_types: Vec<String>,
        suffix_types: Vec<String>,
        middle_type: Option<String>,
    },
}

/// Parse a `tuple[...]` annotation into a structured form.
///
/// Returns `None` for non-tuple annotations or unparseable forms.
pub(super) fn parse_tuple_annotation(ann: &str) -> Option<TupleAnnotation> {
    let ann = ann.trim();
    let inner = ann.strip_prefix("tuple[")?;
    // Strip outer trailing `]` (must be balanced).
    let inner = strip_outer_bracket(inner)?;
    let inner = inner.trim();

    // Empty tuple: `tuple[()]`
    if inner == "()" {
        return Some(TupleAnnotation::Fixed { count: 0 });
    }

    let components: Vec<&str> = split_top_level_commas(inner)
        .into_iter()
        .map(str::trim)
        .collect();

    // Homogeneous unbounded: `tuple[T, ...]`
    if components.len() == 2 && components.get(1).copied() == Some("...") {
        let element_type = (*components.first()?).to_string();
        return Some(TupleAnnotation::Homogeneous { element_type });
    }

    // Check for a starred unpack component `*tuple[...]`
    let star_pos = components.iter().position(|c| c.starts_with('*'));

    let Some(star_idx) = star_pos else {
        // No starred unpack — plain fixed-length tuple.
        return Some(TupleAnnotation::Fixed {
            count: components.len(),
        });
    };

    let star_component = components.get(star_idx)?;
    // Must be `*tuple[...]`
    let unpack_inner = star_component
        .strip_prefix('*')
        .and_then(|s| s.strip_prefix("tuple["))
        .and_then(|s| strip_outer_bracket(s))?;
    let unpack_inner = unpack_inner.trim();

    let prefix_types: Vec<String> = components
        .get(..star_idx)
        .unwrap_or_default()
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let suffix_types: Vec<String> = components
        .get(star_idx + 1..)
        .unwrap_or_default()
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let fixed_prefix = prefix_types.len();
    let fixed_suffix = suffix_types.len();

    // Parse the unpack contents.
    let unpack_parts: Vec<&str> = split_top_level_commas(unpack_inner)
        .into_iter()
        .map(str::trim)
        .collect();

    if unpack_parts.len() == 2 && unpack_parts.get(1).copied() == Some("...") {
        // `*tuple[T, ...]` — unbounded middle.
        let middle_type = Some((*unpack_parts.first()?).to_string());
        Some(TupleAnnotation::Mixed {
            fixed_prefix,
            fixed_suffix,
            has_unbounded: true,
            prefix_types,
            suffix_types,
            middle_type,
        })
    } else if unpack_parts == ["()"] || unpack_parts.is_empty() {
        // `*tuple[()]` — empty fixed unpack.
        Some(TupleAnnotation::Mixed {
            fixed_prefix,
            fixed_suffix,
            has_unbounded: false,
            prefix_types,
            suffix_types,
            middle_type: None,
        })
    } else {
        // `*tuple[T1, T2]` — fixed unpack (adds T1, T2 to total count).
        let extra_fixed = unpack_parts.len();
        Some(TupleAnnotation::Mixed {
            fixed_prefix: fixed_prefix + extra_fixed,
            fixed_suffix,
            has_unbounded: false,
            prefix_types: {
                let mut p = prefix_types;
                p.extend(unpack_parts.iter().map(|s| (*s).to_owned()));
                p
            },
            suffix_types,
            middle_type: None,
        })
    }
}

/// Check whether a variable annotation (for the source side) is incompatible
/// with the target starred-unpack annotation.
///
/// Handles:
/// - `tuple[T, ...]` (homogeneous) assigned to `tuple[int, *tuple[int, ...]]` (mixed) → E
/// - `tuple[int, *tuple[int, ...]]` or `tuple[int, ...]` assigned to `tuple[int]` → E
pub(super) fn check_var_against_annotation(
    src_ann: &str,
    target_ann: &str,
) -> Option<&'static str> {
    // Parse the target annotation structure.
    let target = parse_tuple_annotation(target_ann)?;
    let src = parse_tuple_annotation(src_ann)?;

    match (&target, &src) {
        // target is a mixed starred form like tuple[int, *tuple[int, ...]]
        // source is a homogeneous unbounded form like tuple[int, ...]
        (TupleAnnotation::Mixed { .. }, TupleAnnotation::Homogeneous { .. }) => {
            Some("homogeneous unbounded tuple is not assignable to mixed starred-unpack form")
        }

        // target is a fixed-length tuple like tuple[int]
        // source is anything with potential unbounded length
        (TupleAnnotation::Fixed { count: target_len }, src_t) => {
            let src_may_be_longer = match src_t {
                TupleAnnotation::Homogeneous { .. } => true,
                TupleAnnotation::Mixed {
                    fixed_prefix,
                    fixed_suffix,
                    has_unbounded,
                    ..
                } => *has_unbounded || (fixed_prefix + fixed_suffix > *target_len),
                TupleAnnotation::Fixed { count: src_len } => src_len > target_len,
            };
            if src_may_be_longer {
                Some("source tuple type may have more elements than the fixed-length target allows")
            } else {
                None
            }
        }

        _ => None,
    }
}

/// Check whether a tuple literal (list of element type strings) is compatible
/// with a starred-unpack annotation.
///
/// Returns `Some(message)` when the literal violates the annotation.
pub(super) fn check_literal_against_annotation(
    elems: &[String],
    annotation: &str,
) -> Option<&'static str> {
    let ann = parse_tuple_annotation(annotation)?;

    match ann {
        TupleAnnotation::Fixed { count } => {
            if elems.len() != count {
                return Some("tuple literal length does not match fixed starred-unpack annotation");
            }
            None
        }

        TupleAnnotation::Homogeneous { element_type } => {
            // Every element must match element_type.
            for elem in elems {
                if !elem_type_compatible(elem, &element_type) {
                    return Some(
                        "tuple literal element type incompatible with homogeneous annotation",
                    );
                }
            }
            None
        }

        TupleAnnotation::Mixed {
            fixed_prefix,
            fixed_suffix,
            has_unbounded,
            prefix_types,
            suffix_types,
            middle_type,
        } => check_literal_against_mixed(
            elems,
            fixed_prefix,
            fixed_suffix,
            has_unbounded,
            &prefix_types,
            &suffix_types,
            middle_type.as_deref(),
        ),
    }
}

/// Check a tuple literal against a mixed starred-unpack annotation
/// like `tuple[int, *tuple[str, ...], int]`.
fn check_literal_against_mixed(
    elems: &[String],
    fixed_prefix: usize,
    fixed_suffix: usize,
    has_unbounded: bool,
    prefix_types: &[String],
    suffix_types: &[String],
    middle_type: Option<&str>,
) -> Option<&'static str> {
    let n = elems.len();
    let min_len = fixed_prefix + fixed_suffix;

    if !has_unbounded {
        // Fixed total length: prefix + suffix (no unbounded middle).
        if n != min_len {
            return Some("tuple literal length does not match fixed starred-unpack annotation");
        }
        // Check prefix types.
        for (i, pt) in prefix_types.iter().enumerate() {
            if let Some(elem) = elems.get(i) {
                if !elem_type_compatible(elem, pt) {
                    return Some("tuple literal element type incompatible with annotation prefix");
                }
            }
        }
        // Check suffix types (from the right).
        for (j, st) in suffix_types.iter().enumerate() {
            let elem_idx = n - fixed_suffix + j;
            if let Some(elem) = elems.get(elem_idx) {
                if !elem_type_compatible(elem, st) {
                    return Some("tuple literal element type incompatible with annotation suffix");
                }
            }
        }
        return None;
    }

    // Unbounded middle: must have at least min_len elements.
    if n < min_len {
        return Some("tuple literal has too few elements for starred-unpack annotation");
    }

    // Check fixed prefix.
    for (i, pt) in prefix_types.iter().enumerate() {
        if let Some(elem) = elems.get(i) {
            if !elem_type_compatible(elem, pt) {
                return Some("tuple literal element type incompatible with annotation prefix");
            }
        }
    }

    // Check fixed suffix (from the right).
    for (j, st) in suffix_types.iter().enumerate() {
        let elem_idx = n - fixed_suffix + j;
        if let Some(elem) = elems.get(elem_idx) {
            if !elem_type_compatible(elem, st) {
                return Some("tuple literal element type incompatible with annotation suffix");
            }
        }
    }

    // Check middle elements against the unbounded type.
    if let Some(mid_type) = middle_type {
        let middle_start = fixed_prefix;
        let middle_end = n - fixed_suffix;
        for elem in elems.get(middle_start..middle_end).unwrap_or_default() {
            if !elem_type_compatible(elem, mid_type) {
                return Some(
                    "tuple literal middle element type incompatible with starred-unpack annotation",
                );
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Type compatibility helpers
// ---------------------------------------------------------------------------

/// Determine the inferred type of a tuple literal element (from source text).
fn infer_elem_type(elem: &str) -> Option<&'static str> {
    let elem = elem.trim();
    if is_int_literal(elem) {
        return Some("int");
    }
    if is_float_literal(elem) {
        return Some("float");
    }
    if is_str_literal(elem) {
        return Some("str");
    }
    None
}

/// Check whether a literal element is compatible with an annotation type.
pub(super) fn elem_type_compatible(elem: &str, ann_type: &str) -> bool {
    let Some(inferred) = infer_elem_type(elem) else {
        // Cannot infer type — be conservative and allow.
        return true;
    };
    types_assignable(inferred, ann_type)
}

/// Returns `true` when `src_type` is assignable to `target_type`.
pub(super) fn types_assignable(src: &str, target: &str) -> bool {
    if src == target {
        return true;
    }
    // int is assignable to float and complex (numeric tower).
    if src == "int" && (target == "float" || target == "complex") {
        return true;
    }
    if src == "bool" && (target == "int" || target == "float" || target == "complex") {
        return true;
    }
    // float is assignable to complex.
    if src == "float" && target == "complex" {
        return true;
    }
    // Any is compatible with everything.
    if src == "Any" || target == "Any" {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Literal parsing helpers
// ---------------------------------------------------------------------------

/// Returns `true` when `s` looks like a Python integer literal.
pub(super) fn is_int_literal(s: &str) -> bool {
    let s = s.trim().trim_start_matches('-');
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Returns `true` when `s` looks like a Python float literal (has a `.`).
pub(super) fn is_float_literal(s: &str) -> bool {
    let s = s.trim();
    let s = s.trim_start_matches('-');
    s.contains('.') && s.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Returns `true` when `s` looks like a Python string literal.
pub(super) fn is_str_literal(s: &str) -> bool {
    let s = s.trim();
    (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
}

/// Returns `true` when `annotation` contains a starred unpack `*tuple[...]`.
pub(super) fn annotation_has_starred_unpack(annotation: &str) -> bool {
    annotation.contains("*tuple[")
}

/// Returns `true` when `s` is a simple Python identifier.
pub(super) fn is_simple_name(s: &str) -> bool {
    basilisk_resolver::is_simple_python_identifier(s)
}

// ---------------------------------------------------------------------------
// Bracket and comma splitting utilities
// ---------------------------------------------------------------------------

/// Strip the outer `]` from a string that starts immediately after `[`.
/// Handles nested brackets correctly.
pub(super) fn strip_outer_bracket(s: &str) -> Option<&str> {
    let mut depth = 1i32;
    for (i, byte) in s.bytes().enumerate() {
        match byte {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[..i]);
                }
            }
            _ => {}
        }
    }
    None
}
