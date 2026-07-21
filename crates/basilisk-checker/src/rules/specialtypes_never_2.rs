//! Implements [`specialtypes_never_2`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `specialtypes_never_2`: `Never` type compatibility violations.
//!
//! Detects type compatibility errors involving the `Never` bottom type:
//!
//! 1. Assigning a parameter typed `Container[Never]` to a local annotated
//!    `Container[T]` where `T` is not `Never` or `Any` (invariant violation)
//! 2. Returning `ClassC[Never]()` from a function annotated `-> ClassC[U]`
//!    where the class's type parameter is invariant (not covariant)
//!
//! ```python
//! from typing import Never, Any, Generic, TypeVar
//!
//! T = TypeVar("T")
//! U = TypeVar("U")
//!
//! def func(c: list[Never]):
//!     v: list[int] = c  # E0070 — list is invariant, list[Never] != list[int]
//!
//! class ClassC(Generic[T]):
//!     pass
//!
//! def func2(x: U) -> ClassC[U]:
//!     return ClassC[Never]()  # E0070 — ClassC is invariant
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "specialtypes_never_2",
    docs_url: "https://www.basilisk-python.dev/errors/specialtypes_never_2",
};

/// Emits `specialtypes_never_2` for Never type compatibility violations.
pub(crate) struct NeverTypeCompatibility;

impl Rule for NeverTypeCompatibility {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;

        // Collect covariant TypeVar names so we can exclude covariant contexts.
        let covariant_tvars: Vec<&str> =
            basilisk_resolver::collect_names_where(&module.typevar_calls, |tv| tv.is_covariant);

        // Parse candidate annotated assignments (`var: annotation = rhs`) once
        // for the whole file. Previously each function rescanned the entire
        // source, making this O(functions · lines) ≈ O(n²); the candidates are
        // source-position-independent, so one pass feeds every function.
        let assign_lines = collect_assign_lines(source);

        // Index candidates by RHS name: each function only inspects the lines
        // whose RHS is one of its own parameters, instead of every line.
        let mut assign_by_rhs: HashMap<&str, Vec<usize>> = HashMap::new();
        for (idx, cand) in assign_lines.iter().enumerate() {
            assign_by_rhs.entry(cand.rhs_name).or_default().push(idx);
        }

        // Check function bodies for annotated local assignments and return stmts.
        for func in &module.functions {
            check_local_assignments(
                func,
                source,
                path,
                &assign_lines,
                &assign_by_rhs,
                diagnostics,
            );
            check_return_stmts(func, source, path, &covariant_tvars, module, diagnostics);
        }
    }
}

/// One source line that parses as `var_name: annotation = rhs_name`, with the
/// byte offset of the line start. Collected once per file so the per-function
/// `Never` check matches against it without rescanning the source.
struct AssignLine<'a> {
    /// The original (un-trimmed) line text — used to locate `var_name` for the span.
    line: &'a str,
    /// Byte offset of the start of this line in the source.
    line_offset: usize,
    /// The trimmed left-hand-side variable name.
    var_name: &'a str,
    /// The trimmed annotation text (e.g. `list[int]`).
    annotation: &'a str,
    /// The trimmed right-hand-side identifier (a candidate parameter reference).
    rhs_name: &'a str,
    /// [`extract_generic_base`] of `annotation`, precomputed once per line.
    ann_base: &'a str,
    /// [`extract_generic_inner`] of `annotation`, precomputed once per line.
    ann_inner: Option<&'a str>,
}

/// Parse a single line as `var_name: annotation = rhs_name`, returning the
/// trimmed components when it matches the shape the `Never` check looks for.
///
/// Mirrors the original per-line filter exactly: both `": "` and `" = "` must be
/// present, and the LHS name and RHS (after stripping a trailing comment) must be
/// simple identifiers.
fn parse_assign_line(line: &str) -> Option<(&str, &str, &str)> {
    let trimmed = line.trim();
    if !trimmed.contains(": ") || !trimmed.contains(" = ") {
        return None;
    }
    let (var_name, rest) = trimmed.split_once(": ")?;
    let var_name = var_name.trim();
    if !is_simple_identifier(var_name) {
        return None;
    }
    let (annotation, rhs_part) = rest.split_once(" = ")?;
    let annotation = annotation.trim();
    // Strip trailing comments from the RHS before checking it is a plain name.
    let rhs_name = rhs_part.split('#').next().unwrap_or(rhs_part).trim();
    if !is_simple_identifier(rhs_name) {
        return None;
    }
    Some((var_name, annotation, rhs_name))
}

/// Scan the source once, collecting every line that parses as an annotated
/// assignment. Line offsets accumulate exactly as the previous per-line
/// `line_byte_offset` did (`+= line.len() + 1` over [`str::lines`]), so emitted
/// spans are byte-for-byte identical.
fn collect_assign_lines(source: &str) -> Vec<AssignLine<'_>> {
    let mut result = Vec::new();
    let mut offset = 0usize;
    for line in source.lines() {
        if let Some((var_name, annotation, rhs_name)) = parse_assign_line(line) {
            result.push(AssignLine {
                line,
                line_offset: offset,
                var_name,
                annotation,
                rhs_name,
                ann_base: extract_generic_base(annotation),
                ann_inner: extract_generic_inner(annotation),
            });
        }
        offset += line.len() + 1;
    }
    result
}

/// Scan source lines for annotated local variable assignments where the RHS is
/// a parameter typed `Container[Never]` and the LHS declares `Container[T]`
/// with `T` not being `Never` or `Any`.
fn check_local_assignments(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    assign_lines: &[AssignLine<'_>],
    assign_by_rhs: &HashMap<&str, Vec<usize>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Gather violations per parameter via the RHS-name index, then emit in
    // line order (matching the previous candidate-major scan). Parameter names
    // are unique, so each candidate still pairs with at most one parameter.
    let mut matches: Vec<(usize, &str)> = Vec::new();

    for param in &func.parameters {
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let param_annotation = ann_text.trim();
        let Some(indices) = assign_by_rhs.get(param.name.as_str()) else {
            continue;
        };
        // Only a `Container[Never]` parameter can produce a violation.
        let Some(source_inner) = extract_generic_inner(param_annotation) else {
            continue;
        };
        if source_inner != "Never" {
            continue;
        }
        let source_base = extract_generic_base(param_annotation);

        for &idx in indices {
            let Some(candidate) = assign_lines.get(idx) else {
                continue;
            };
            // Check for invariant Never mismatch.
            // E.g. annotation = "list[int]", param_annotation = "list[Never]"
            let Some(target_inner) = candidate.ann_inner else {
                continue;
            };
            if candidate.ann_base == source_base && target_inner != "Never" && target_inner != "Any"
            {
                matches.push((idx, param_annotation));
            }
        }
    }

    matches.sort_unstable_by_key(|&(idx, _)| idx);
    for (idx, param_annotation) in matches {
        let Some(candidate) = assign_lines.get(idx) else {
            continue;
        };
        let name_start_in_line = candidate.line.find(candidate.var_name).unwrap_or(0);
        let span_start = u32::try_from(candidate.line_offset + name_start_in_line).unwrap_or(0);
        let span_end = span_start + u32::try_from(candidate.var_name.len()).unwrap_or(0);

        diagnostics.push(make_assignment_diagnostic(
            Span {
                start: span_start,
                end: span_end,
            },
            candidate.var_name,
            candidate.annotation,
            param_annotation,
            path,
        ));
    }
}

/// Check return statements for invariant Never violations.
fn check_return_stmts(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    covariant_tvars: &[&str],
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = slice_span(source, ann_span) else {
        return;
    };
    let ann_text = ann_text.trim();

    // Only handle generic return types like `ClassC[U]`.
    let Some(ann_inner) = extract_generic_inner(ann_text) else {
        return;
    };
    let ann_base = extract_generic_base(ann_text);

    for ret_stmt in &func.return_stmts {
        if !ret_stmt.has_value {
            continue;
        }
        let Some(ret_full) = slice_span(source, ret_stmt.span) else {
            continue;
        };
        let ret_full = ret_full.trim();

        // Strip "return " prefix.
        let ret_expr = ret_full.strip_prefix("return ").unwrap_or(ret_full).trim();

        // Handle call expressions: `ClassC[Never]()` -> `ClassC[Never]`.
        let ret_type_text = strip_call_parens(ret_expr);

        let Some(ret_inner) = extract_generic_inner(ret_type_text) else {
            continue;
        };
        let ret_base = extract_generic_base(ret_type_text);

        // The generic bases must match.
        if ret_base != ann_base {
            continue;
        }

        // The return expression uses `Never` as a type argument.
        if ret_inner != "Never" {
            continue;
        }

        // The target type parameter is not `Never` or `Any`.
        if ann_inner == "Never" || ann_inner == "Any" {
            continue;
        }

        // Check if the annotation's type parameter is a covariant TypeVar.
        if covariant_tvars.contains(&ann_inner) {
            continue;
        }

        // Check if the class itself declares a covariant type parameter.
        if is_class_param_covariant(ann_base, module, covariant_tvars) {
            continue;
        }

        diagnostics.push(make_return_diagnostic(
            ret_stmt.span,
            &func.name,
            ann_text,
            ret_type_text,
            path,
        ));
    }
}

/// Check if a class's generic type parameter is covariant.
fn is_class_param_covariant(
    class_name: &str,
    module: &ResolvedModule,
    covariant_tvars: &[&str],
) -> bool {
    module
        .classes
        .iter()
        .filter(|cls| cls.name == class_name)
        .flat_map(|cls| &cls.generic_params)
        .any(|param| covariant_tvars.contains(&param.name.as_str()))
}

/// Extract the inner type parameter from a generic annotation.
///
/// `"list[int]"` -> `Some("int")`, `"ClassC[Never]"` -> `Some("Never")`
fn extract_generic_inner(text: &str) -> Option<&str> {
    let bracket_pos = text.find('[')?;
    let close_bracket = text.rfind(']')?;
    if close_bracket <= bracket_pos {
        return None;
    }
    Some(text.get(bracket_pos + 1..close_bracket)?.trim())
}

/// Extract the base name from a generic annotation.
///
/// `"list[int]"` -> `"list"`, `"ClassC[Never]"` -> `"ClassC"`
fn extract_generic_base(text: &str) -> &str {
    text.find('[')
        .map_or(text, |pos| text.get(..pos).unwrap_or(text).trim())
}

/// Strip trailing `()` call from an expression.
///
/// `"ClassC[Never]()"` -> `"ClassC[Never]"`
/// `"ClassC[Never](1, 2)"` -> `"ClassC[Never]"`
fn strip_call_parens(text: &str) -> &str {
    if let Some(stripped) = text.strip_suffix("()") {
        return stripped;
    }
    if let Some(pos) = text.find("](") {
        return text.get(..=pos).unwrap_or(text);
    }
    text
}

/// Check if a string looks like a simple Python identifier.
fn is_simple_identifier(text: &str) -> bool {
    basilisk_resolver::is_simple_ascii_python_identifier(text)
}

fn make_assignment_diagnostic(
    span: Span,
    var_name: &str,
    annotation: &str,
    source_type: &str,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign `{source_type}` to `{var_name}` annotated `{annotation}`: \
             `Never` is only compatible with `Never` and `Any` in invariant contexts"
        ),
        span,
        path,
        Some("Change the annotation to `Never` or `Any`, or change the assigned value".to_owned()),
        Some(
            "PEP 484: `Never` is a bottom type and cannot be assigned to other types \
             except in covariant contexts or when the target is `Any`"
                .to_owned(),
        ),
    )
}

fn make_return_diagnostic(
    span: Span,
    func_name: &str,
    return_annotation: &str,
    return_expr: &str,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot return `{return_expr}` from `{func_name}` annotated \
             `-> {return_annotation}`: `Never` is only compatible with `Never` \
             and `Any` in invariant contexts"
        ),
        span,
        path,
        Some("Change the return type annotation or the returned value".to_owned()),
        Some(
            "PEP 484: `Never` is a bottom type and cannot substitute for invariant \
             type parameters"
                .to_owned(),
        ),
    )
}
