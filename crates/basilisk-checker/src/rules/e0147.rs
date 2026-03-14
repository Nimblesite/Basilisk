//! BSK-E0147: Tuple starred-unpack type compatibility violation.
//!
//! Detects assignments where a tuple literal or a tuple-typed variable is
//! assigned to a target whose annotation contains a starred unpack expression
//! (`*tuple[T, ...]` or `*tuple[T]`) and the assignment is incompatible with
//! that annotation.
//!
//! Covers module-level bare reassignments of annotated tuple variables and
//! function-body variable assignments.
//!
//! ## Examples
//!
//! ```python
//! t1: tuple[int, *tuple[str]] = (1, "")  # OK
//! t1 = (1, "", "")  # E — too many elements for *tuple[str]
//!
//! t2: tuple[int, *tuple[str, ...]] = (1, "")  # OK
//! t2 = (1, 1, "")  # E — second element must be str
//!
//! def f(t1: tuple[int], t2: tuple[int, *tuple[int, ...]], t3: tuple[int, ...]):
//!     v2: tuple[int, *tuple[int, ...]]
//!     v2 = t3  # E — homogeneous tuple[int,...] not assignable to mixed starred form
//!     v3: tuple[int]
//!     v3 = t2  # E — t2 may have more elements than v3 allows
//!     v3 = t3  # E — t3 is unbounded, v3 is fixed length 1
//! ```
//!
//! # Specification
//!
//! <https://typing.readthedocs.io/en/latest/spec/tuples.html#type-compatibility-rules>

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0147",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0147",
};

/// Emits BSK-E0147 for incompatible starred-unpack tuple assignments.
pub(crate) struct TupleStarredUnpackCompatibility;

impl Rule for TupleStarredUnpackCompatibility {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        check_module_level(source, path, diagnostics);
        check_function_bodies(module, source, path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Module-level bare reassignment checking
// ---------------------------------------------------------------------------

/// Check module-level bare assignments like `t2 = (1, 1, "")` after a
/// preceding annotated declaration like `t2: tuple[int, *tuple[str, ...]] = ...`.
fn check_module_level(source: &str, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    // Collect annotated module-level variables: name -> annotation text.
    let mut known_annotations: Vec<(String, String)> = Vec::new();

    for line_info in iter_source_lines(source) {
        let trimmed = line_info.text.trim();

        // Skip comment-only lines and blank lines.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Annotated declaration: `name: type` or `name: type = value`
        if let Some((name, annotation)) = parse_annotated_decl(trimmed) {
            if annotation_has_starred_unpack(&annotation) {
                // Insert or update.
                if let Some(existing) = known_annotations.iter_mut().find(|(n, _)| n == &name) {
                    existing.1 = annotation;
                } else {
                    known_annotations.push((name, annotation));
                }
            }
            continue;
        }

        // Bare assignment: `name = (...)` — only module-level (not indented).
        if line_info.indent == 0 {
            if let Some((lhs, rhs)) = parse_bare_assignment(trimmed) {
                // Find previously declared annotation for this name.
                if let Some((_, annotation)) = known_annotations.iter().find(|(n, _)| n == &lhs) {
                    let annotation = annotation.clone();
                    // Only check when RHS is a tuple literal.
                    if let Some(elems) = parse_tuple_literal(rhs) {
                        if let Some(msg) = check_literal_against_annotation(&elems, &annotation) {
                            let span = line_span(source, line_info.offset);
                            diagnostics.push(make_diag(msg, span, path));
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Function-body checking
// ---------------------------------------------------------------------------

/// Check inside function bodies for incompatible assignments to starred-unpack
/// annotated local variables, using parameter types as the source type.
fn check_function_bodies(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for func in &module.functions {
        // Build a map: param_name -> annotation text.
        let mut param_annotations: Vec<(String, String)> = Vec::new();
        for param in &func.parameters {
            if let Some(ann_span) = param.annotation_span {
                if let Some(ann_text) = slice_span(source, ann_span) {
                    param_annotations.push((param.name.clone(), ann_text.trim().to_owned()));
                }
            }
        }

        // Collect local variable annotations declared inside the function.
        let mut local_annotations: Vec<(String, String)> = Vec::new();

        // Extract the function body source (lines indented past the `def`).
        let body_lines = func_body_lines(source, func.def_span.start_usize());

        for line_info in &body_lines {
            let trimmed = line_info.text.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Local annotated decl: `v2: tuple[int, *tuple[int, ...]]` or `v3: tuple[int]`.
            // Track all tuple annotations so we can check assignments where the source
            // type has a starred unpack but the target is a plain fixed-length tuple.
            if let Some((name, annotation)) = parse_annotated_decl(trimmed) {
                if annotation.starts_with("tuple[") {
                    if let Some(existing) = local_annotations.iter_mut().find(|(n, _)| n == &name) {
                        existing.1 = annotation;
                    } else {
                        local_annotations.push((name, annotation));
                    }
                }
                // Even if not a tuple annotation, continue — the annotated decl may
                // also carry a value (handled below as a normal assignment line).
            }

            // Bare assignment: `v2 = t3` inside the function body.
            if let Some((lhs, rhs)) = parse_bare_assignment(trimmed) {
                // Target must have a starred-unpack annotation.
                let target_ann = local_annotations
                    .iter()
                    .find(|(n, _)| n == &lhs)
                    .map(|(_, a)| a.clone());
                let Some(target_ann) = target_ann else {
                    continue;
                };

                let rhs = rhs.trim();

                // RHS is a simple name — look it up as a parameter annotation.
                if is_simple_name(rhs) {
                    if let Some((_, src_ann)) = param_annotations.iter().find(|(n, _)| n == rhs) {
                        let src_ann = src_ann.clone();
                        if let Some(msg) = check_var_against_annotation(&src_ann, &target_ann) {
                            let span = line_span_in_source(source, line_info.source_offset);
                            diagnostics.push(make_diag(msg, span, path));
                        }
                    }
                    continue;
                }

                // RHS is a tuple literal.
                if let Some(elems) = parse_tuple_literal(rhs) {
                    if let Some(msg) = check_literal_against_annotation(&elems, &target_ann) {
                        let span = line_span_in_source(source, line_info.source_offset);
                        diagnostics.push(make_diag(msg, span, path));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tuple annotation compatibility helpers
// ---------------------------------------------------------------------------

/// Check whether a variable annotation (for the source side) is incompatible
/// with the target starred-unpack annotation.
///
/// Handles:
/// - `tuple[T, ...]` (homogeneous) assigned to `tuple[int, *tuple[int, ...]]` (mixed) → E
/// - `tuple[int, *tuple[int, ...]]` or `tuple[int, ...]` assigned to `tuple[int]` → E
fn check_var_against_annotation(src_ann: &str, target_ann: &str) -> Option<&'static str> {
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
fn check_literal_against_annotation(elems: &[String], annotation: &str) -> Option<&'static str> {
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
#[expect(
    clippy::too_many_arguments,
    reason = "tuple type checking requires all context"
)]
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
// Parsed tuple annotation representation
// ---------------------------------------------------------------------------

/// A parsed representation of a tuple type annotation.
#[derive(Debug)]
enum TupleAnnotation {
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
fn parse_tuple_annotation(ann: &str) -> Option<TupleAnnotation> {
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

    let prefix_types: Vec<String> = components.get(..star_idx)
        .unwrap_or_default()
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let suffix_types: Vec<String> = components.get(star_idx + 1..)
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
fn elem_type_compatible(elem: &str, ann_type: &str) -> bool {
    let Some(inferred) = infer_elem_type(elem) else {
        // Cannot infer type — be conservative and allow.
        return true;
    };
    types_assignable(inferred, ann_type)
}

/// Returns `true` when `src_type` is assignable to `target_type`.
fn types_assignable(src: &str, target: &str) -> bool {
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
fn is_int_literal(s: &str) -> bool {
    let s = s.trim().trim_start_matches('-');
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Returns `true` when `s` looks like a Python float literal (has a `.`).
fn is_float_literal(s: &str) -> bool {
    let s = s.trim();
    let s = s.trim_start_matches('-');
    s.contains('.') && s.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Returns `true` when `s` looks like a Python string literal.
fn is_str_literal(s: &str) -> bool {
    let s = s.trim();
    (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
}

/// Returns `true` when `annotation` contains a starred unpack `*tuple[...]`.
fn annotation_has_starred_unpack(annotation: &str) -> bool {
    annotation.contains("*tuple[")
}

/// Returns `true` when `s` is a simple Python identifier.
fn is_simple_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

// ---------------------------------------------------------------------------
// Source text parsing helpers
// ---------------------------------------------------------------------------

/// Parse an annotated declaration line: `name: annotation` or `name: annotation = value`.
/// Returns `(name, annotation_text)` on success.
fn parse_annotated_decl(line: &str) -> Option<(String, String)> {
    // Must contain `:` before any `=`.
    let colon_pos = line.find(':')?;
    let name = line[..colon_pos].trim();
    if !is_simple_name(name) {
        return None;
    }
    let after_colon = line[colon_pos + 1..].trim();
    // Strip `= value` part if present (at top level).
    let annotation = strip_assignment_rhs(after_colon).trim().to_owned();
    if annotation.is_empty() {
        return None;
    }
    Some((name.to_owned(), annotation))
}

/// Strip the `= value` suffix from an annotation string, respecting brackets.
fn strip_assignment_rhs(s: &str) -> &str {
    let mut depth = 0i32;
    for (i, byte) in s.bytes().enumerate() {
        match byte {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'=' if depth == 0 => return &s[..i],
            _ => {}
        }
    }
    s
}

/// Parse a bare assignment line: `name = expr`.
/// Returns `(lhs, rhs)` on success (no `:` in the line, one `=` at top level).
fn parse_bare_assignment(line: &str) -> Option<(String, &str)> {
    // Must not be an annotated assignment.
    if line.contains(':') {
        // Could be a comment after `# E`, ignore annotation lines.
        // But a `:` in a type annotation is fine — we just don't want `name: T = val` here.
        // If the colon appears before the `=`, it's an annotated assignment.
        let colon_pos = line.find(':')?;
        let eq_pos = find_top_level_eq(line)?;
        if colon_pos < eq_pos {
            return None;
        }
    }
    let eq_pos = find_top_level_eq(line)?;
    let lhs = line[..eq_pos].trim();
    let rhs = line[eq_pos + 1..].trim();
    // Strip trailing comment.
    let rhs = strip_trailing_comment(rhs);
    if !is_simple_name(lhs) {
        return None;
    }
    Some((lhs.to_owned(), rhs))
}

/// Find the position of the first `=` at top level (not `==`).
fn find_top_level_eq(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                // Ensure not `==`, `!=`, `<=`, `>=`
                let prev_ok = i == 0 || !matches!(bytes[i - 1], b'!' | b'<' | b'>' | b'=');
                let next_ok = i + 1 >= bytes.len() || bytes[i + 1] != b'=';
                if prev_ok && next_ok {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Strip a trailing `# ...` comment from a source fragment.
fn strip_trailing_comment(s: &str) -> &str {
    // Walk forward; once we see `#` outside a string, stop.
    let mut in_str = false;
    let mut str_char = b'"';
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' if !in_str => {
                in_str = true;
                str_char = bytes[i];
            }
            c if in_str && c == str_char => {
                in_str = false;
            }
            b'#' if !in_str => return s[..i].trim_end(),
            _ => {}
        }
        i += 1;
    }
    s.trim_end()
}

/// Parse a tuple literal `(elem1, elem2, ...)` into its element strings.
/// Returns `None` if the text is not a tuple literal.
fn parse_tuple_literal(s: &str) -> Option<Vec<String>> {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    let parts = split_top_level_commas(inner);
    Some(
        parts
            .into_iter()
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty())
            .collect(),
    )
}

/// Split `s` by top-level commas (respecting `[]`, `()`, `{}`).
fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, byte) in s.bytes().enumerate() {
        match byte {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Strip the outer `]` from a string that starts immediately after `[`.
/// Handles nested brackets correctly.
fn strip_outer_bracket(s: &str) -> Option<&str> {
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

// ---------------------------------------------------------------------------
// Source line iteration helpers
// ---------------------------------------------------------------------------

/// A single line of source with its indentation level and byte offset.
struct LineInfo<'src> {
    text: &'src str,
    indent: usize,
    offset: usize,
    source_offset: usize,
}

/// Iterate over all lines of `source`, yielding `LineInfo` for each.
fn iter_source_lines(source: &str) -> Vec<LineInfo<'_>> {
    let mut result = Vec::new();
    let mut offset = 0;
    for line in source.split('\n') {
        let indent = line.len() - line.trim_start().len();
        result.push(LineInfo {
            text: line,
            indent,
            offset,
            source_offset: offset,
        });
        offset += line.len() + 1; // +1 for the `\n` we stripped
    }
    result
}

/// Extract source lines belonging to a function body (lines after the `def` line
/// that are indented past the `def` line's own indentation).
fn func_body_lines(source: &str, def_offset: usize) -> Vec<LineInfo<'_>> {
    // Find the line that contains `def_offset`.
    let mut result = Vec::new();
    let mut offset = 0;
    let mut found_def = false;
    let mut def_indent = 0usize;

    for line in source.split('\n') {
        let line_end = offset + line.len();
        let indent = line.len() - line.trim_start().len();

        if found_def {
            let trimmed = line.trim();
            // Stop when we hit a non-blank, non-comment line at or before the def's indent.
            if !trimmed.is_empty() && !trimmed.starts_with('#') && indent <= def_indent {
                break;
            }
            result.push(LineInfo {
                text: line,
                indent,
                offset,
                source_offset: offset,
            });
        } else if offset <= def_offset && def_offset <= line_end {
            found_def = true;
            def_indent = indent;
        }

        offset += line.len() + 1;
    }
    result
}

/// Compute a `Span` for an entire source line given the line's byte offset.
#[expect(
    clippy::cast_possible_truncation,
    reason = "byte offsets fit u32 for source files"
)]
fn line_span(source: &str, line_offset: usize) -> Span {
    let start = line_offset as u32;
    let end = source[line_offset..]
        .find('\n')
        .map_or(source.len(), |i| line_offset + i) as u32;
    Span { start, end }
}

/// Compute a `Span` for a line given its absolute byte offset in the full source.
fn line_span_in_source(source: &str, offset: usize) -> Span {
    line_span(source, offset)
}

/// Build a diagnostic from a message, span, and file path.
fn make_diag(message: &'static str, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!("Tuple type compatibility violation: {message}"),
        span,
        path: path.to_owned(),
        help: Some(
            "Ensure the assigned tuple matches the declared starred-unpack annotation".to_owned(),
        ),
        note: Some(
            "See https://typing.readthedocs.io/en/latest/spec/tuples.html#type-compatibility-rules"
                .to_owned(),
        ),
    }
}
